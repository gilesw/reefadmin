use sea_orm::DbErr;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdminError {
    #[error("Access denied.")]
    AccessDenied,
    /// The admin configuration is invalid.
    #[error("admin configuration error: {0}")]
    Config(String),

    #[error("database error: {0}")]
    Database(#[from] DbErr),

    #[error("template error: {0}")]
    Template(#[from] tera::Error),

    #[error("{0}")]
    Custom(String),
}

/// Host-supplied mapping from a failed add or change write to public form messages.
///
/// `entity` is the registered entity name. Keys are form field names; the key
/// `__all__` renders as a form-level alert, and messages for names that are not
/// on the form are appended to that alert. Return `None` for internal failures;
/// they remain generic HTTP 500 responses.
pub trait ValidationErrorMapper: Send + Sync {
    fn validation_errors(
        &self,
        entity: &str,
        error: &AdminError,
    ) -> Option<HashMap<String, String>>;
}

impl AdminError {
    pub fn custom(message: impl Into<String>) -> Self {
        Self::Custom(message.into())
    }
}
