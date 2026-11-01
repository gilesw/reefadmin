use crate::{
    access::{RelationAccess, WriteAccess},
    sql,
};
mod cells;
mod form;
mod list;

use crate::{AccessEntity, AdminActor, CsrfToken, EntityAdmin, Operation, Row};
use crate::{app::AdminState, error::AdminError, render::ADMIN_JS};
use axum::{
    Router,
    extract::{FromRequestParts, Path},
    http::{StatusCode, header, request::Parts},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use sea_orm::{Condition, sea_query::Expr};
use serde::Serialize;
use std::{collections::HashMap, ops::Deref, sync::Arc};
use tera::Context;

type AppState = Arc<AdminState>;

/// Trusted host identity and presentation data for this request.
pub(super) struct RequestState {
    state: AppState,
    csrf: Option<CsrfToken>,
    actor: AdminActor,
}

impl Deref for RequestState {
    type Target = AdminState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl RequestState {
    fn base(&self, current: &str) -> Context {
        let mut context = self.state.base(current, &self.actor);
        context.insert("csrf", &self.csrf);
        context
    }
}

impl RequestState {
    fn identity<'a>(&self, entity: &'a EntityAdmin) -> AccessEntity<'a> {
        AccessEntity {
            name: &entity.name,
            table: entity.backend.table(),
        }
    }

    fn scope(&self, entity: &EntityAdmin, operation: &Operation) -> Result<Condition, Failure> {
        let identity = self.identity(entity);
        let view = self
            .policy
            .scope(&self.actor, identity, &Operation::View)
            .ok_or(Failure::Forbidden("Access denied."))?;
        if *operation == Operation::View {
            return Ok(view);
        }
        let scope = self
            .policy
            .scope(&self.actor, identity, operation)
            .ok_or(Failure::Forbidden("Access denied."))?;
        let mut scope = Condition::all().add(view).add(scope);
        if matches!(operation, Operation::Toggle(_)) {
            scope = scope.add(
                self.policy
                    .scope(&self.actor, identity, &Operation::Update)
                    .ok_or(Failure::Forbidden("Access denied."))?,
            );
        }
        Ok(scope)
    }

    fn read_scope(&self, entity: &EntityAdmin) -> Condition {
        self.scope(entity, &Operation::View)
            .unwrap_or_else(|_| Condition::all().add(Expr::val(false)))
    }

    fn write_access<'a>(
        &'a self,
        entity: &'a EntityAdmin,
        operation: Operation,
    ) -> Result<WriteAccess<'a>, Failure> {
        let scope = self.scope(entity, &operation)?;
        let relations = entity
            .options
            .iter()
            .filter_map(|(column, options)| {
                let target = self.entity(options.foreign_key.as_deref()?)?;
                Some(RelationAccess {
                    column: column.clone(),
                    table: target.backend.table().into(),
                    key: target.single_pk()?.clone(),
                    scope: self.read_scope(target),
                })
            })
            .collect();
        Ok(WriteAccess {
            actor: &self.actor,
            policy: self.policy.as_ref(),
            entity: self.identity(entity),
            operation,
            scope,
            relations,
        })
    }

    async fn get(
        &self,
        entity: &EntityAdmin,
        id: &str,
        operation: &Operation,
    ) -> Result<Row, Failure> {
        sql::get(&self.db, entity, id, self.scope(entity, operation)?)
            .await?
            .ok_or(Failure::NotFound)
    }

    async fn permits_row(
        &self,
        entity: &EntityAdmin,
        id: &str,
        operation: &Operation,
    ) -> Result<bool, AdminError> {
        let Ok(scope) = self.scope(entity, operation) else {
            return Ok(false);
        };
        Ok(sql::get(&self.db, entity, id, scope).await?.is_some())
    }
}

impl FromRequestParts<AppState> for RequestState {
    type Rejection = Failure;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let actor = parts
            .extensions
            .get::<AdminActor>()
            .cloned()
            .filter(|actor| !actor.id.is_empty())
            .ok_or(Failure::Unauthorized)?;
        if !state.policy.can_enter(&actor) {
            return Err(Failure::Forbidden("Access denied."));
        }
        Ok(Self {
            actor,
            state: state.clone(),
            csrf: parts.extensions.get::<CsrfToken>().cloned(),
        })
    }
}

