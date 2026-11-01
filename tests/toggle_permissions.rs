mod common;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use common::{Reply, send_raw};
use loco_admin::{AccessEntity, AccessPolicy, AdminActor, AdminApp, EntityAdmin, Operation, Row};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, Database, DatabaseConnection,
    EntityTrait, Schema, Set,
};
use serde_json::json;
use std::sync::{Arc, Mutex};

mod flag {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "flags")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        pub tenant: i64,
        pub label: String,
        pub enabled: bool,
        pub featured: bool,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

struct Policy;
impl AccessPolicy for Policy {
    fn can_enter(&self, _: &AdminActor) -> bool {
        true
    }
    fn scope(
        &self,
        actor: &AdminActor,
        _: AccessEntity<'_>,
        operation: &Operation,
    ) -> Option<Condition> {
        let condition = match operation {
            Operation::View => flag::Column::Tenant.eq(actor.id.parse::<i64>().ok()?),
            Operation::Update if actor.claims["readonly"] != true => flag::Column::Id.ne(3),
            Operation::Toggle(field) if field == "enabled" => flag::Column::Id.ne(2),
            Operation::Toggle(field)
                if field == "featured" && actor.claims["hide_featured"] != true =>
            {
                flag::Column::Id.ne(1)
            }
            _ => return None,
        };
        Some(Condition::all().add(condition))
    }
    fn can_write(
        &self,
        actor: &AdminActor,
        _: AccessEntity<'_>,
        _: &Operation,
        _: Option<&Row>,
        _: &Row,
    ) -> bool {
        actor.claims["deny_write"] != true
    }
}

async fn database(count: i64) -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    db.execute(&Schema::new(db.get_database_backend()).create_table_from_entity(flag::Entity))
        .await
        .unwrap();
    for id in 1..=count {
        flag::ActiveModel {
            id: Set(id),
            tenant: Set(1),
            label: Set(format!("Flag {id}")),
            enabled: Set(false),
            featured: Set(false),
        }
        .insert(&db)
        .await
        .unwrap();
    }
    db
}

fn registration() -> EntityAdmin {
    EntityAdmin::from_entity::<flag::Entity>("flags")
        .list_display(["label", "enabled", "featured"])
        .search_fields(["label"])
        .toggles(["enabled", "featured"])
        .per_page(1000)
}

fn app(db: &mut DatabaseConnection, entity: EntityAdmin) -> (Router, Arc<Mutex<Vec<String>>>) {
    let queries = Arc::new(Mutex::new(Vec::new()));
    let captured = queries.clone();
    db.set_metric_callback(move |info| captured.lock().unwrap().push(info.statement.sql.clone()));
    let app = AdminApp::new(db.clone())
        .access_policy(Policy)
        .register(entity)
        .into_router()
        .unwrap();
    (Router::new().nest("/admin", app), queries)
}

async fn request(app: &Router, path: &str, actor: AdminActor, partial: bool, write: bool) -> Reply {
    let mut request = if write {
        Request::post(path)
    } else {
        Request::get(path)
    }
    .body(Body::empty())
    .unwrap();
    request.extensions_mut().insert(actor);
    if partial {
        request
            .headers_mut()
            .insert("hx-request", "true".parse().unwrap());
    }
    send_raw(app, request).await
}

#[tokio::test]
async fn toggle_query_count_depends_on_fields_and_batches_not_row_count() {
    for (count, expected_queries) in [(1, 4), (50, 4), (501, 6)] {
        let mut db = database(count).await;
        let (app, queries) = app(&mut db, registration());
        for partial in [false, true] {
            queries.lock().unwrap().clear();
            let reply = request(&app, "/admin/flags", AdminActor::new("1"), partial, false).await;
            assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
            let queries = queries.lock().unwrap();
            // Two list queries (count/results), then one key lookup per field per 500 rows.
            assert_eq!(queries.len(), expected_queries, "{count} rows: {queries:?}");
            for sql in &queries[2..] {
                assert!(
                    sql.starts_with("SELECT \"flags\".\"id\" AS \"id\" FROM"),
                    "{sql}"
                );
                assert!(
                    !sql.contains("label"),
                    "permission queries must not load display values: {sql}"
                );
            }
        }
    }
}

#[tokio::test]
async fn unavailable_or_undisplayed_toggles_do_not_query_permissions() {
    let mut db = database(5).await;
    for (entity, actor, path, expected_queries) in [
        (
            registration().can_edit(false),
            AdminActor::new("1"),
            "/admin/flags",
            2,
        ),
        (
            registration().list_display(["label"]),
            AdminActor::new("1"),
            "/admin/flags",
            2,
        ),
        (
            registration(),
            AdminActor::new("1").with_claims(json!({"readonly": true})),
            "/admin/flags",
            2,
        ),
        (
            registration(),
            AdminActor::new("1").with_claims(json!({"hide_featured": true})),
            "/admin/flags",
            3,
        ),
        (
            registration(),
            AdminActor::new("1"),
            "/admin/flags?q=absent",
            2,
        ),
    ] {
        let (app, queries) = app(&mut db, entity);
        let reply = request(&app, path, actor, true, false).await;
        assert_eq!(reply.status, StatusCode::OK);
        let queries = queries.lock().unwrap();
        assert_eq!(queries.len(), expected_queries, "{queries:?}");
        if expected_queries == 2 {
            assert!(!reply.body.contains("/toggle/"));
        } else {
            assert!(!reply.body.contains("/toggle/featured"));
        }
    }
}

