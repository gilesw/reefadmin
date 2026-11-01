use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
mod common;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use common::{Reply, account, database, rule, send_raw};
use loco_admin::{AccessEntity, AccessPolicy, AdminActor, AdminApp, Operation, Row};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter};
use serde_json::json;

struct OwnerPolicy;
impl AccessPolicy for OwnerPolicy {
    fn can_enter(&self, actor: &AdminActor) -> bool {
        actor.claims["staff"] == true
    }
    fn scope(
        &self,
        actor: &AdminActor,
        entity: AccessEntity<'_>,
        op: &Operation,
    ) -> Option<Condition> {
        if actor.claims["readonly"] == true
            && !matches!(
                op,
                Operation::View | Operation::Autocomplete | Operation::Export
            )
        {
            return None;
        }
        let owner = actor.id.parse::<i64>().ok()?;
        let scope = match entity.name {
            "accounts" => Condition::all().add(account::Column::Id.eq(owner)),
            "rules" => Condition::all().add(rule::Column::AccountId.eq(owner)),
            "exchanges" => return Some(Condition::all()),
            _ => return None,
        };
        // Row 2 is visible but cannot be edited; toggles must respect this too.
        Some(if entity.name == "rules" && *op == Operation::Update {
            scope.add(rule::Column::Id.ne(2))
        } else {
            scope
        })
    }
    fn can_write(
        &self,
        actor: &AdminActor,
        entity: AccessEntity<'_>,
        _: &Operation,
        _: Option<&Row>,
        proposed: &Row,
    ) -> bool {
        entity.name != "rules"
            || proposed
                .get("account_id")
                .and_then(|v| v.as_i64())
                .map(|id| id.to_string())
                == Some(actor.id.clone())
    }
}
fn actor(id: &str) -> AdminActor {
    AdminActor::new(id).with_claims(json!({"staff": true}))
}
async fn request(
    app: &Router,
    actor: Option<AdminActor>,
    path: &str,
    body: Option<&str>,
    partial: bool,
) -> Reply {
    let mut req = if let Some(body) = body {
        Request::post(path)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body.to_owned()))
            .unwrap()
    } else {
        Request::get(path).body(Body::empty()).unwrap()
    };
    if let Some(actor) = actor {
        req.extensions_mut().insert(actor);
    }
    if partial {
        req.headers_mut()
            .insert("hx-request", "true".parse().unwrap());
    }
    send_raw(app, req).await
}
async fn fixture() -> (Router, sea_orm::DatabaseConnection) {
    let db = database().await;
    let app = common::admin(&db, "/admin")
        .access_policy(OwnerPolicy)
        .into_router()
        .unwrap();
    (Router::new().nest("/admin", app), db)
}

