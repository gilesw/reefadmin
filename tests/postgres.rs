//! Run explicitly with REEFADMIN_TEST_DATABASE_URL pointing at a test database.
mod common;

use axum::{Router, http::StatusCode};
use common::{get, post};
use reefadmin::{AdminApp, EntityAdmin, MutationObserver};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbErr,
    EntityTrait, Schema,
};
use serde_json::Value;
use std::{
    env,
    future::Future,
    panic::resume_unwind,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

mod item {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "items")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        #[sea_orm(unique)]
        pub name: String,
        pub tags: Vec<String>,
        pub optional_tags: Option<Vec<String>>,
        pub active: bool,
        pub small: i16,
        pub rating: f32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

struct Reject;

#[async_trait::async_trait]
impl MutationObserver for Reject {
    async fn before(&self, _: &DatabaseTransaction, _: &str, row: &Value) -> Result<Value, DbErr> {
        Ok(row.clone())
    }

    async fn after(
        &self,
        db: &DatabaseTransaction,
        _: &str,
        _: &Value,
        _: Value,
    ) -> Result<(), DbErr> {
        db.execute_unprepared("UPDATE items SET name = 'observer side effect'")
            .await?;
        Err(DbErr::Custom("reject derived update".into()))
    }
}

async fn in_schema<F, Fut>(test: F)
where
    F: FnOnce(DatabaseConnection) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let url = env::var("REEFADMIN_TEST_DATABASE_URL")
        .expect("set REEFADMIN_TEST_DATABASE_URL to a disposable PostgreSQL test database");
    assert!(url.starts_with("postgres://") || url.starts_with("postgresql://"));
    let control = Database::connect(&url).await.unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let schema = format!("reefadmin_{}_{nonce}_{sequence}", process::id());
    control
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    // Run setup and assertions in a task so its panic cannot bypass schema cleanup.
    let mut options = ConnectOptions::new(url);
    options.set_schema_search_path(&schema).max_connections(1);
    let outcome = tokio::spawn(async move {
        let db = Database::connect(options).await.unwrap();
        db.execute(&Schema::new(db.get_database_backend()).create_table_from_entity(item::Entity))
            .await
            .unwrap();
        db.execute(
            &Schema::new(db.get_database_backend())
                .create_table_from_entity(common::exchange::Entity),
        )
        .await
        .unwrap();
        test(db).await;
    })
    .await;
    control
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    control.close().await.unwrap();
    if let Err(error) = outcome {
        resume_unwind(error.into_panic());
    }
}

fn registration() -> EntityAdmin {
    EntityAdmin::from_entity::<item::Entity>("items")
        .fields(["name", "tags", "optional_tags", "active", "small", "rating"])
        .toggles(["active"])
        .list_filter(["small", "rating", "optional_tags"])
}

fn app(db: &DatabaseConnection) -> Router {
    Router::new().nest(
        "/admin",
        AdminApp::new(db.clone())
            .access_policy(common::TestPolicy)
            .register(registration())
            .into_router()
            .unwrap(),
    )
}

const INSERT: &str =
    "name=First&tags=%5B%22fish%22%5D&optional_tags=&active=true&small=7&rating=1.5";

