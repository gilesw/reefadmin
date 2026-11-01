use crate::{
    AdminError, Row,
    access::{WriteAccess, denied},
};
use async_trait::async_trait;
use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, DbErr,
    EntityTrait, IdenStatic, IntoActiveModel, Iterable, ModelTrait, PrimaryKeyToColumn,
    PrimaryKeyTrait, QueryFilter, QuerySelect, TransactionTrait, TryIntoModel,
    sea_query::ColumnType, sea_query::Expr,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use std::marker::PhantomData;

/// Application-owned derived data updated inside the same transaction as an admin write.
/// `before` captures dependencies that may disappear during cascades or relation edits.
#[async_trait]
pub trait MutationObserver: Send + Sync {
    async fn before(
        &self,
        db: &sea_orm::DatabaseTransaction,
        table: &str,
        row: &Value,
    ) -> Result<Value, DbErr>;
    async fn after(
        &self,
        db: &sea_orm::DatabaseTransaction,
        table: &str,
        row: &Value,
        captured: Value,
    ) -> Result<(), DbErr>;
}

/// How a column's values are parsed from forms and shown in pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColKind {
    Integer,
    Float,
    Decimal,
    Bool,
    Text,
    LongText,
    Date,
    DateTime,
    DateTimeTz,
    Time,
    Json,
    Array,
    Uuid,
    Enum,
    Other,
}

impl ColKind {
    fn from_column_type(column_type: &ColumnType) -> Self {
        match column_type {
            ColumnType::Char(_) | ColumnType::String(_) => Self::Text,
            ColumnType::Text => Self::LongText,
            ColumnType::TinyInteger
            | ColumnType::SmallInteger
            | ColumnType::Integer
            | ColumnType::BigInteger
            | ColumnType::TinyUnsigned
            | ColumnType::SmallUnsigned
            | ColumnType::Unsigned
            | ColumnType::BigUnsigned => Self::Integer,
            ColumnType::Float | ColumnType::Double => Self::Float,
            ColumnType::Decimal(_) | ColumnType::Money(_) => Self::Decimal,
            ColumnType::Boolean => Self::Bool,
            ColumnType::Date => Self::Date,
            ColumnType::DateTime | ColumnType::Timestamp => Self::DateTime,
            ColumnType::TimestampWithTimeZone => Self::DateTimeTz,
            ColumnType::Time => Self::Time,
            ColumnType::Json | ColumnType::JsonBinary => Self::Json,
            ColumnType::Array(_) => Self::Array,
            ColumnType::Uuid => Self::Uuid,
            ColumnType::Enum { .. } => Self::Enum,
            _ => Self::Other,
        }
    }