#[tokio::test]
async fn identity_and_policy_are_required_and_default_to_denial() {
    struct Deny;
    impl AccessPolicy for Deny {}
    let db = database().await;
    assert!(AdminApp::new(db.clone()).into_router().is_err());
    let app = AdminApp::new(db).access_policy(Deny).into_router().unwrap();
    for path in ["/", "/_static/admin.css", "/accounts"] {
        assert_eq!(
            request(&app, None, path, None, false).await.status,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            request(&app, Some(actor("")), path, None, false)
                .await
                .status,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            request(&app, Some(actor("1")), path, None, false)
                .await
                .status,
            StatusCode::FORBIDDEN
        );
    }
    let (app, _) = fixture().await;
    assert_eq!(
        request(&app, Some(AdminActor::new("1")), "/admin", None, false)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn scopes_filter_lists_counts_filters_autocomplete_and_exports() {
    let (app, _) = fixture().await;
    for partial in [false, true] {
        let page = request(&app, Some(actor("1")), "/admin/accounts", None, partial).await;
        assert_eq!(page.status, StatusCode::OK);
        assert!(page.body.contains("alice"));
        assert!(page.text().contains("1 result"));
        assert!(!page.body.contains("bob"));
        assert!(!page.body.contains("value=\"premium\""));
        let options = request(
            &app,
            Some(actor("1")),
            "/admin/accounts/autocomplete",
            None,
            partial,
        )
        .await;
        assert!(options.body.contains("alice"));
        assert!(!options.body.contains("bob"));
    }
    let csv = request(
        &app,
        Some(actor("1")),
        "/admin/accounts",
        Some("action=__csv__"),
        false,
    )
    .await;
    assert_eq!(csv.status, StatusCode::OK);
    assert!(csv.body.contains("alice"));
    assert!(!csv.body.contains("bob"));
    let nav = request(&app, Some(actor("1")), "/admin", None, false).await;
    assert!(!nav.body.contains("locked-exchanges"));
    assert_eq!(
        request(
            &app,
            Some(actor("1")),
            "/admin/locked-exchanges",
            None,
            false
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let bob = request(&app, Some(actor("2")), "/admin/accounts", None, false).await;
    assert!(bob.body.contains("bob"));
    assert!(!bob.body.contains("alice"));
}

#[tokio::test]
async fn direct_requests_and_mixed_bulk_selections_cannot_escape_scope() {
    let (app, db) = fixture().await;
    let before = account::Entity::find().all(&db).await.unwrap();
    for partial in [false, true] {
        for (path, body) in [
            ("/admin/accounts/2/change", None),
            (
                "/admin/accounts/2/change",
                Some("username=stolen&account_type=free"),
            ),
            ("/admin/accounts/2/delete", None),
            ("/admin/accounts/2/delete", Some("")),
            ("/admin/accounts/2/toggle/is_staff", Some("")),
            (
                "/admin/accounts",
                Some("action=__delete__&selected=1&selected=2"),
            ),
            (
                "/admin/accounts",
                Some("action=__csv__&selected=1&selected=2"),
            ),
            (
                "/admin/accounts",
                Some("action=make_premium&selected=1&selected=2"),
            ),
            ("/admin/accounts", Some("action=make_premium&selected=bad")),
            (
                "/admin/accounts",
                Some("action=__delete__&selected=1&selected=999"),
            ),
        ] {
            let reply = request(&app, Some(actor("1")), path, body, partial).await;
            assert_eq!(
                reply.status,
                StatusCode::NOT_FOUND,
                "{path}: {}",
                reply.body
            );
        }
    }
    assert_eq!(account::Entity::find().all(&db).await.unwrap(), before);
    let own = request(
        &app,
        Some(actor("1")),
        "/admin/accounts",
        Some("action=make_premium&selected=1&selected=01"),
        false,
    )
    .await;
    assert_eq!(own.status, StatusCode::OK);
    assert_eq!(
        account::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .account_type,
        "premium"
    );
}

#[tokio::test]
async fn readonly_and_per_record_update_permissions_control_forms_and_toggles() {
    let (app, db) = fixture().await;
    let readonly = AdminActor::new("1").with_claims(json!({"staff": true, "readonly": true}));
    let form = request(
        &app,
        Some(readonly.clone()),
        "/admin/accounts/1/change",
        None,
        false,
    )
    .await;
    assert_eq!(form.status, StatusCode::OK);
    assert!(!form.body.contains("name=\"_save\""));
    for (path, body) in [
        ("/admin/accounts/add", None),
        (
            "/admin/accounts/1/change",
            Some("username=no&account_type=free"),
        ),
        ("/admin/accounts/1/delete", Some("")),
        ("/admin/accounts/1/toggle/is_staff", Some("")),
        ("/admin/accounts", Some("action=make_premium&selected=1")),
    ] {
        assert_eq!(
            request(&app, Some(readonly.clone()), path, body, false)
                .await
                .status,
            StatusCode::FORBIDDEN
        );
    }
    let before = rule::Entity::find_by_id(2).one(&db).await.unwrap().unwrap();
    // Register a toggle to exercise the intersection of Toggle and Update scopes.
    let router = AdminApp::new(db.clone())
        .access_policy(OwnerPolicy)
        .register(loco_admin::EntityAdmin::from_entity::<rule::Entity>("rules").toggles(["active"]))
        .into_router()
        .unwrap();
    assert_eq!(
        request(
            &router,
            Some(actor("1")),
            "/rules/2/toggle/active",
            Some(""),
            true
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        rule::Entity::find_by_id(2).one(&db).await.unwrap().unwrap(),
        before
    );
}

#[tokio::test]
async fn proposed_ownership_changes_and_new_rows_are_checked() {
    let (app, db) = fixture().await;
    let before = rule::Entity::find().all(&db).await.unwrap();
    for path in ["/admin/rules/1/change", "/admin/rules/add"] {
        let reply = request(
            &app,
            Some(actor("1")),
            path,
            Some("account_id=2&name=stolen&priority=1&active=true&exchange_mic=XLON"),
            false,
        )
        .await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{}", reply.body);
    }
    assert_eq!(rule::Entity::find().all(&db).await.unwrap(), before);
    let reply = request(
        &app,
        Some(actor("1")),
        "/admin/rules/add",
        Some("account_id=1&name=mine&priority=1&active=true&exchange_mic=XLON"),
        false,
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);
    assert_eq!(
        rule::Entity::find()
            .filter(rule::Column::Name.eq("mine"))
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .account_id,
        1
    );
}

#[derive(Clone)]
struct CountingObserver(Arc<AtomicUsize>);
#[async_trait::async_trait]
impl loco_admin::MutationObserver for CountingObserver {
    async fn before(
        &self,
        _: &sea_orm::DatabaseTransaction,
        _: &str,
        _: &serde_json::Value,
    ) -> Result<serde_json::Value, sea_orm::DbErr> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(serde_json::Value::Null)
    }
    async fn after(
        &self,
        _: &sea_orm::DatabaseTransaction,
        _: &str,
        _: &serde_json::Value,
        _: serde_json::Value,
    ) -> Result<(), sea_orm::DbErr> {
        Ok(())
    }
}

#[tokio::test]
async fn denied_relations_and_writes_do_not_invoke_observers() {
    struct Restricted;
    impl AccessPolicy for Restricted {
        fn can_enter(&self, _: &AdminActor) -> bool {
            true
        }
        fn scope(
            &self,
            _: &AdminActor,
            entity: AccessEntity<'_>,
            _: &Operation,
        ) -> Option<Condition> {
            Some(if entity.name == "accounts" {
                Condition::all().add(account::Column::Id.eq(1))
            } else {
                Condition::all()
            })
        }
        fn can_write(
            &self,
            _: &AdminActor,
            _: AccessEntity<'_>,
            _: &Operation,
            _: Option<&Row>,
            row: &Row,
        ) -> bool {
            row["name"] != "denied"
        }
    }
    let db = database().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let app = common::admin(&db, "/admin")
        .access_policy(Restricted)
        .observe_mutations(CountingObserver(calls.clone()))
        .into_router()
        .unwrap();
    let app = Router::new().nest("/admin", app);
    let before = rule::Entity::find().all(&db).await.unwrap();
    for body in [
        "account_id=2&name=unauthorized&priority=1&active=true",
        "account_id=1&name=denied&priority=1&active=true",
    ] {
        assert_eq!(
            request(
                &app,
                Some(actor("1")),
                "/admin/rules/1/change",
                Some(body),
                false
            )
            .await
            .status,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &app,
                Some(actor("1")),
                "/admin/rules/add",
                Some(body),
                false
            )
            .await
            .status,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(rule::Entity::find().all(&db).await.unwrap(), before);
    // Existing hidden references survive unrelated edits without exposing their labels.
    let form = request(&app, Some(actor("1")), "/admin/rules/3/change", None, false).await;
    assert!(!form.body.contains(">bob<"));
    assert!(form.body.contains("value=\"2\" selected"), "{}", form.body);
    assert!(!form.body.contains("/accounts/2/change"));
    let saved = request(
        &app,
        Some(actor("1")),
        "/admin/rules/3/change",
        Some("account_id=2&name=renamed&priority=3&active=true&exchange_mic=XAMS"),
        false,
    )
    .await;
    assert_eq!(saved.status, StatusCode::SEE_OTHER, "{}", saved.body);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn concurrent_actors_and_inline_rows_stay_separate() {
    let (app, _) = fixture().await;
    let (alice, bob) = tokio::join!(
        request(
            &app,
            Some(actor("1")),
            "/admin/accounts/1/change",
            None,
            false
        ),
        request(
            &app,
            Some(actor("2")),
            "/admin/accounts/2/change",
            None,
            false
        ),
    );
    assert!(alice.body.contains("Alice low"));
    assert!(!alice.body.contains("Bob middle"));
    assert!(bob.body.contains("Bob middle"));
    assert!(!bob.body.contains("Alice low"));
}

#[tokio::test]
async fn unspecified_operations_and_write_predicates_deny_access() {
    struct EntryOnly;
    impl AccessPolicy for EntryOnly {
        fn can_enter(&self, _: &AdminActor) -> bool {
            true
        }
    }
    struct ScopedOnly;
    impl AccessPolicy for ScopedOnly {
        fn can_enter(&self, _: &AdminActor) -> bool {
            true
        }
        fn scope(&self, _: &AdminActor, _: AccessEntity<'_>, _: &Operation) -> Option<Condition> {
            Some(Condition::all())
        }
    }
    let db = database().await;
    let app = common::admin(&db, "/admin")
        .access_policy(EntryOnly)
        .into_router()
        .unwrap();
    assert_eq!(
        request(&app, Some(actor("1")), "/accounts", None, false)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let app = common::admin(&db, "/admin")
        .access_policy(ScopedOnly)
        .into_router()
        .unwrap();
    let before = account::Entity::find().all(&db).await.unwrap();
    assert_eq!(
        request(
            &app,
            Some(actor("1")),
            "/accounts/1/toggle/is_staff",
            Some(""),
            false
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(account::Entity::find().all(&db).await.unwrap(), before);
}
