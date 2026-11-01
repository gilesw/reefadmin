use crate::auth;
use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use loco_rs::app::AppContext;
use reefadmin::{AdminActor, CsrfToken};

pub async fn protect(State(ctx): State<AppContext>, mut request: Request, next: Next) -> Response {
    let identity = match auth::identity(&ctx, request.headers()).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return auth::unauthenticated(request.headers()),
        Err(error) => return error.into_response(),
    };
    if !identity.user.is_staff {
        return (StatusCode::FORBIDDEN, "Staff access required.").into_response();
    }
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        let (parts, body) = request.into_parts();
        let bytes = match to_bytes(body, 1024 * 1024).await {
            Ok(bytes) => bytes,
            Err(_) => {
                return (StatusCode::PAYLOAD_TOO_LARGE, "Request is too large.").into_response();
            }
        };
        let tokens: Vec<_> = form_urlencoded::parse(&bytes)
            .filter(|(key, _)| key == "_csrf")
            .map(|(_, value)| value.into_owned())
            .collect();
        if tokens.as_slice() != [identity.session.csrf.as_str()] {
            return (StatusCode::FORBIDDEN, "Invalid CSRF token.").into_response();
        }
        request = Request::from_parts(parts, Body::from(bytes));
    }
    request.extensions_mut().insert(
        AdminActor::new(identity.user.id.to_string())
            .with_claims(serde_json::json!({"staff": true})),
    );
    request.extensions_mut().insert(CsrfToken {
        field_name: "_csrf".into(),
        value: identity.session.csrf,
    });
    next.run(request).await
}

/// Host response policy, applied after the initializer has composed all routes.
pub async fn security_headers(request: Request, next: Next) -> Response {
    use axum::http::HeaderValue;
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.remove("x-powered-by");
    headers
        .entry("cache-control")
        .or_insert(HeaderValue::from_static("no-store"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("same-origin"));
    headers.insert("content-security-policy", HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'"));
    response
}