#[tokio::test]
#[ignore = "requires REEFADMIN_TEST_DATABASE_URL"]
async fn arrays_distinguish_null_empty_and_invalid_shapes() {
    in_schema(|db| async move {
        let app = app(&db);
        assert_eq!(
            post(&app, "/admin/items/add", INSERT).await.status,
            StatusCode::SEE_OTHER
        );
        let row = item::Entity::find_by_id(1).one(&db).await.unwrap().unwrap();
        assert_eq!(row.tags, ["fish"]);
        assert_eq!(row.optional_tags, None);
        let saved = post(
            &app,
            "/admin/items/1/change",
            "name=First&tags=%5B%5D&optional_tags=%5B%5D&active=true&small=7&rating=1.5",
        )
        .await;
        assert_eq!(saved.status, StatusCode::SEE_OTHER, "{}", saved.body);
        let row = item::Entity::find_by_id(1).one(&db).await.unwrap().unwrap();
        assert!(row.tags.is_empty());
        assert_eq!(row.optional_tags, Some(vec![]));
        for tags in ["%7B%7D", "%5Bbroken", "null"] {
            let response = post(
                &app,
                "/admin/items/1/change",
                &format!("name=Bad&tags={tags}&small=7&rating=1.5"),
            )
            .await;
            assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(
                item::Entity::find_by_id(1).one(&db).await.unwrap().unwrap(),
                row
            );
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "requires REEFADMIN_TEST_DATABASE_URL"]
async fn typed_numeric_filters_and_integer_keys_work_on_postgres() {
    in_schema(|db| async move {
        let app = app(&db);
        assert_eq!(
            post(&app, "/admin/items/add", INSERT).await.status,
            StatusCode::SEE_OTHER
        );
        for query in ["f_small=7", "f_rating=1.5", "f_optional_tags=__null__"] {
            let response = get(&app, &format!("/admin/items?{query}")).await;
            assert_eq!(response.status, StatusCode::OK, "{}", response.body);
            assert!(response.body.contains("/admin/items/1/change"));
        }
        assert!(
            !get(&app, "/admin/items?f_small=8")
                .await
                .body
                .contains("/admin/items/1/change")
        );
        assert_eq!(
            get(&app, "/admin/items/1/change").await.status,
            StatusCode::OK
        );
        assert_eq!(
            get(&app, "/admin/items/2147483648/change").await.status,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            post(&app, "/admin/items/1/toggle/active", "").await.status,
            StatusCode::SEE_OTHER
        );
        assert!(
            !item::Entity::find_by_id(1)
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .active
        );
        assert_eq!(
            post(&app, "/admin/items/1/delete", "").await.status,
            StatusCode::SEE_OTHER
        );
        assert!(item::Entity::find().all(&db).await.unwrap().is_empty());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires REEFADMIN_TEST_DATABASE_URL"]
async fn string_keys_and_database_constraint_failures_preserve_data() {
    in_schema(|db| async move {
        let app = Router::new().nest(
            "/admin",
            AdminApp::new(db.clone())
                .access_policy(common::TestPolicy)
                .register(EntityAdmin::from_entity::<common::exchange::Entity>(
                    "exchanges",
                ))
                .register(registration())
                .into_router()
                .unwrap(),
        );
        assert_eq!(
            post(
                &app,
                "/admin/exchanges/add",
                "mic=XTEST&name=Market&country=GB"
            )
            .await
            .status,
            StatusCode::SEE_OTHER
        );
        assert_eq!(
            post(
                &app,
                "/admin/exchanges/XTEST/change",
                "name=Updated&country="
            )
            .await
            .status,
            StatusCode::SEE_OTHER
        );
        let row = common::exchange::Entity::find_by_id("XTEST")
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.name, "Updated");
        assert_eq!(row.country, None);
        assert_eq!(
            post(&app, "/admin/items/add", INSERT).await.status,
            StatusCode::SEE_OTHER
        );
        let before = item::Entity::find().all(&db).await.unwrap();
        let failed = post(&app, "/admin/items/add", INSERT).await;
        assert_eq!(failed.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(failed.body.contains("contact your administrator"));
        assert!(!failed.body.contains("duplicate key"));
        assert_eq!(item::Entity::find().all(&db).await.unwrap(), before);
        assert_eq!(
            post(&app, "/admin/exchanges/XTEST/delete", "").await.status,
            StatusCode::SEE_OTHER
        );
        assert!(
            common::exchange::Entity::find()
                .all(&db)
                .await
                .unwrap()
                .is_empty()
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires REEFADMIN_TEST_DATABASE_URL"]
async fn observer_failures_roll_back_each_write_and_derived_changes() {
    in_schema(|db| async move {
        let app = app(&db);
        assert_eq!(
            post(&app, "/admin/items/add", INSERT).await.status,
            StatusCode::SEE_OTHER
        );
        let before = item::Entity::find().all(&db).await.unwrap();
        let rejecting = Router::new().nest(
            "/admin",
            AdminApp::new(db.clone())
                .access_policy(common::TestPolicy)
                .observe_mutations(Reject)
                .register(registration())
                .into_router()
                .unwrap(),
        );
        for (path, body, expected) in [
            (
                "/admin/items/add",
                "name=Rejected&tags=%5B%5D&small=7&rating=1.5",
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                "/admin/items/1/change",
                "name=Rejected&tags=%5B%5D&small=7&rating=1.5",
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                "/admin/items/1/delete",
                "",
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                "/admin/items/1/toggle/active",
                "",
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                "/admin/items",
                "action=__delete__&selected=1",
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ] {
            let response = post(&rejecting, path, body).await;
            assert_eq!(response.status, expected, "{path}");
            assert!(!response.body.contains("reject derived update"));
            assert!(response.body.contains("contact your administrator"));
            assert_eq!(item::Entity::find().all(&db).await.unwrap(), before);
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL database"]
async fn policy_scopes_recheck_saved_rows_and_roll_back_writes() {
    use reefadmin::{AccessEntity, AccessPolicy, AdminActor, Operation, Row};
    use sea_orm::{ColumnTrait, Condition};
    struct ActiveOnly;
    impl AccessPolicy for ActiveOnly {
        fn can_enter(&self, _: &AdminActor) -> bool {
            true
        }
        fn scope(&self, _: &AdminActor, _: AccessEntity<'_>, _: &Operation) -> Option<Condition> {
            Some(Condition::all().add(item::Column::Active.eq(true)))
        }
        fn can_write(
            &self,
            _: &AdminActor,
            _: AccessEntity<'_>,
            _: &Operation,
            _: Option<&Row>,
            _: &Row,
        ) -> bool {
            true
        }
    }
    in_schema(|db| async move {
        let app = Router::new().nest(
            "/admin",
            AdminApp::new(db.clone())
                .access_policy(ActiveOnly)
                .register(registration())
                .into_router()
                .unwrap(),
        );
        assert_eq!(
            post(&app, "/admin/items/add", INSERT).await.status,
            StatusCode::SEE_OTHER
        );
        assert_eq!(
            post(&app, "/admin/items/1/toggle/active", "").await.status,
            StatusCode::FORBIDDEN
        );
        assert!(
            item::Entity::find_by_id(1)
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .active
        );
        let rejected = INSERT
            .replace("name=First", "name=Hidden")
            .replace("active=true", "active=false");
        assert_eq!(
            post(&app, "/admin/items/add", &rejected).await.status,
            StatusCode::FORBIDDEN
        );
        assert_eq!(item::Entity::find().all(&db).await.unwrap().len(), 1);
        assert_eq!(
            post(&app, "/admin/items/1/delete", "").await.status,
            StatusCode::SEE_OTHER
        );
        assert!(item::Entity::find().all(&db).await.unwrap().is_empty());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires REEFADMIN_TEST_DATABASE_URL"]
async fn exports_project_allowed_columns_and_keep_typed_filters_and_selected_ids() {
    in_schema(|db| async move {
        let app = Router::new().nest(
            "/admin",
            AdminApp::new(db)
                .access_policy(common::TestPolicy)
                .register(registration().export_fields(["name", "tags"]))
                .into_router()
                .unwrap(),
        );
        assert_eq!(
            post(&app, "/admin/items/add", INSERT).await.status,
            StatusCode::SEE_OTHER
        );
        for body in [
            "action=__csv__&q=f_small%3D7",
            "action=__csv__&selected=1&selected=01",
        ] {
            let csv = post(&app, "/admin/items", body).await;
            assert_eq!(csv.status, StatusCode::OK, "{}", csv.body);
            assert_eq!(csv.body, "name,tags\r\nFirst,\"[\"\"fish\"\"]\"\r\n");
        }
        let empty = post(&app, "/admin/items", "action=__csv__&q=f_small%3D8").await;
        assert_eq!(empty.status, StatusCode::OK);
        assert_eq!(empty.body, "name,tags\r\n");
        assert_eq!(
            post(&app, "/admin/items", "action=__csv__&selected=999")
                .await
                .status,
            StatusCode::NOT_FOUND
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires REEFADMIN_TEST_DATABASE_URL"]
async fn toggle_permissions_batch_typed_postgres_keys() {
    use std::sync::{Arc, Mutex};
    in_schema(|mut db| async move {
        let seed = app(&db);
        assert_eq!(
            post(&seed, "/admin/items/add", INSERT).await.status,
            StatusCode::SEE_OTHER
        );
        assert_eq!(
            post(
                &seed,
                "/admin/items/add",
                &INSERT.replace("First", "Second")
            )
            .await
            .status,
            StatusCode::SEE_OTHER
        );
        let queries = Arc::new(Mutex::new(Vec::new()));
        let captured = queries.clone();
        db.set_metric_callback(move |info| {
            captured.lock().unwrap().push(info.statement.sql.clone())
        });
        let app = Router::new().nest(
            "/admin",
            AdminApp::new(db)
                .access_policy(common::TestPolicy)
                .register(
                    EntityAdmin::from_entity::<item::Entity>("items")
                        .list_display(["name", "active"])
                        .toggles(["active"]),
                )
                .into_router()
                .unwrap(),
        );
        let page = get(&app, "/admin/items").await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        assert!(page.body.contains("/items/1/toggle/active"));
        assert!(page.body.contains("/items/2/toggle/active"));
        let queries = queries.lock().unwrap();
        assert_eq!(queries.len(), 3, "{queries:?}");
        assert!(queries[2].starts_with("SELECT \"items\".\"id\" AS \"id\" FROM"));
    })
    .await;
}
