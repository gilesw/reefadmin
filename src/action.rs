use crate::error::AdminError;
use sea_orm::DatabaseConnection;
use std::{future::Future, pin::Pin, str::FromStr, sync::Arc};

pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

type ActionHandler =
    Arc<dyn Fn(ActionContext) -> BoxFuture<Result<ActionResult, AdminError>> + Send + Sync>;

/// What a custom action receives: the actor, database and authorized primary keys.
pub struct ActionContext {
    /// Authenticated actor whose policy authorized the selection.
    pub actor: crate::AdminActor,
    pub db: DatabaseConnection,
    /// Primary keys of the selected rows, as they appear in admin URLs.
    pub ids: Vec<String>,
}

impl ActionContext {
    /// Parses every selected primary key, for example as `i64`.
    pub fn ids_as<T: FromStr>(&self) -> Result<Vec<T>, AdminError> {
        self.ids
            .iter()
            .map(|id| {
                id.parse::<T>()
                    .map_err(|_| AdminError::custom(format!("Invalid ID: {id}")))
            })
            .collect()
    }
}

/// The outcome of a custom action, shown on the list page.
pub enum ActionResult {
    Success(String),
    Error(String),
    /// Redirects to a path inside the admin, relative to its prefix (for example `/rules`).
    Redirect(String),
}

impl ActionResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self::Success(message.into())
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }
}

/// A bulk action offered in the list page's action menu.
#[derive(Clone)]
pub struct Action {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) confirm: Option<String>,
    pub(crate) handler: ActionHandler,
}

impl Action {
    pub fn new<F, Fut>(name: &str, label: &str, handler: F) -> Self
    where
        F: Fn(ActionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ActionResult, AdminError>> + Send + 'static,
    {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            confirm: None,
            handler: Arc::new(move |ctx| Box::pin(handler(ctx))),
        }
    }

    /// Asks the browser to confirm before running the action.
    pub fn confirm(mut self, message: &str) -> Self {
        self.confirm = Some(message.to_string());
        self
    }
}