    pub(crate) fn is_textual(self) -> bool {
        matches!(self, Self::Text | Self::LongText)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ColumnMeta {
    /// Database column name.
    pub name: String,
    /// Key of the column in the model's serde representation.
    pub json_key: String,
    pub kind: ColKind,
    /// Exact database type, retained for prepared comparison bindings.
    pub column_type: ColumnType,
    pub nullable: bool,
    pub enum_values: Vec<String>,
}

/// Entity metadata plus the writes that go through the model's `ActiveModelBehavior`.
#[async_trait]
pub(crate) trait Backend: Send + Sync {
    fn table(&self) -> &str;
    fn columns(&self) -> &[ColumnMeta];
    fn primary_key(&self) -> &[String];
    fn pk_auto_increment(&self) -> bool;

    /// Inserts a row from values keyed by column name and returns the stored row keyed the same way.
    async fn insert(
        &self,
        db: &DatabaseConnection,
        data: Map<String, Value>,
        observer: Option<&dyn MutationObserver>,
        access: &WriteAccess<'_>,
    ) -> Result<Map<String, Value>, AdminError>;

    /// Updates the row whose primary key values are included in `data`.
    async fn update(
        &self,
        db: &DatabaseConnection,
        data: Map<String, Value>,
        observer: Option<&dyn MutationObserver>,
        access: &WriteAccess<'_>,
    ) -> Result<(), AdminError>;

    /// Deletes the row matching `condition`; returns false when no row matched.
    async fn delete(
        &self,
        db: &DatabaseConnection,
        condition: Expr,
        observer: Option<&dyn MutationObserver>,
        access: &WriteAccess<'_>,
    ) -> Result<bool, AdminError>;
}

pub(crate) struct SeaOrmBackend<E> {
    table: String,
    columns: Vec<ColumnMeta>,
    primary_key: Vec<String>,
    _entity: PhantomData<fn() -> E>,
}

impl<E> SeaOrmBackend<E>
where
    E: EntityTrait,
{
    pub(crate) fn new() -> Self {
        let columns = E::Column::iter()
            .map(|column| {
                let def = column.def();
                let column_type = def.get_column_type();
                let enum_values = match column_type {
                    ColumnType::Enum { variants, .. } => {
                        variants.iter().map(|v| v.to_string()).collect()
                    }
                    _ => Vec::new(),
                };
                ColumnMeta {
                    name: column.as_str().to_string(),
                    json_key: column.json_key().to_string(),
                    kind: ColKind::from_column_type(column_type),
                    column_type: column_type.clone(),
                    nullable: def.is_null(),
                    enum_values,
                }
            })
            .collect();
        let primary_key = E::PrimaryKey::iter()
            .map(|pk| pk.into_column().as_str().to_string())
            .collect();
        Self {
            table: E::default().table_name().to_string(),
            columns,
            primary_key,
            _entity: PhantomData,
        }
    }

    /// An ActiveModel with exactly the columns in `data` set.
    fn active_model(&self, data: &Map<String, Value>) -> Result<E::ActiveModel, DbErr>
    where
        E::Model: Serialize + DeserializeOwned + IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: TryIntoModel<E::Model>,
    {
        // `from_json` fills absent keys with dummy values that chrono cannot always parse,
        // so every key is supplied and the absent ones are unset afterwards.
        let full: Map<String, Value> = self
            .columns
            .iter()
            .map(|c| {
                let value = data.get(&c.name).cloned().unwrap_or_else(|| placeholder(c));
                (c.json_key.clone(), value)
            })
            .collect();
        let mut active = E::ActiveModel::from_json(Value::Object(full))?;
        for column in E::Column::iter() {
            if !data.contains_key(column.as_str()) {
                active.not_set(column);
            }
        }
        Ok(active)
    }

    /// Renames a model's serde keys to column names.
    fn column_keys(&self, data: Map<String, Value>) -> Map<String, Value> {
        data.into_iter()
            .map(|(key, value)| {
                let name = self
                    .columns
                    .iter()
                    .find(|c| c.json_key == key)
                    .map_or(key, |c| c.name.clone());
                (name, value)
            })
            .collect()
    }
}

/// A value of the column's type that the model can deserialise.
fn placeholder(column: &ColumnMeta) -> Value {
    if column.nullable {
        return Value::Null;
    }
    match column.kind {
        ColKind::Integer => Value::from(0),
        ColKind::Float => Value::from(0.0),
        ColKind::Decimal => Value::from("0"),
        ColKind::Bool => Value::Bool(false),
        ColKind::Text | ColKind::LongText => Value::from(""),
        ColKind::Date => Value::from("1970-01-01"),
        ColKind::DateTime => Value::from("1970-01-01T00:00:00"),
        ColKind::DateTimeTz => Value::from("1970-01-01T00:00:00+00:00"),
        ColKind::Time => Value::from("00:00:00"),
        ColKind::Uuid => Value::from("00000000-0000-0000-0000-000000000000"),
        ColKind::Enum => column
            .enum_values
            .first()
            .map_or(Value::Null, |v| Value::from(v.as_str())),
        ColKind::Array => Value::Array(vec![]),
        ColKind::Json | ColKind::Other => Value::Null,
    }
}

fn json_err(err: serde_json::Error) -> DbErr {
    DbErr::Json(err.to_string())
}

#[async_trait]
impl<E> Backend for SeaOrmBackend<E>
where
    E: EntityTrait + Sync,
    E::Model: Serialize + DeserializeOwned + IntoActiveModel<E::ActiveModel> + Send + Sync,
    E::ActiveModel: ActiveModelBehavior + TryIntoModel<E::Model> + Send + Sync,
{
    fn table(&self) -> &str {
        &self.table
    }

    fn columns(&self) -> &[ColumnMeta] {
        &self.columns
    }

    fn primary_key(&self) -> &[String] {
        &self.primary_key
    }

    fn pk_auto_increment(&self) -> bool {
        <E::PrimaryKey as PrimaryKeyTrait>::auto_increment()
    }

    async fn insert(
        &self,
        db: &DatabaseConnection,
        data: Map<String, Value>,
        observer: Option<&dyn MutationObserver>,
        access: &WriteAccess<'_>,
    ) -> Result<Map<String, Value>, AdminError> {
        let tx = db.begin().await?;
        access.check(&tx, None, &data).await?;
        let captured = if let Some(o) = observer {
            o.before(&tx, &self.table, &Value::Object(data.clone()))
                .await?
        } else {
            Value::Null
        };
        let model = self.active_model(&data)?.insert(&tx).await?;
        self.check_saved(&tx, &model, access).await?;
        if let Some(o) = observer {
            o.after(
                &tx,
                &self.table,
                &serde_json::to_value(&model).map_err(json_err)?,
                captured,
            )
            .await?;
        }
        tx.commit().await?;
        match serde_json::to_value(&model).map_err(json_err)? {
            Value::Object(map) => Ok(self.column_keys(map)),
            _ => Err(DbErr::Json("model did not serialise to an object".to_string()).into()),
        }
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        data: Map<String, Value>,
        observer: Option<&dyn MutationObserver>,
        access: &WriteAccess<'_>,
    ) -> Result<(), AdminError> {
        let tx = db.begin().await?;
        // Lock before the observer snapshots old relations. A concurrent update
        // must finish before we decide which former dependencies need refreshing.
        let active = self.active_model(&data)?;
        let mut condition = Condition::all();
        for pk in E::PrimaryKey::iter() {
            let column = pk.into_column();
            let value = active
                .get(column)
                .take()
                .ok_or_else(|| DbErr::Custom("Missing primary key".into()))?;
            condition = condition.add(column.eq(value));
        }
        let current = E::find()
            .filter(condition)
            .filter(access.scope.clone())
            .lock_exclusive()
            .one(&tx)
            .await?
            .ok_or_else(denied)?;
        let current = self.model_row(&current)?;
        let mut proposed = current.clone();
        proposed.extend(data.clone());
        access.check(&tx, Some(&current), &proposed).await?;
        let captured = if let Some(o) = observer {
            o.before(&tx, &self.table, &Value::Object(data.clone()))
                .await?
        } else {
            Value::Null
        };
        let model = active.update(&tx).await?;
        self.check_saved(&tx, &model, access).await?;
        if let Some(o) = observer {
            o.after(
                &tx,
                &self.table,
                &serde_json::to_value(&model).map_err(json_err)?,
                captured,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn delete(
        &self,
        db: &DatabaseConnection,
        condition: Expr,
        observer: Option<&dyn MutationObserver>,
        access: &WriteAccess<'_>,
    ) -> Result<bool, AdminError> {
        let tx = db.begin().await?;
        let Some(model) = E::find()
            .filter(condition)
            .filter(access.scope.clone())
            .lock_exclusive()
            .one(&tx)
            .await?
        else {
            return Ok(false);
        };
        let current = self.model_row(&model)?;
        access.check(&tx, Some(&current), &current).await?;
        let row = serde_json::to_value(&model).map_err(json_err)?;
        let captured = if let Some(o) = observer {
            o.before(&tx, &self.table, &row).await?
        } else {
            Value::Null
        };
        model.into_active_model().delete(&tx).await?;
        if let Some(o) = observer {
            o.after(&tx, &self.table, &Value::Null, captured).await?;
        }
        tx.commit().await?;
        Ok(true)
    }
}

impl<E> SeaOrmBackend<E>
where
    E: EntityTrait,
    E::Model: Serialize,
{
    fn model_row(&self, model: &E::Model) -> Result<Row, AdminError> {
        match serde_json::to_value(model).map_err(json_err)? {
            Value::Object(row) => Ok(self.column_keys(row)),
            _ => Err(DbErr::Json("model is not an object".into()).into()),
        }
    }

    async fn check_saved(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        model: &E::Model,
        access: &WriteAccess<'_>,
    ) -> Result<(), AdminError> {
        let mut keys = Condition::all();
        for pk in E::PrimaryKey::iter() {
            let column = pk.into_column();
            keys = keys.add(column.eq(model.get(column)));
        }
        if E::find()
            .filter(keys)
            .filter(access.scope.clone())
            .one(tx)
            .await?
            .is_none()
        {
            return Err(denied());
        }
        Ok(())
    }
}
