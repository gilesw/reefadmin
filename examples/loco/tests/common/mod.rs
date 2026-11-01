#![allow(dead_code)]
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt;
use loco_rs::app::AppContext;
use reefadmin_loco_example::{
    app, auth,
    models::{categories, products, users},
};
use sea_orm::{ActiveModelTrait, Set};
use tower::ServiceExt;
pub const PASSWORD: &str = "test-only-inventory-password";
pub const SECRET: &str = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234";
pub const ORIGIN: &str = "http://127.0.0.1:39128";
pub struct Fixture {
    pub router: Router,
    pub ctx: AppContext,
    pub alice: users::Model,
    pub bob: users::Model,
    pub member: users::Model,
    pub first: products::Model,
    pub second: products::Model,
    pub other: products::Model,
}
pub async fn fixture() -> Fixture {
    fixture_at("sqlite::memory:").await
}
pub async fn fixture_at(url: &str) -> Fixture {
    let boot = app::boot(url, SECRET, 39128).await.unwrap();
    let db = &boot.app_context.db;
    let alice = auth::create_user(db, "alice@example.test", PASSWORD, true)
        .await
        .unwrap();
    let bob = auth::create_user(db, "bob@example.test", PASSWORD, true)
        .await
        .unwrap();
    let member = auth::create_user(db, "member@example.test", PASSWORD, false)
        .await
        .unwrap();
    let category = categories::ActiveModel {
        owner_id: Set(alice.id),
        name: Set("Alice equipment".into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
    let other_category = categories::ActiveModel {
        owner_id: Set(bob.id),
        name: Set("Bob equipment".into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
    let first = products::ActiveModel {
        owner_id: Set(alice.id),
        category_id: Set(category.id),
        name: Set("Alice compass".into()),
        active: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
    let second = products::ActiveModel {
        owner_id: Set(alice.id),
        category_id: Set(category.id),
        name: Set("Alice lamp".into()),
        active: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
    let other = products::ActiveModel {
        owner_id: Set(bob.id),
        category_id: Set(other_category.id),
        name: Set("Bob radio".into()),
        active: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
    Fixture {
        router: boot.router.unwrap(),
        ctx: boot.app_context,
        alice,
        bob,
        member,
        first,
        second,
        other,
    }
}
pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}
pub async fn send(app: &Router, request: Request<Body>) -> Reply {
    let response = app.clone().oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    Reply {
        status: parts.status,
        headers: parts.headers,
        body: String::from_utf8(body.collect().await.unwrap().to_bytes().to_vec()).unwrap(),
    }
}
pub async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: &str,
    enhanced: bool,
) -> Reply {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("origin", ORIGIN);
    if let Some(cookie) = cookie {
        req = req.header("cookie", cookie);
    }
    if enhanced {
        req = req.header("HX-Request", "true");
    }
    send(app, req.body(Body::from(body.to_owned())).unwrap()).await
}
pub struct Session {
    pub cookie: String,
    pub csrf: String,
}
pub async fn attempt_login(app: &Router, email: &str, password: &str) -> Reply {
    let page = request(app, "GET", "/login", None, "", false).await;
    let csrf = page
        .body
        .split("name=\"_csrf\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let cookie = page.headers["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    request(
        app,
        "POST",
        "/login",
        Some(cookie),
        &format!("email={email}&password={password}&_csrf={csrf}"),
        false,
    )
    .await
}
pub async fn login(app: &Router, email: &str) -> Session {
    let reply = attempt_login(app, email, PASSWORD).await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);
    let cookie = reply
        .headers
        .get_all("set-cookie")
        .iter()
        .map(|value| value.to_str().unwrap())
        .find(|value| value.starts_with("inventory_session="))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let account = request(app, "GET", "/account", Some(&cookie), "", false).await;
    assert_eq!(account.status, StatusCode::OK, "{}", account.body);
    let csrf = account
        .body
        .split("name=\"_csrf\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    Session { cookie, csrf }
}
pub async fn write(
    f: &Fixture,
    session: &Session,
    path: &str,
    body: &str,
    enhanced: bool,
) -> Reply {
    request(
        &f.router,
        "POST",
        path,
        Some(&session.cookie),
        &format!("{body}&_csrf={}", session.csrf),
        enhanced,
    )
    .await
}
