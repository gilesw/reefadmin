mod common;
use axum::http::StatusCode;
use common::*;
use reefadmin_loco_example::models::products;
use sea_orm::{ConnectionTrait, Database, EntityTrait};

#[tokio::test]
#[ignore = "requires a disposable REEFADMIN_TEST_DATABASE_URL"]
async fn loco_postgres_boot_login_and_protected_domain_write() {
    let url = std::env::var("REEFADMIN_TEST_DATABASE_URL")
        .expect("Set a disposable PostgreSQL database URL");
    let control = Database::connect(&url).await.unwrap();
    let schema = format!("reefadmin_loco_{}", uuid::Uuid::new_v4().simple());
    control
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let options = form_urlencoded::Serializer::new(String::new())
        .append_pair("options", &format!("-c search_path={schema}"))
        .finish();
    let separator = if url.contains('?') { '&' } else { '?' };
    let scoped_url = format!("{url}{separator}{options}");
    let outcome = tokio::spawn(async move {
        let f = fixture_at(&scoped_url).await;
        let alice = login(&f.router, &f.alice.email).await;
        let path = format!("/staff/products/{}/change", f.first.id);
        let denied = request(
            &f.router,
            "POST",
            &path,
            Some(&alice.cookie),
            "name=Forged",
            false,
        )
        .await;
        assert_eq!(denied.status, StatusCode::FORBIDDEN);
        let edit = write(
            &f,
            &alice,
            &path,
            &format!(
                "name=Postgres&category_id={}&active=true",
                f.first.category_id
            ),
            false,
        )
        .await;
        assert_eq!(edit.status, StatusCode::SEE_OTHER, "{}", edit.body);
        let archived = write(
            &f,
            &alice,
            "/staff/products",
            &format!("action=archive&selected={}", f.first.id),
            false,
        )
        .await;
        assert_eq!(archived.status, StatusCode::OK, "{}", archived.body);
        assert!(
            !products::Entity::find_by_id(f.first.id)
                .one(&f.ctx.db)
                .await
                .unwrap()
                .unwrap()
                .active
        );
        assert!(
            products::Entity::find_by_id(f.other.id)
                .one(&f.ctx.db)
                .await
                .unwrap()
                .unwrap()
                .active
        );
        f.ctx.db.close().await.unwrap();
    })
    .await;
    control
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    if let Err(error) = outcome {
        std::panic::resume_unwind(error.into_panic());
    }
}