pub(crate) fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/_static/{*file}", get(asset))
        .route("/{entity}", get(list::list_page).post(list::bulk_action))
        .route("/{entity}/autocomplete", get(list::autocomplete))
        .route("/{entity}/add", get(form::add_page).post(form::add_submit))
        .route(
            "/{entity}/{id}/change",
            get(form::change_page).post(form::change_submit),
        )
        .route(
            "/{entity}/{id}/delete",
            get(form::delete_page).post(form::delete_submit),
        )
        .route("/{entity}/{id}/toggle/{field}", post(list::toggle))
        .with_state(state)
}

/// A failed request, rendered as a short page.
pub(crate) enum Failure {
    NotFound,
    Unauthorized,
    Forbidden(&'static str),
    BadRequest(String),
    Internal(AdminError),
}

impl From<AdminError> for Failure {
    fn from(err: AdminError) -> Self {
        match err {
            AdminError::AccessDenied => Self::Forbidden("Access denied."),
            err => Self::Internal(err),
        }
    }
}

impl From<sea_orm::DbErr> for Failure {
    fn from(err: sea_orm::DbErr) -> Self {
        Self::Internal(err.into())
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Authentication required.".to_owned(),
            ),
            Self::NotFound => (StatusCode::NOT_FOUND, "Not found.".to_string()),
            Self::Forbidden(message) => (StatusCode::FORBIDDEN, message.to_string()),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::Internal(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };
        (status, [(header::CACHE_CONTROL, "no-store")], message).into_response()
    }
}

pub(crate) type PageResult = Result<Response, Failure>;

/// A message shown at the top of a page.
#[derive(Serialize)]
pub(crate) struct Flash {
    pub level: &'static str,
    pub text: String,
}

impl Flash {
    pub(crate) fn success(text: impl Into<String>) -> Self {
        Self {
            level: "success",
            text: text.into(),
        }
    }

    pub(crate) fn error(text: impl Into<String>) -> Self {
        Self {
            level: "danger",
            text: text.into(),
        }
    }
}

pub(crate) fn html(status: StatusCode, body: String) -> Response {
    (status, [(header::CACHE_CONTROL, "no-store")], Html(body)).into_response()
}

/// Parses an urlencoded body; for repeated keys the last value wins.
pub(crate) fn form_map(body: &[u8]) -> HashMap<String, String> {
    form_urlencoded::parse(body).into_owned().collect()
}

pub(crate) fn form_values(body: &[u8], key: &str) -> Vec<String> {
    form_urlencoded::parse(body)
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
        .collect()
}

async fn index(state: RequestState) -> PageResult {
    let body = state.renderer.render("index.html", state.base(""))?;
    Ok(html(StatusCode::OK, body))
}

async fn asset(_state: RequestState, Path(file): Path<String>) -> Response {
    let (content, content_type, cache) = match file.as_str() {
        "admin.css" => (
            include_str!("../../static/admin.css"),
            "text/css; charset=utf-8",
            "no-cache",
        ),
        "admin.js" => (ADMIN_JS, "text/javascript; charset=utf-8", "no-cache"),
        "vendor/bootstrap-5.3.8/bootstrap.min.css" => (
            include_str!("../../static/vendor/bootstrap-5.3.8/bootstrap.min.css"),
            "text/css; charset=utf-8",
            "public, max-age=31536000, immutable",
        ),
        "vendor/bootstrap-5.3.8/bootstrap.bundle.min.js" => (
            include_str!("../../static/vendor/bootstrap-5.3.8/bootstrap.bundle.min.js"),
            "text/javascript; charset=utf-8",
            "public, max-age=31536000, immutable",
        ),
        "vendor/htmx-2.0.11/htmx.min.js" => (
            include_str!("../../static/vendor/htmx-2.0.11/htmx.min.js"),
            "text/javascript; charset=utf-8",
            "public, max-age=31536000, immutable",
        ),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        content,
    )
        .into_response()
}
