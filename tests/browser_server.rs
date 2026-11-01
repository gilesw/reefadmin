mod common;

use axum::{Router, middleware, routing::get};
use tokio::net::TcpListener;

/// A loopback-only fixture with disposable data and test session/CSRF middleware.
#[tokio::test]
#[ignore = "long-running fixture started by Playwright"]
async fn browser_server() {
    let db = common::database().await;
    let admin = common::admin(&db, "/staff")
        .extra_stylesheet("/theme.css")
        .configure_templates(|tera| {
            tera.register_filter("shout", |value: &str, _: tera::Kwargs, _: &tera::State| value.to_uppercase());
        })
        .template("index.html", "{% extends \"layout.html\" %}{% block content %}<h1>{{ admin_title | shout }}</h1>{% endblock %}")
        .into_router().unwrap()
        .layer(middleware::from_fn(common::protect));
    let app = Router::new()
        .nest("/staff", admin)
        .route("/health", get(|| async { "ready" }))
        .route(
            "/theme.css",
            get(|| async {
                (
                    [("content-type", "text/css")],
                    ":root { --admin-cell-space: .4rem; }",
                )
            }),
        );
    let listener = TcpListener::bind("127.0.0.1:39127").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
