//! A server-rendered admin for Axum and SeaORM, with Loco integration.
//!
//! Mount the router behind your application’s session and CSRF middleware.
//! Supply a trusted [`AdminActor`] and an [`AccessPolicy`] for permission checks.

mod access;
mod action;
mod app;
mod backend;
mod entity;
mod error;
mod render;
mod router;
mod sql;
mod urls;
mod validator;
mod values;

pub use access::{AccessEntity, AccessPolicy, AdminActor, Operation};

pub use action::{Action, ActionContext, ActionResult};
pub use app::AdminApp;
pub use entity::{EntityAdmin, Inline, Row};
pub use error::AdminError;
pub use error::ValidationErrorMapper;
pub use validator::{
    EmailFormat, MaxLength, MaxValue, MinLength, MinValue, RegexValidator, Required, Validator,
};

pub use backend::MutationObserver;

/// A host-issued CSRF token to include in admin forms.
///
/// Insert this into request extensions after resolving the session. The host must
/// validate submitted tokens; this type neither issues tokens nor grants access.
/// Choose a field name that does not collide with entity fields or admin parameters.
#[derive(Clone, serde::Serialize)]
pub struct CsrfToken {
    pub field_name: String,
    pub value: String,
}