#[tokio::test]
async fn buttons_respect_view_update_and_field_scopes_without_cross_request_caching() {
    let mut db = database(4).await;
    flag::ActiveModel {
        id: Set(4),
        tenant: Set(2),
        ..Default::default()
    }
    .update(&db)
    .await
    .unwrap();
    let (app, _) = app(&mut db, registration().per_page(2));
    for partial in [false, true] {
        let page = request(&app, "/admin/flags", AdminActor::new("1"), partial, false).await;
        assert!(page.body.contains("/flags/1/toggle/enabled"));
        assert!(!page.body.contains("/flags/1/toggle/featured"));
        assert!(!page.body.contains("/flags/2/toggle/enabled"));
        assert!(page.body.contains("/flags/2/toggle/featured"));
        assert!(!page.body.contains("Flag 4"));
        let second = request(
            &app,
            "/admin/flags?p=2",
            AdminActor::new("1"),
            partial,
            false,
        )
        .await;
        assert!(second.body.contains("Flag 3"));
        assert!(!second.body.contains("/toggle/"));
        let other = request(&app, "/admin/flags", AdminActor::new("2"), partial, false).await;
        assert!(other.body.contains("/flags/4/toggle/enabled"));
        assert!(!other.body.contains("Flag 1"));
    }
    // Changing ownership after rendering must invalidate the next write.
    flag::ActiveModel {
        id: Set(1),
        tenant: Set(2),
        ..Default::default()
    }
    .update(&db)
    .await
    .unwrap();
    for path in [
        "/admin/flags/1/toggle/enabled",
        "/admin/flags/2/toggle/enabled",
        "/admin/flags/3/toggle/featured",
        "/admin/flags/4/toggle/enabled",
    ] {
        assert_eq!(
            request(&app, path, AdminActor::new("1"), true, true)
                .await
                .status,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        request(
            &app,
            "/admin/flags/2/toggle/featured",
            AdminActor::new("1").with_claims(json!({"deny_write": true})),
            true,
            true
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert!(
        !flag::Entity::find_by_id(2)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .featured
    );
    let toggled = request(
        &app,
        "/admin/flags/2/toggle/featured",
        AdminActor::new("1"),
        true,
        true,
    )
    .await;
    assert_eq!(toggled.status, StatusCode::OK, "{}", toggled.body);
    assert!(toggled.body.contains("/flags/2/toggle/featured"));
    assert!(!toggled.body.contains("/flags/2/toggle/enabled"));
    assert!(
        flag::Entity::find_by_id(2)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .featured
    );
}

mod text_flag {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "text_flags")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub key: String,
        pub enabled: bool,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

#[tokio::test]
async fn string_keys_remain_bound_and_permission_results_preserve_their_values() {
    let mut db = Database::connect("sqlite::memory:").await.unwrap();
    db.execute(&Schema::new(db.get_database_backend()).create_table_from_entity(text_flag::Entity))
        .await
        .unwrap();
    for key in ["café", "x' OR 1=1 --"] {
        text_flag::ActiveModel {
            key: Set(key.into()),
            enabled: Set(false),
        }
        .insert(&db)
        .await
        .unwrap();
    }
    let queries = Arc::new(Mutex::new(Vec::new()));
    let captured = queries.clone();
    db.set_metric_callback(move |info| captured.lock().unwrap().push(info.statement.sql.clone()));
    let app = Router::new().nest(
        "/admin",
        AdminApp::new(db.clone())
            .access_policy(common::TestPolicy)
            .register(
                EntityAdmin::from_entity::<text_flag::Entity>("text-flags").toggles(["enabled"]),
            )
            .into_router()
            .unwrap(),
    );
    let page = request(&app, "/admin/text-flags", AdminActor::new("1"), true, false).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert_eq!(page.body.matches("hx-post=").count(), 2);
    {
        let queries = queries.lock().unwrap();
        assert_eq!(queries.len(), 3, "{queries:?}");
        assert!(!queries[2].contains("OR 1=1"));
    }
    let toggled = request(
        &app,
        "/admin/text-flags/caf%C3%A9/toggle/enabled",
        AdminActor::new("1"),
        true,
        true,
    )
    .await;
    assert_eq!(toggled.status, StatusCode::OK, "{}", toggled.body);
    assert!(
        text_flag::Entity::find_by_id("café")
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .enabled
    );
    assert!(
        !text_flag::Entity::find_by_id("x' OR 1=1 --")
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .enabled
    );
}
