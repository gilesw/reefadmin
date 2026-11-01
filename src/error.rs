use sea_orm::DbErr;
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

impl AdminError {
    pub fn custom(message: impl Into<String>) -> Self {
        Self::Custom(message.into())
    }
}
