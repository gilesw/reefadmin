//! The inventory application's login and session handling, independent of ReefAdmin.
use crate::models::{sessions, users};
use axum::{
    extract::{Form, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, Key, SameSite, SignedCookieJar};
use chrono::Utc;
use loco_rs::{app::AppContext, hash};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::Deserialize;

pub const SESSION_COOKIE: &str = "inventory_session";
pub type WebError = (StatusCode, &'static str);
pub struct Identity {
    pub user: users::Model,
    pub session: sessions::Model,
}

pub fn internal(error: impl std::fmt::Display) -> WebError {
    tracing::error!(%error, "Inventory request failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "The request could not be completed.",
    )
}
fn session_key(ctx: &AppContext) -> Result<Key, WebError> {
    let secret = ctx
        .config
        .settings
        .as_ref()
        .and_then(|s| s["session_secret"].as_str())
        .ok_or_else(|| internal("Missing session configuration"))?;
    Key::try_from(secret.as_bytes()).map_err(internal)
}
pub async fn identity(ctx: &AppContext, headers: &HeaderMap) -> Result<Option<Identity>, WebError> {
    let jar = SignedCookieJar::from_headers(headers, session_key(ctx)?);
    let Some(cookie) = jar.get(SESSION_COOKIE) else {
        return Ok(None);
    };
    let Some(session) = sessions::Entity::find_by_id(cookie.value())
        .one(&ctx.db)
        .await
        .map_err(internal)?
    else {
        return Ok(None);
    };
    if session.expires_at <= Utc::now().timestamp() {
        return Ok(None);
    }
    let Some(user) = users::Entity::find_by_id(session.user_id)
        .one(&ctx.db)
        .await
        .map_err(internal)?
    else {
        return Ok(None);
    };
    Ok(Some(Identity { user, session }))
}
pub fn unauthenticated(headers: &HeaderMap) -> Response {
    if headers
        .get("HX-Request")
        .is_some_and(|value| value == "true")
    {
        (
            StatusCode::UNAUTHORIZED,
            [("HX-Redirect", "/login"), ("Cache-Control", "no-store")],
            "Sign in again.",
        )
            .into_response()
    } else {
        Redirect::to("/login").into_response()
    }
}
fn private_html(body: String) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], Html(body)).into_response()
}
pub async fn login_page(State(ctx): State<AppContext>) -> Result<Response, WebError> {
    let csrf = uuid::Uuid::new_v4().to_string();
    let cookie = Cookie::build(("inventory_login_csrf", csrf.clone()))
        .path("/login")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(ctx.config.server.host.starts_with("https://"))
        .max_age(time::Duration::minutes(10))
        .build();
    let body = format!(
        r#"<!doctype html><html lang="en"><title>Inventory sign in</title><h1>Sign in to inventory</h1><form method="post" action="/login"><input type="hidden" name="_csrf" value="{csrf}"><label>Email <input type="email" name="email" required autocomplete="username"></label><label>Password <input type="password" name="password" required autocomplete="current-password"></label><button>Sign in</button></form></html>"#
    );
    Ok((
        SignedCookieJar::new(session_key(&ctx)?).add(cookie),
        private_html(body),
    )
        .into_response())
}
#[derive(Deserialize)]
pub struct Login {
    #[serde(default)]
    _csrf: String,
    email: String,
    password: String,
}
pub async fn login(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Form(input): Form<Login>,
) -> Result<Response, WebError> {
    // Reject cross-site login submissions, including missing/null Origin.
    if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok())
        != Some(ctx.config.server.full_url().as_str())
    {
        return Err((StatusCode::FORBIDDEN, "Invalid request origin."));
    }
    let jar = SignedCookieJar::from_headers(&headers, session_key(&ctx)?);
    if !jar
        .get("inventory_login_csrf")
        .is_some_and(|cookie| cookie.value() == input._csrf)
    {
        return Err((StatusCode::FORBIDDEN, "Invalid CSRF token."));
    }
    let user = users::Entity::find()
        .filter(users::Column::Email.eq(input.email))
        .one(&ctx.db)
        .await
        .map_err(internal)?;
    let Some(user) = user else {
        return Err((StatusCode::UNAUTHORIZED, "Invalid email or password."));
    };
    let password_hash = user.password_hash.clone();
    let verified =
        tokio::task::spawn_blocking(move || hash::verify_password(&input.password, &password_hash))
            .await
            .map_err(internal)?;
    if !verified {
        return Err((StatusCode::UNAUTHORIZED, "Invalid email or password."));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let csrf = uuid::Uuid::new_v4().to_string();
    sessions::ActiveModel {
        id: Set(id.clone()),
        user_id: Set(user.id),
        csrf: Set(csrf),
        expires_at: Set(Utc::now().timestamp() + 3600),
    }
    .insert(&ctx.db)
    .await
    .map_err(internal)?;
    let cookie = Cookie::build((SESSION_COOKIE, id))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(ctx.config.server.host.starts_with("https://"))
        .build();
    Ok((
        SignedCookieJar::new(session_key(&ctx)?)
            .add(cookie)
            .remove(Cookie::build("inventory_login_csrf").path("/login").build()),
        [(header::CACHE_CONTROL, "no-store")],
        Redirect::to("/account"),
    )
        .into_response())
}
pub async fn account(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> Result<Response, WebError> {
    let Some(identity) = identity(&ctx, &headers).await? else {
        return Ok(unauthenticated(&headers));
    };
    // CSRF tokens are UUIDs generated by this application, never user-supplied markup.
    Ok(private_html(format!(
        r#"<!doctype html><html lang="en"><title>Inventory account</title><h1>Inventory account</h1><a href="/staff">Administration</a><form method="post" action="/logout"><input type="hidden" name="_csrf" value="{}"><button>Sign out</button></form></html>"#,
        identity.session.csrf
    )))
}
#[derive(Deserialize)]
pub struct Logout {
    #[serde(default)]
    _csrf: String,
}
pub async fn logout(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Form(input): Form<Logout>,
) -> Result<Response, WebError> {
    let Some(identity) = identity(&ctx, &headers).await? else {
        return Ok(unauthenticated(&headers));
    };
    if input._csrf != identity.session.csrf {
        return Err((StatusCode::FORBIDDEN, "Invalid CSRF token."));
    }
    sessions::Entity::delete_by_id(identity.session.id)
        .exec(&ctx.db)
        .await
        .map_err(internal)?;
    Ok((
        SignedCookieJar::new(session_key(&ctx)?)
            .remove(Cookie::build(SESSION_COOKIE).path("/").build()),
        Redirect::to("/login"),
    )
        .into_response())
}
/// Explicit provisioning for local setup and test fixtures; never an HTTP route.
pub async fn create_user(
    db: &sea_orm::DatabaseConnection,
    email: &str,
    password: &str,
    staff: bool,
) -> loco_rs::Result<users::Model> {
    if password.len() < 12 {
        return Err(loco_rs::Error::Message(
            "Use a password of at least 12 characters.".into(),
        ));
    }
    let password = password.to_owned();
    let password_hash = tokio::task::spawn_blocking(move || hash::hash_password(&password))
        .await
        .map_err(|e| loco_rs::Error::Message(e.to_string()))??;
    Ok(users::ActiveModel {
        email: Set(email.to_owned()),
        password_hash: Set(password_hash),
        is_staff: Set(staff),
        ..Default::default()
    }
    .insert(db)
    .await?)
}
