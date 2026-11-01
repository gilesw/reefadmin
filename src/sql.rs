use crate::{
    entity::{Computed, EntityAdmin, Row},
    values::{column_expr, eq_condition, in_condition, key_string},
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, FromQueryResult, JsonValue,
    sea_query::{Alias, Condition, Expr, ExprTrait, Func, LikeExpr, Order, Query, SelectStatement},
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Which rows of an entity to read, and in what order.
pub(crate) struct ListSpec {
    pub search: String,
    pub scope: Condition,
    pub conditions: Vec<Expr>,
    /// Column or SQL-computed column names with a descending flag, already validated.
    pub order: Vec<(String, bool)>,
    pub limit: u64,
    pub offset: u64,
}

fn like_pattern(term: &str) -> LikeExpr {
    let mut escaped = String::with_capacity(term.len() + 2);
    escaped.push('%');
    for c in term.to_lowercase().chars() {
        if matches!(c, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped.push('%');
    LikeExpr::new(escaped).escape('\\')
}

fn text_lower(expr: Expr) -> Expr {
    Expr::from(Func::lower(expr.cast_as(Alias::new("text"))))
}

fn search_condition(entity: &EntityAdmin, term: &str) -> Option<Condition> {
    let term = term.trim();
    if term.is_empty() || (entity.search_fields.is_empty() && entity.search_sql.is_empty()) {
        return None;
    }
    let table = entity.backend.table();
    let mut any = Condition::any();
    for field in &entity.search_fields {
        any = any.add(text_lower(column_expr(table, field)).like(like_pattern(term)));
    }
    for sql in &entity.search_sql {
        any = any.add(text_lower(Expr::cust(format!("({sql})"))).like(like_pattern(term)));
    }
    Some(any)
}

fn where_condition(entity: &EntityAdmin, spec: &ListSpec) -> Condition {
    let mut all = Condition::all().add(spec.scope.clone());
    if let Some(search) = search_condition(entity, &spec.search) {
        all = all.add(search);
    }
    for condition in &spec.conditions {
        all = all.add(condition.clone());
    }
    all
}

fn computed_sql(entity: &EntityAdmin, name: &str) -> Option<Expr> {
    match &entity.computed_column(name)?.kind {
        Computed::Sql(sql) => Some(Expr::cust(format!("({sql})"))),
        _ => None,
    }
}

fn select_columns(entity: &EntityAdmin, backend: DbBackend, query: &mut SelectStatement) {
    let table = entity.backend.table();
    for column in entity.backend.columns() {
        query.expr_as(column_expr(table, &column.name), Alias::new(&column.name));
    }
    for computed in &entity.computed {
        if let Some(expr) = computed_sql(entity, &computed.name) {
            // SQLite reports no type for expression columns, so their values are read as text.
            let expr = if backend == DbBackend::Sqlite {
                expr.cast_as(Alias::new("text"))
            } else {
                expr
            };
            query.expr_as(expr, Alias::new(&computed.name));
        }
    }
    query.from(Alias::new(table));
}

async fn rows(db: &DatabaseConnection, query: &SelectStatement) -> Result<Vec<Row>, DbErr> {
    let statement = db.get_database_backend().build(query);
    let values = JsonValue::find_by_statement(statement).all(db).await?;
    Ok(values
        .into_iter()
        .filter_map(|v| match v {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .collect())
}

pub(crate) async fn fetch(
    db: &DatabaseConnection,
    entity: &EntityAdmin,
    spec: &ListSpec,
) -> Result<Vec<Row>, DbErr> {
    let mut query = Query::select();
    select_columns(entity, db.get_database_backend(), &mut query);
    apply_list_spec(entity, spec, &mut query);
    rows(db, &query).await
}

/// Export only explicitly configured columns, without computing unrelated display values.
pub(crate) async fn fetch_export(
    db: &DatabaseConnection,
    entity: &EntityAdmin,
    spec: &ListSpec,
) -> Result<Vec<Row>, DbErr> {
    let table = entity.backend.table();
    let mut query = Query::select();
    for name in &entity.export_fields {
        query.expr_as(column_expr(table, name), Alias::new(name));
    }
    query.from(Alias::new(table));
    apply_list_spec(entity, spec, &mut query);
    rows(db, &query).await
}

fn apply_list_spec(entity: &EntityAdmin, spec: &ListSpec, query: &mut SelectStatement) {
    let table = entity.backend.table();
    query.cond_where(where_condition(entity, spec));
    for (name, desc) in &spec.order {
        let order = if *desc { Order::Desc } else { Order::Asc };
        if entity.column(name).is_some() {
            query.order_by_expr(column_expr(table, name), order);
        } else if let Some(expr) = computed_sql(entity, name) {
            query.order_by_expr(expr, order);
        }
    }
    for pk in entity.backend.primary_key() {
        if !spec.order.iter().any(|(name, _)| name == pk) {
            query.order_by_expr(column_expr(table, pk), Order::Asc);
        }
    }
    query.limit(spec.limit).offset(spec.offset);
}

pub(crate) async fn count(
    db: &DatabaseConnection,
    entity: &EntityAdmin,
    spec: &ListSpec,
) -> Result<u64, DbErr> {
    let mut query = Query::select();
    query
        .expr(Expr::cust("COUNT(*)"))
        .from(Alias::new(entity.backend.table()))
        .cond_where(where_condition(entity, spec));
    let Some(row) = db.query_one(&query).await? else {
        return Ok(0);
    };
    let total: i64 = row.try_get_by_index(0)?;
    Ok(u64::try_from(total).unwrap_or(0))
}

/// The condition selecting one row by its URL primary key.
pub(crate) fn pk_condition(entity: &EntityAdmin, id: &str) -> Option<Expr> {
    let pk = entity.single_pk()?;
    eq_condition(entity.backend.table(), pk, id).ok()
}

pub(crate) async fn get(
    db: &DatabaseConnection,
    entity: &EntityAdmin,
    id: &str,
    scope: Condition,
) -> Result<Option<Row>, DbErr> {
    let Some(condition) = pk_condition(entity, id) else {
        return Ok(None);
    };
    let spec = ListSpec {
        scope,
        search: String::new(),
        conditions: vec![condition],
        order: Vec::new(),
        limit: 1,
        offset: 0,
    };
    Ok(fetch(db, entity, &spec).await?.into_iter().next())
}

/// Authorized keys from the displayed rows; never loads model or computed columns.
pub(crate) async fn permitted_ids(
    db: &DatabaseConnection,
    entity: &EntityAdmin,
    ids: &[String],
    scope: Condition,
) -> Result<HashSet<String>, DbErr> {
    let mut allowed = HashSet::new();
    let Some(pk) = entity.single_pk() else {
        return Ok(allowed);
    };
    // Keep large configured pages below backend bind-parameter limits.
    for batch in ids.chunks(500) {
        let mut query = Query::select();
        query
            .expr_as(
                column_expr(entity.backend.table(), &pk.name),
                Alias::new(&pk.name),
            )
            .from(Alias::new(entity.backend.table()))
            .cond_where(scope.clone())
            .and_where(in_condition(entity.backend.table(), pk, batch));
        allowed.extend(
            rows(db, &query)
                .await?
                .iter()
                .filter_map(|row| row.get(&pk.name))
                .map(key_string),
        );
    }
    Ok(allowed)
}

/// Resolve a selected export ID without loading non-exported columns or computed values.
pub(crate) async fn export_id(
    db: &DatabaseConnection,
    entity: &EntityAdmin,
    id: &str,
    scope: Condition,
) -> Result<Option<String>, DbErr> {
    let Some(pk) = entity.single_pk() else {
        return Ok(None);
    };
    let Some(condition) = pk_condition(entity, id) else {
        return Ok(None);
    };
    let mut query = Query::select();
    query
        .expr_as(
            column_expr(entity.backend.table(), &pk.name),
            Alias::new(&pk.name),
        )
        .from(Alias::new(entity.backend.table()))
        .cond_where(scope)
        .and_where(condition)
        .limit(1);
    Ok(rows(db, &query)
        .await?
        .first()
        .and_then(|row| row.get(&pk.name))
        .map(key_string))
}

/// Distinct non-null values of a column, sorted.
pub(crate) async fn distinct_values(
    db: &DatabaseConnection,
    entity: &EntityAdmin,
    column: &str,
    limit: u64,
    scope: Condition,
) -> Result<Vec<Value>, DbErr> {
    let table = entity.backend.table();
    let mut query = Query::select();
    query
        .distinct()
        .cond_where(scope)
        .expr_as(column_expr(table, column), Alias::new("v"))
        .from(Alias::new(table))
        .and_where(column_expr(table, column).is_not_null())
        .order_by_expr(Expr::col(Alias::new("v")), Order::Asc)
        .limit(limit);
    Ok(rows(db, &query)
        .await?
        .into_iter()
        .filter_map(|mut row| row.remove("v"))
        .collect())
}

fn label_query(target: &EntityAdmin) -> Option<(SelectStatement, String)> {
    let pk = target.single_pk()?;
    let table = target.backend.table();
    let display = target.display_column_name()?.to_string();
    let mut query = Query::select();
    query
        .expr_as(column_expr(table, &pk.name), Alias::new("k"))
        .expr_as(column_expr(table, &display), Alias::new("l"))
        .from(Alias::new(table));
    Some((query, display))
}

fn label_pairs(rows: Vec<Row>) -> Vec<(String, String)> {
    rows.into_iter()
        .map(|row| {
            let key = row.get("k").map(key_string).unwrap_or_default();
            let label = row
                .get("l")
                .map(key_string)
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| key.clone());
            (key, label)
        })
        .collect()
}

/// Display labels of `target` rows keyed by primary key.
pub(crate) async fn labels(
    db: &DatabaseConnection,
    target: &EntityAdmin,
    keys: &[String],
    scope: Condition,
) -> Result<HashMap<String, String>, DbErr> {
    let (Some((mut query, _)), Some(pk)) = (label_query(target), target.single_pk()) else {
        return Ok(HashMap::new());
    };
    if keys.is_empty() {
        return Ok(HashMap::new());
    }
    query.cond_where(scope);
    query.and_where(in_condition(target.backend.table(), pk, keys));
    Ok(label_pairs(rows(db, &query).await?).into_iter().collect())
}

/// `(primary key, label)` pairs of `target`, optionally narrowed by a search term.
pub(crate) async fn options(
    db: &DatabaseConnection,
    target: &EntityAdmin,
    search: Option<&str>,
    limit: u64,
    scope: Condition,
) -> Result<Vec<(String, String)>, DbErr> {
    let Some((mut query, display)) = label_query(target) else {
        return Ok(Vec::new());
    };
    query.cond_where(scope);
    let table = target.backend.table();
    if let Some(term) = search.map(str::trim).filter(|t| !t.is_empty()) {
        let mut any =
            Condition::any().add(text_lower(column_expr(table, &display)).like(like_pattern(term)));
        if let Some(pk) = target.single_pk() {
            any = any.add(text_lower(column_expr(table, &pk.name)).like(like_pattern(term)));
        }
        if let Some(search) = search_condition(target, term) {
            any = any.add(search);
        }
        query.cond_where(any);
    }
    query
        .order_by_expr(column_expr(table, &display), Order::Asc)
        .limit(limit);
    Ok(label_pairs(rows(db, &query).await?))
}

/// Bounded select options, always including the current or submitted relation.
pub(crate) async fn form_options(
    db: &DatabaseConnection,
    target: &EntityAdmin,
    selected: &str,
    limit: u64,
    scope: Condition,
) -> Result<Vec<(String, String)>, DbErr> {
    let mut choices = options(db, target, None, limit, scope.clone()).await?;
    if selected.is_empty() || choices.iter().any(|(key, _)| key == selected) {
        return Ok(choices);
    }
    let keys = [selected.to_owned()];
    if let Some(label) = labels(db, target, &keys, scope).await?.remove(selected) {
        choices.push((selected.to_owned(), label));
    }
    Ok(choices)
}
