mod common;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::*;
use reefadmin_loco_example::{
    domain,
    models::{products, sessions, users},
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};

#[tokio::test]
async fn loco_boot_mounts_protected_routes_and_keeps_host_routes() {
    let f = fixture().await;
    assert_eq!(
        request(&f.router, "GET", "/health", None, "", false)
            .await
            .body,
        "ok"
    );
    for path in [
        "/staff",
        "/staff/products",
        "/staff/products/1/change",
        "/staff/categories/autocomplete",
        "/staff/_static/admin.js",
    ] {
        let response = request(&f.router, "GET", path, None, "", false).await;
        assert_eq!(response.status, StatusCode::SEE_OTHER, "{path}");
        assert_eq!(response.headers["location"], "/login");
        let enhanced = request(&f.router, "GET", path, None, "", true).await;
        assert_eq!(enhanced.status, StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(enhanced.headers["HX-Redirect"], "/login");
        assert!(!enhanced.headers.contains_key("X-ReefAdmin-Fragment"));
    }
    let forged = send(
        &f.router,
        Request::get("/staff/products")
            .header("x-user-id", f.alice.id.to_string())
            .header("x-staff", "true")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(forged.status, StatusCode::SEE_OTHER);
    let member = login(&f.router, &f.member.email).await;
    assert_eq!(
        request(&f.router, "GET", "/staff", Some(&member.cookie), "", false)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let alice = login(&f.router, &f.alice.email).await;
    assert_eq!(
        request(
            &f.router,
            "GET",
            "/staff/_static/admin.js",
            Some(&alice.cookie),
            "",
            false
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &f.router,
            "GET",
            "/staff/users",
            Some(&alice.cookie),
            "",
            false
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn login_requires_valid_credentials_and_same_origin() {
    let f = fixture().await;
    for origin in [None, Some("http://other.example"), Some("null")] {
        let mut req =
            Request::post("/login").header("content-type", "application/x-www-form-urlencoded");
        if let Some(origin) = origin {
            req = req.header("origin", origin);
        }
        assert_eq!(
            send(
                &f.router,
                req.body(Body::from(format!(
                    "email={}&password={PASSWORD}",
                    f.alice.email
                )))
                .unwrap()
            )
            .await
            .status,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        attempt_login(&f.router, &f.alice.email, "wrong")
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let response = attempt_login(&f.router, &f.alice.email, PASSWORD).await;
    let cookie = response
        .headers
        .get_all("set-cookie")
        .iter()
        .map(|value| value.to_str().unwrap())
        .find(|value| value.starts_with("inventory_session="))
        .unwrap();
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    assert_eq!(
        request(
            &f.router,
            "GET",
            "/staff",
            Some("inventory_session=forged"),
            "",
            false
        )
        .await
        .status,
        StatusCode::SEE_OTHER
    );
}

#[tokio::test]
async fn scopes_cover_pages_autocomplete_exports_and_direct_ids() {
    let f = fixture().await;
    let alice = login(&f.router, &f.alice.email).await;
    let bob = login(&f.router, &f.bob.email).await;
    for enhanced in [false, true] {
        for (session, visible, hidden) in [
            (&alice, "Alice compass", "Bob radio"),
            (&bob, "Bob radio", "Alice compass"),
        ] {
            let reply = request(
                &f.router,
                "GET",
                "/staff/products",
                Some(&session.cookie),
                "",
                enhanced,
            )
            .await;
            assert_eq!(reply.status, StatusCode::OK);
            assert!(reply.body.contains(visible));
            assert!(!reply.body.contains(hidden));
        }
    }
    let autocomplete = request(
        &f.router,
        "GET",
        "/staff/categories/autocomplete?q=equipment",
        Some(&alice.cookie),
        "",
        true,
    )
    .await;
    assert!(autocomplete.body.contains("Alice equipment"));
    assert!(!autocomplete.body.contains("Bob equipment"));
    assert_eq!(
        request(
            &f.router,
            "GET",
            &format!("/staff/products/{}/change", f.other.id),
            Some(&alice.cookie),
            "",
            false
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    let csv = write(&f, &alice, "/staff/products", "action=__csv__", false).await;
    assert_eq!(csv.status, StatusCode::OK, "{}", csv.body);
    assert!(csv.body.contains("Alice compass"));
    assert!(!csv.body.contains("Bob radio"));
    assert!(!csv.body.contains("owner_id"));
}

#[tokio::test]
async fn csrf_and_ownership_protect_edits_toggles_and_actions() {
    let f = fixture().await;
    let alice = login(&f.router, &f.alice.email).await;
    let bob = login(&f.router, &f.bob.email).await;
    let change = format!("/staff/products/{}/change", f.first.id);
    for token in [
        "".to_string(),
        "&_csrf=wrong".into(),
        format!("&_csrf={}", bob.csrf),
        format!("&_csrf={}&_csrf=wrong", alice.csrf),
    ] {
        assert_eq!(
            request(
                &f.router,
                "POST",
                &change,
                Some(&alice.cookie),
                &format!("name=Forged{token}"),
                false
            )
            .await
            .status,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        products::Entity::find_by_id(f.first.id)
            .one(&f.ctx.db)
            .await
            .unwrap()
            .unwrap(),
        f.first
    );
    let edit = write(
        &f,
        &alice,
        &change,
        &format!(
            "name=Edited&category_id={}&active=true&owner_id={}",
            f.first.category_id, f.bob.id
        ),
        false,
    )
    .await;
    assert_eq!(edit.status, StatusCode::SEE_OTHER, "{}", edit.body);
    let row = products::Entity::find_by_id(f.first.id)
        .one(&f.ctx.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.owner_id, f.alice.id);
    assert_eq!(row.name, "Edited");
    let relation = write(
        &f,
        &alice,
        &change,
        &format!("name=Forged&category_id={}", f.other.category_id),
        false,
    )
    .await;
    assert!(relation.status.is_client_error());
    assert_eq!(
        products::Entity::find_by_id(f.first.id)
            .one(&f.ctx.db)
            .await
            .unwrap()
            .unwrap(),
        row
    );
    assert_eq!(
        write(
            &f,
            &alice,
            &format!("/staff/products/{}/toggle/active", f.other.id),
            "",
            true
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    let toggled = write(
        &f,
        &alice,
        &format!("/staff/products/{}/toggle/active", f.first.id),
        "",
        true,
    )
    .await;
    assert_eq!(toggled.status, StatusCode::OK);
    assert_eq!(toggled.headers["X-ReefAdmin-Fragment"], "true");
    let mixed = write(
        &f,
        &alice,
        "/staff/products",
        &format!(
            "action=archive&selected={}&selected={}",
            f.second.id, f.other.id
        ),
        false,
    )
    .await;
    assert!(mixed.status.is_client_error());
    assert!(
        products::Entity::find_by_id(f.second.id)
            .one(&f.ctx.db)
            .await
            .unwrap()
            .unwrap()
            .active
    );
    assert_eq!(
        write(&f, &alice, "/staff/products/add", "name=Forbidden", false)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn domain_action_rechecks_access_and_rolls_back_the_selection() {
    let f = fixture().await;
    let alice = login(&f.router, &f.alice.email).await;
    let mut second = f.second.clone().into_active_model();
    second.active = Set(false);
    second.update(&f.ctx.db).await.unwrap();
    let failed = write(
        &f,
        &alice,
        "/staff/products",
        &format!(
            "action=archive&selected={}&selected={}",
            f.first.id, f.second.id
        ),
        false,
    )
    .await;
    assert_eq!(failed.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!failed.body.contains("already archived"));
    assert!(
        products::Entity::find_by_id(f.first.id)
            .one(&f.ctx.db)
            .await
            .unwrap()
            .unwrap()
            .active
    );
    let actor = reefadmin::AdminActor::new(f.alice.id.to_string());
    assert!(
        domain::archive(&f.ctx.db, &actor, &[f.other.id])
            .await
            .is_err()
    );
    let ok = write(
        &f,
        &alice,
        "/staff/products",
        &format!("action=archive&selected={}", f.first.id),
        false,
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK);
    assert!(
        !products::Entity::find_by_id(f.first.id)
            .one(&f.ctx.db)
            .await
            .unwrap()
            .unwrap()
            .active
    );
    let mut user = f.alice.clone().into_active_model();
    user.is_staff = Set(false);
    user.update(&f.ctx.db).await.unwrap();
    assert!(
        domain::archive(&f.ctx.db, &actor, &[f.first.id])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn revoked_expired_sessions_and_role_changes_take_effect() {
    let f = fixture().await;
    let alice = login(&f.router, &f.alice.email).await;
    assert_eq!(
        write(&f, &alice, "/logout", "", false).await.status,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        request(
            &f.router,
            "GET",
            "/staff/products",
            Some(&alice.cookie),
            "",
            true
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    let alice = login(&f.router, &f.alice.email).await;
    sessions::Entity::update_many()
        .col_expr(
            sessions::Column::ExpiresAt,
            sea_orm::sea_query::Expr::value(0i64),
        )
        .filter(sessions::Column::UserId.eq(f.alice.id))
        .exec(&f.ctx.db)
        .await
        .unwrap();
    assert_eq!(
        request(&f.router, "GET", "/staff", Some(&alice.cookie), "", false)
            .await
            .status,
        StatusCode::SEE_OTHER
    );
    let alice = login(&f.router, &f.alice.email).await;
    users::Entity::update_many()
        .col_expr(
            users::Column::IsStaff,
            sea_orm::sea_query::Expr::value(false),
        )
        .filter(users::Column::Id.eq(f.alice.id))
        .exec(&f.ctx.db)
        .await
        .unwrap();
    assert_eq!(
        request(&f.router, "GET", "/staff", Some(&alice.cookie), "", false)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}
