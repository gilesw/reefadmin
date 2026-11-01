//! Application-owned identity and policy decisions, enforced by admin routes.
use crate::{AdminError, Row, backend::ColumnMeta};
use sea_orm::sea_query::Condition;
use serde_json::Value;

/// Trusted identity supplied by host session middleware through request extensions.
/// Never construct this from unverified request parameters or headers.
#[derive(Clone, Debug)]
pub struct AdminActor {
    pub id: String,
    pub claims: Value,
}

impl AdminActor {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            claims: Value::Null,
        }
    }

    pub fn with_claims(mut self, claims: Value) -> Self {
        self.claims = claims;
        self
    }
}

/// Registered entity identity. Use `table` to qualify SQL scope columns.
#[derive(Clone, Copy, Debug)]
pub struct AccessEntity<'a> {
    pub name: &'a str,
    pub table: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    View,
    Create,
    Update,
    Delete,
    Export,
    Autocomplete,
    Toggle(String),
    Action(String),
}

/// Defines permissions for the authenticated actor. All default methods deny access.
///
/// A scope is a SQL condition that selects permitted rows. The admin applies scopes
/// before counting, limiting or returning rows. Each operation must also satisfy
/// [`Operation::View`]. Toggles must additionally satisfy [`Operation::Update`].
/// Write checks run inside the transaction before observers and model hooks.
pub trait AccessPolicy: Send + Sync {
    /// Returns whether the actor can enter the admin.
    fn can_enter(&self, _actor: &AdminActor) -> bool {
        false
    }

    /// Returns the permitted row scope, or `None` to deny the operation.
    fn scope(
        &self,
        _actor: &AdminActor,
        _entity: AccessEntity<'_>,
        _operation: &Operation,
    ) -> Option<Condition> {
        None
    }

    /// Checks proposed values keyed by database column name.
    ///
    /// Updates supply the current row and a complete proposed row. Inserts have no
    /// current row and supply submitted values before model defaults and hooks run.
    /// Deletes supply the current row as both `current` and `proposed`.
    fn can_write(
        &self,
        _actor: &AdminActor,
        _entity: AccessEntity<'_>,
        _operation: &Operation,
        _current: Option<&Row>,
        _proposed: &Row,
    ) -> bool {
        false
    }
}

pub(crate) fn denied() -> AdminError {
    AdminError::AccessDenied
}

pub(crate) struct WriteAccess<'a> {
    pub actor: &'a AdminActor,
    pub policy: &'a dyn AccessPolicy,
    pub entity: AccessEntity<'a>,
    pub operation: Operation,
    pub scope: Condition,
    pub relations: Vec<RelationAccess>,
}

pub(crate) struct RelationAccess {
    pub column: String,
    pub table: String,
    pub key: ColumnMeta,
    pub scope: Condition,
}

impl WriteAccess<'_> {
    pub async fn check<C: sea_orm::ConnectionTrait>(
        &self,
        db: &C,
        current: Option<&Row>,
        proposed: &Row,
    ) -> Result<(), AdminError> {
        use crate::values::{eq_condition, key_string};
        use sea_orm::sea_query::{Alias, Expr, Query};
        if !self
            .policy
            .can_write(self.actor, self.entity, &self.operation, current, proposed)
        {
            return Err(denied());
        }
        if self.operation == Operation::Delete {
            return Ok(());
        }
        for relation in &self.relations {
            let Some(value) = proposed.get(&relation.column) else {
                continue;
            };
            if value.is_null() || Some(value) == current.and_then(|row| row.get(&relation.column)) {
                continue;
            }
            let id = key_string(value);
            let condition =
                eq_condition(&relation.table, &relation.key, &id).map_err(|_| denied())?;
            let query = Query::select()
                .expr(Expr::val(1))
                .from(Alias::new(&relation.table))
                .cond_where(relation.scope.clone())
                .and_where(condition)
                .to_owned();
            if db.query_one(&query).await?.is_none() {
                return Err(denied());
            }
        }
        Ok(())
    }
}
