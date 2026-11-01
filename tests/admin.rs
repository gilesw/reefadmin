mod common;

use axum::{Router, http::StatusCode};
use common::{account, admin, app, database, exchange, get, links, post, rule};
use reefadmin::{AdminApp, AdminError, EntityAdmin};
use sea_orm::{EntityTrait, PaginatorTrait};

#[tokio::test]
async fn index_and_assets_use_the_prefix() {
    let (app, _db) = app().await;
    let page = get(&app, "/admin").await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body.contains(r#"href="/admin/accounts""#),
        "{}",
        page.body
    );
    assert!(page.body.contains(r#"href="/admin/accounts/add""#));
    assert!(page.body.contains(r#"src="/admin/_static/admin.js""#));
    assert!(
        page.body
            .contains(r#"href="/admin/_static/vendor/bootstrap-5.3.8/bootstrap.min.css""#)
    );
    assert!(page.text().contains("Markets"));
    assert!(!page.body.contains(r#"rel="icon""#));

    let js = get(&app, "/admin/_static/admin.js").await;
    assert_eq!(js.status, StatusCode::OK);
    assert!(js.body.contains("data-admin-autocomplete"));
}

#[tokio::test]
async fn configured_favicon_is_rendered_and_escaped() {
    let db = database().await;
    let router = admin(&db, "/staff")
        .favicon("/static/icon.svg?theme=light&size=small")
        .into_router()
        .unwrap();
    let app = Router::new().nest("/staff", router);
    let page = get(&app, "/staff").await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains(
        r#"rel="icon" type="image/svg+xml" href="/static/icon.svg?theme=light&amp;size=small""#
    ));
}

#[tokio::test]
async fn unknown_entities_are_not_found() {
    let (app, _db) = app().await;
    assert_eq!(
        get(&app, "/admin/nothing").await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, "/admin/accounts/99/change").await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn list_shows_choices_and_computed_columns() {
    let (app, _db) = app().await;
    let page = get(&app, "/admin/accounts").await;
    assert_eq!(page.status, StatusCode::OK);
    let text = page.text();
    assert!(text.contains("alice Free No Turn on 2 ALICE 1"), "{text}");
    assert!(text.contains("bob Premium No Turn on 1 BOB 1"), "{text}");
    assert!(page.body.contains(r#"href="/admin/accounts/1/change""#));
    assert!(text.contains("2 results"));
}

#[tokio::test]
async fn list_labels_and_links_foreign_keys() {
    let (app, _db) = app().await;
    let page = get(&app, "/admin/rules").await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body
            .contains(r#"<a href="/admin/accounts/1/change">alice</a>"#),
        "{}",
        page.body
    );
    assert!(
        page.body
            .contains(r#"<a href="/admin/exchanges/XLON/change">London Stock Exchange</a>"#)
    );
    let text = page.text();
    // Default ordering is by descending priority.
    let high = text.find("Alice high").unwrap();
    let middle = text.find("Bob middle").unwrap();
    let low = text.find("Alice low").unwrap();
    assert!(high < middle && middle < low, "{text}");
}

#[tokio::test]
async fn search_matches_columns_and_sql_expressions() {
    let (app, _db) = app().await;
    let text = get(&app, "/admin/accounts?q=ALI").await.text();
    assert!(text.contains("alice") && !text.contains("bob"), "{text}");

    let text = get(&app, "/admin/rules?q=bob").await.text();
    assert!(
        text.contains("Bob middle") && !text.contains("Alice low"),
        "{text}"
    );

    let text = get(&app, "/admin/rules?q=%25").await.text();
    assert!(text.contains("0 results"), "{text}");
}

#[tokio::test]
async fn filters_by_choice_boolean_foreign_key_and_empty() {
    let (app, _db) = app().await;
    let page = get(&app, "/admin/accounts?f_account_type=premium").await;
    let text = page.text();
    assert!(text.contains("bob") && !text.contains("ALICE"), "{text}");
    assert!(
        page.body
            .contains(r#"href="/admin/accounts?f_account_type=friend""#)
    );
    assert!(
        page.body
            .contains(r#"href="/admin/accounts?f_account_type=premium&amp;f_is_staff=true""#),
        "{}",
        page.body
    );

    let text = get(&app, "/admin/rules?f_active=false").await.text();
    assert!(
        text.contains("Alice high") && !text.contains("Alice low"),
        "{text}"
    );

    let page = get(&app, "/admin/rules?f_account_id=2").await;
    assert!(page.text().contains("Bob middle") && !page.text().contains("Alice low"));
    // The foreign-key filter lists referenced rows by their label.
    assert!(
        page.body
            .contains(r#"href="/admin/rules?f_account_id=1">alice</a>"#),
        "{}",
        page.body
    );

    let text = get(&app, "/admin/exchanges?f_country=__null__")
        .await
        .text();
    assert!(
        text.contains("Off exchange") && !text.contains("Euronext"),
        "{text}"
    );
    assert!(text.contains("By country All GB NL Empty"), "{text}");
}

#[tokio::test]
async fn sorting_and_pagination() {
    let (app, _db) = app().await;
    let page = get(&app, "/admin/accounts?o=-rule_count").await;
    let text = page.text();
    assert!(text.find("alice").unwrap() < text.find("bob").unwrap());
    let page = get(&app, "/admin/accounts?o=rule_count").await;
    let text = page.text();
    assert!(
        text.find("bob").unwrap() < text.find("ALICE").unwrap(),
        "{text}"
    );
    assert!(
        page.body
            .contains(r#"href="/admin/accounts?o=-rule_count""#)
    );

    let first = get(&app, "/admin/exchanges").await;
    assert!(first.text().contains("XAMS") && !first.text().contains("XOFF"));
    assert!(first.body.contains(r#"href="/admin/exchanges?p=2""#));
    let second = get(&app, "/admin/exchanges?p=2").await;
    assert!(second.text().contains("XOFF") && !second.text().contains("XAMS"));
}

#[tokio::test]
async fn create_with_choices_and_before_save_hook() {
    let (app, db) = app().await;
    let form = get(&app, "/admin/accounts/add").await;
    assert_eq!(form.status, StatusCode::OK);
    assert!(form.body.contains(r#"action="/admin/accounts/add""#));
    assert!(
        form.body
            .contains(r#"<option value="friend">Friend</option>"#)
    );
    assert!(!form.body.contains(r#"name="created_at""#));
    assert!(!form.body.contains(r#"name="id""#));

    let reply = post(
        &app,
        "/admin/accounts/add",
        "username=carol&account_type=friend&is_staff=false&is_staff=true",
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    assert_eq!(reply.location(), "/admin/accounts?notice=added");
    let carol = account::Entity::find_by_id(3)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(carol.username, "carol");
    assert_eq!(carol.account_type, "friend");
    assert!(carol.is_staff);

    let page = get(&app, reply.location()).await;
    assert!(page.text().contains("The account was added."));

    let reply = post(
        &app,
        "/admin/accounts/add",
        "username=dave&account_type=premium&_continue=1",
    )
    .await;
    assert_eq!(reply.location(), "/admin/accounts/4/change?notice=added");
}

#[tokio::test]
async fn create_rejects_invalid_values() {
    let (app, db) = app().await;
    let reply = post(
        &app,
        "/admin/accounts/add",
        "username=eve&account_type=gold",
    )
    .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(reply.text().contains("Choose one of the listed options."));
    assert!(reply.body.contains(r#"value="eve""#));

    let reply = post(
        &app,
        "/admin/accounts/add",
        "username=aaaaaaaaaaaaaaaaaaaaaaaaa&account_type=free",
    )
    .await;
    assert!(reply.text().contains("Must be at most 20 characters."));

    let reply = post(
        &app,
        "/admin/rules/add",
        "account_id=1&name=x&priority=high",
    )
    .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(reply.text().contains("Enter a whole number."));
    assert_eq!(account::Entity::find().count(&db).await.unwrap(), 2);
}

#[tokio::test]
async fn string_primary_keys_create_edit_and_delete() {
    let (app, db) = app().await;
    let reply = post(
        &app,
        "/admin/exchanges/add",
        "mic=X+PAR&name=Paris&country=FR",
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);

    let list = get(&app, "/admin/exchanges?q=paris").await;
    assert!(
        list.body
            .contains(r#"href="/admin/exchanges/X%20PAR/change""#),
        "{}",
        list.body
    );

    let form = get(&app, "/admin/exchanges/X%20PAR/change").await;
    assert_eq!(form.status, StatusCode::OK);
    assert!(form.body.contains(r#"value="Paris""#));
    assert!(
        !form.body.contains(r#"name="mic""#),
        "primary key is read-only on edit"
    );

    let reply = post(
        &app,
        "/admin/exchanges/X%20PAR/change",
        "name=Paris+Bourse&country=",
    )
    .await;
    assert_eq!(reply.location(), "/admin/exchanges?notice=changed");
    let paris = exchange::Entity::find_by_id("X PAR".to_string())
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(paris.name, "Paris Bourse");
    assert_eq!(paris.country, None);

    let confirm = get(&app, "/admin/exchanges/X%20PAR/delete").await;
    assert!(
        confirm
            .text()
            .contains("Are you sure you want to delete the exchange “Paris Bourse”?")
    );
    assert!(
        confirm
            .body
            .contains(r#"action="/admin/exchanges/X%20PAR/delete""#)
    );
    let reply = post(&app, "/admin/exchanges/X%20PAR/delete", "").await;
    assert_eq!(reply.location(), "/admin/exchanges?notice=deleted");
    assert!(
        exchange::Entity::find_by_id("X PAR".to_string())
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn read_only_fields_are_shown_but_not_written() {
    let (app, db) = app().await;
    let before = account::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let form = get(&app, "/admin/accounts/1/change").await;
    assert!(!form.body.contains(r#"name="created_at""#));
    assert!(form.text().contains("Created at"));
    assert!(
        form.text()
            .contains(&before.created_at.format("%Y-%m-%d %H:%M UTC").to_string())
    );
    // Read-only computed columns show their value.
    assert!(form.text().contains("Rules 2"), "{}", form.text());

    let reply = post(
        &app,
        "/admin/accounts/1/change",
        "username=alicia&account_type=free&created_at=2000-01-01T00%3A00&rule_count=9",
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);
    let after = account::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.username, "alicia");
    assert_eq!(after.created_at, before.created_at);
    assert!(!after.is_staff, "an absent checkbox means false");
}

#[tokio::test]
async fn adding_can_be_disabled() {
    let (app, _db) = app().await;
    let list = get(&app, "/admin/locked-exchanges").await;
    assert_eq!(list.status, StatusCode::OK);
    assert!(!list.body.contains("/admin/locked-exchanges/add"));
    assert!(!list.body.contains("__delete__"));
    assert_eq!(
        get(&app, "/admin/locked-exchanges/add").await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(&app, "/admin/locked-exchanges/add", "mic=X&name=Y")
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(&app, "/admin/locked-exchanges/XLON/delete", "")
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let form = get(&app, "/admin/locked-exchanges/XLON/change").await;
    assert!(!form.body.contains(r#"name="name""#));
    assert!(form.body.contains(r#"name="country""#));
    assert!(!form.body.contains("/admin/locked-exchanges/XLON/delete"));
}

#[tokio::test]
async fn custom_bulk_action_gets_ids_and_database() {
    let (app, db) = app().await;
    let list = get(&app, "/admin/accounts").await;
    assert!(list.body.contains(r#"<option value="make_premium" data-confirm="Upgrade the selected accounts?">Make premium</option>"#), "{}", list.body);

    let reply = post(&app, "/admin/accounts", "action=make_premium&selected=1&q=").await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.text().contains("Upgraded 1 account(s)."));
    let alice = account::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(alice.account_type, "premium");

    let reply = post(&app, "/admin/accounts", "action=make_premium&q=").await;
    assert!(reply.text().contains("Select at least one row first."));
}

#[tokio::test]
async fn bulk_delete_and_csv_export() {
    let (app, db) = app().await;
    assert!(
        get(&app, "/admin/rules")
            .await
            .body
            .contains("value=\"__csv__\"")
    );
    let csv = post(&app, "/admin/rules", "action=__csv__&q=f_active%3Dtrue").await;
    assert_eq!(csv.status, StatusCode::OK);
    assert_eq!(
        csv.headers.get("content-type").unwrap(),
        "text/csv; charset=utf-8"
    );
    assert_eq!(
        csv.headers.get("content-disposition").unwrap(),
        "attachment; filename=\"rules.csv\""
    );
    let lines: Vec<&str> = csv.body.lines().collect();
    assert_eq!(lines[0], "id,account_id,exchange_mic,name,priority,active");
    assert_eq!(lines.len(), 3, "{}", csv.body);

    let csv = post(&app, "/admin/rules", "action=__csv__&selected=2&q=").await;
    assert_eq!(csv.body.lines().count(), 2);
    assert!(csv.body.contains("Alice high"));

    let reply = post(
        &app,
        "/admin/rules",
        "action=__delete__&selected=1&selected=2&q=",
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(
        reply.text().contains("Deleted 2 rules."),
        "{}",
        reply.text()
    );
    assert_eq!(rule::Entity::find().count(&db).await.unwrap(), 1);
}

#[tokio::test]
async fn boolean_toggle_flips_and_keeps_list_state() {
    let (app, db) = app().await;
    let list = get(&app, "/admin/accounts?f_account_type=free").await;
    assert!(
        list.body
            .contains(r#"formaction="/admin/accounts/1/toggle/is_staff""#)
    );
    let reply = post(
        &app,
        "/admin/accounts/1/toggle/is_staff",
        "q=f_account_type%3Dfree",
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    assert_eq!(reply.location(), "/admin/accounts?f_account_type=free");
    let alice = account::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(alice.is_staff);
    assert_eq!(
        post(&app, "/admin/accounts/1/toggle/username", "")
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn foreign_key_widgets() {
    let (app, db) = app().await;
    let found = get(&app, "/admin/exchanges/autocomplete?q=lon").await;
    assert_eq!(found.status, StatusCode::OK);
    let items: serde_json::Value = serde_json::from_str(&found.body).unwrap();
    assert_eq!(
        items,
        serde_json::json!([{"id": "XLON", "label": "London Stock Exchange"}])
    );

    let form = get(&app, "/admin/rules/add?account_id=2").await;
    assert!(
        form.body
            .contains(r#"data-admin-autocomplete="/admin/exchanges/autocomplete""#)
    );
    assert!(
        form.body
            .contains(r#"<option value="2" selected>bob</option>"#),
        "{}",
        form.body
    );

    let reply = post(
        &app,
        "/admin/rules/add",
        "account_id=2&exchange_mic=XOFF&name=New&priority=2&active=true&notes=",
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);
    let created = rule::Entity::find_by_id(4).one(&db).await.unwrap().unwrap();
    assert_eq!(created.exchange_mic.as_deref(), Some("XOFF"));
    assert_eq!(created.notes, None);

    let form = get(&app, "/admin/rules/1/change").await;
    assert!(form.text().contains("Currently: London Stock Exchange"));
    assert!(
        form.body
            .contains(r#"<textarea class="form-control" id="id_notes" name="notes""#)
    );
}

#[tokio::test]
async fn edit_page_lists_child_rows() {
    let (app, _db) = app().await;
    let form = get(&app, "/admin/accounts/1/change").await;
    assert!(
        form.body
            .contains(r#"<a href="/admin/rules/1/change">Alice low</a>"#),
        "{}",
        form.body
    );
    assert!(
        form.body
            .contains(r#"<a href="/admin/rules/2/change">Alice high</a>"#)
    );
    assert!(!form.text().contains("Bob middle"));
    assert!(
        form.body
            .contains(r#"href="/admin/rules/add?account_id=1""#)
    );
    assert!(form.body.contains(r#"href="/admin/rules?f_account_id=1""#));
}

#[tokio::test]
async fn composite_keys_are_list_only() {
    let (app, _db) = app().await;
    let list = get(&app, "/admin/watches").await;
    assert_eq!(list.status, StatusCode::OK);
    assert!(list.text().contains("Watching London"));
    assert!(!list.body.contains("/admin/watches/add"));
    assert!(!list.body.contains(r#"name="selected""#));
    assert!(!list.body.contains("/change"));
    assert_eq!(
        get(&app, "/admin/watches/add").await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&app, "/admin/watches/1/change").await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn every_link_respects_another_prefix() {
    let db = database().await;
    let router = admin(&db, "/staff/admin/").into_router().unwrap();
    let app = Router::new().nest("/staff/admin", router);
    let allowed = |link: &str| {
        link.starts_with("/staff/admin")
            || link == "/"
            || link == "#"
            || link.starts_with("/static/vendor/bootstrap/")
    };
    for uri in [
        "/staff/admin",
        "/staff/admin/accounts",
        "/staff/admin/accounts?f_is_staff=false&o=-username",
        "/staff/admin/accounts/1/change",
        "/staff/admin/accounts/add",
        "/staff/admin/rules/1/change",
        "/staff/admin/exchanges/XLON/delete",
        "/staff/admin/exchanges",
    ] {
        let page = get(&app, uri).await;
        assert_eq!(page.status, StatusCode::OK, "{uri}");
        let links = links(&page.body);
        assert!(!links.is_empty());
        for link in links {
            assert!(allowed(&link), "{uri} links to {link}");
        }
    }
    let reply = post(
        &app,
        "/staff/admin/accounts/add",
        "username=z&account_type=free",
    )
    .await;
    assert_eq!(reply.location(), "/staff/admin/accounts?notice=added");
    let reply = post(&app, "/staff/admin/exchanges/XAMS/delete", "").await;
    assert_eq!(reply.location(), "/staff/admin/exchanges?notice=deleted");
}

#[tokio::test]
async fn registration_mistakes_fail_at_startup() {
    let db = database().await;
    let unknown_column = AdminApp::new(db.clone())
        .access_policy(common::TestPolicy)
        .register(EntityAdmin::from_entity::<rule::Entity>("rules").list_display(["nope"]))
        .into_router();
    assert!(matches!(unknown_column, Err(AdminError::Config(_))));

    let unknown_target = AdminApp::new(db.clone())
        .access_policy(common::TestPolicy)
        .register(
            EntityAdmin::from_entity::<rule::Entity>("rules").foreign_key("account_id", "accounts"),
        )
        .into_router();
    assert!(matches!(unknown_target, Err(AdminError::Config(_))));

    let not_boolean = AdminApp::new(db)
        .access_policy(common::TestPolicy)
        .register(EntityAdmin::from_entity::<rule::Entity>("rules").toggles(["name"]))
        .into_router();
    assert!(matches!(not_boolean, Err(AdminError::Config(_))));
}

#[tokio::test]
async fn observer_failure_rolls_back_mutations_and_observer_writes() {
    use sea_orm::{ConnectionTrait, DatabaseTransaction, DbErr};
    use serde_json::Value;
    struct Reject;
    #[async_trait::async_trait]
    impl reefadmin::MutationObserver for Reject {
        async fn before(
            &self,
            _db: &DatabaseTransaction,
            table: &str,
            row: &Value,
        ) -> Result<Value, DbErr> {
            assert_eq!(table, "exchanges");
            Ok(row.clone())
        }
        async fn after(
            &self,
            db: &DatabaseTransaction,
            _table: &str,
            _row: &Value,
            captured: Value,
        ) -> Result<(), DbErr> {
            assert!(captured["mic"].is_string());
            db.execute_unprepared("UPDATE exchanges SET name='observer changed this'")
                .await?;
            Err(DbErr::Custom("Derived update failed".into()))
        }
    }
    let db = database().await;
    let before = exchange::Entity::find().all(&db).await.unwrap();
    let app = Router::new().nest(
        "/admin",
        AdminApp::new(db.clone())
            .access_policy(common::TestPolicy)
            .observe_mutations(Reject)
            .register(
                EntityAdmin::from_entity::<exchange::Entity>("exchanges")
                    .fields(["mic", "name", "country"]),
            )
            .into_router()
            .unwrap(),
    );
    for (path, body) in [
        ("/admin/exchanges/add", "mic=FAIL&name=Failure&country=GB"),
        (
            "/admin/exchanges/XAMS/change",
            "mic=XAMS&name=Changed&country=NL",
        ),
        ("/admin/exchanges/XAMS/delete", ""),
    ] {
        let response = post(&app, path, body).await;
        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!response.body.contains("Derived update failed"));
        assert!(response.body.contains("contact your administrator"));
        assert_eq!(
            exchange::Entity::find().all(&db).await.unwrap(),
            before,
            "{path}"
        );
    }
}

#[tokio::test]
async fn host_mapped_write_errors_render_public_messages_and_roll_back() {
    use reefadmin::ValidationErrorMapper;
    use sea_orm::DbErr;
    use std::collections::HashMap;

    struct MapValidation;
    impl ValidationErrorMapper for MapValidation {
        fn validation_errors(
            &self,
            entity: &str,
            error: &AdminError,
        ) -> Option<HashMap<String, String>> {
            let AdminError::Database(DbErr::Custom(message)) = error else {
                return None;
            };
            match (entity, message.as_str()) {
                ("exchanges", "invalid name") => Some(HashMap::from([(
                    "name".to_string(),
                    "Enter an accepted name.".to_string(),
                )])),
                ("exchanges", "unknown field") => Some(HashMap::from([
                    ("__all__".to_string(), "Check the record.".to_string()),
                    ("timezone".to_string(), "Set a timezone.".to_string()),
                ])),
                _ => None,
            }
        }
    }
    struct Reject;
    #[async_trait::async_trait]
    impl reefadmin::MutationObserver for Reject {
        async fn before(
            &self,
            _db: &sea_orm::DatabaseTransaction,
            _table: &str,
            row: &serde_json::Value,
        ) -> Result<serde_json::Value, DbErr> {
            Ok(row.clone())
        }
        async fn after(
            &self,
            _db: &sea_orm::DatabaseTransaction,
            table: &str,
            row: &serde_json::Value,
            _captured: serde_json::Value,
        ) -> Result<(), DbErr> {
            Err(DbErr::Custom(match (table, row["name"].as_str()) {
                ("accounts", _) | (_, Some("Bad")) => "invalid name".into(),
                (_, Some("Unknown")) => "unknown field".into(),
                _ => "database down".into(),
            }))
        }
    }
    let db = database().await;
    let exchanges = EntityAdmin::from_entity::<exchange::Entity>("exchanges")
        .fields(["mic", "name", "country"]);
    let accounts = EntityAdmin::from_entity::<account::Entity>("accounts").fields([
        "username",
        "account_type",
        "is_staff",
    ]);
    let app = Router::new().nest(
        "/admin",
        AdminApp::new(db.clone())
            .access_policy(common::TestPolicy)
            .observe_mutations(Reject)
            .map_write_errors(MapValidation)
            .register(exchanges)
            .register(accounts)
            .into_router()
            .unwrap(),
    );

    let response = post(&app, "/admin/exchanges/add", "mic=TEST&name=Bad&country=GB").await;
    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(response.body.contains("Enter an accepted name."));
    assert!(response.body.contains(r#"value="TEST""#));
    assert!(!response.body.contains("invalid name"));
    assert!(
        exchange::Entity::find_by_id("TEST")
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );

    let response = post(
        &app,
        "/admin/exchanges/XLON/change",
        "mic=XLON&name=Bad&country=GB",
    )
    .await;
    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(response.body.contains("Enter an accepted name."));
    assert!(!response.body.contains("invalid name"));
    let saved = exchange::Entity::find_by_id("XLON")
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.name, "London Stock Exchange");

    let response = post(
        &app,
        "/admin/exchanges/add",
        "mic=TEST&name=Unknown&country=GB",
    )
    .await;
    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response
            .text()
            .matches("Check the record. Set a timezone.")
            .count(),
        1
    );

    let response = post(
        &app,
        "/admin/exchanges/add",
        "mic=TEST&name=Down&country=GB",
    )
    .await;
    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!response.body.contains("database down"));

    let response = post(
        &app,
        "/admin/accounts/add",
        "username=carol&account_type=free&is_staff=false",
    )
    .await;
    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!response.body.contains("Enter an accepted name."));
}

#[tokio::test]
async fn truncated_foreign_key_selects_keep_current_and_submitted_relations() {
    use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};

    let db = database().await;
    account::Entity::insert_many((0..1002).map(|index| account::ActiveModel {
        username: Set(format!("account {index:04}")),
        account_type: Set("free".into()),
        is_staff: Set(false),
        created_at: Set(chrono::Utc::now().fixed_offset()),
        ..Default::default()
    }))
    .exec(&db)
    .await
    .unwrap();
    exchange::Entity::insert_many((0..1002).map(|index| exchange::ActiveModel {
        mic: Set(format!("M{index:04}")),
        name: Set(format!("Market {index:04}")),
        country: Set(None),
    }))
    .exec(&db)
    .await
    .unwrap();
    let mut row = rule::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    row.account_id = Set(1004);
    row.exchange_mic = Set(Some("M1001".into()));
    row.update(&db).await.unwrap();
    let router = AdminApp::new(db.clone())
        .access_policy(common::TestPolicy)
        .prefix("/admin")
        .register(
            EntityAdmin::from_entity::<account::Entity>("accounts").display_column("username"),
        )
        .register(EntityAdmin::from_entity::<exchange::Entity>("exchanges").display_column("name"))
        .register(
            EntityAdmin::from_entity::<rule::Entity>("rules")
                .foreign_key("account_id", "accounts")
                .foreign_key("exchange_mic", "exchanges"),
        )
        .into_router()
        .unwrap();
    let app = Router::new().nest("/admin", router);
    let selected = |body: &str, account: &str, exchange: &str| {
        assert!(
            body.contains(&format!(r#"<option value="{account}" selected>"#)),
            "{body}"
        );
        assert!(
            body.contains(&format!(r#"<option value="{exchange}" selected>"#)),
            "{body}"
        );
        assert_eq!(
            body.matches(&format!(r#"<option value="{account}""#))
                .count(),
            1
        );
        assert_eq!(
            body.matches(&format!(r#"<option value="{exchange}""#))
                .count(),
            1
        );
    };
    let form = get(&app, "/admin/rules/1/change").await;
    assert_eq!(form.status, StatusCode::OK);
    selected(&form.body, "1004", "M1001");
    assert!(form.body.contains("account 1001</option>"));
    assert!(form.body.contains("Market 1001</option>"));

    // Redisplay must keep the submitted relation, even when it differs from the saved row.
    let failed = post(
        &app,
        "/admin/rules/1/change",
        "account_id=1003&exchange_mic=M1000&name=Changed&priority=invalid",
    )
    .await;
    assert_eq!(failed.status, StatusCode::UNPROCESSABLE_ENTITY);
    selected(&failed.body, "1003", "M1000");
    let saved = rule::Entity::find_by_id(1).one(&db).await.unwrap().unwrap();
    assert_eq!(saved.account_id, 1004);
    assert_eq!(saved.exchange_mic.as_deref(), Some("M1001"));

    let reply = post(
        &app,
        "/admin/rules/1/change",
        "account_id=1004&exchange_mic=M1001&name=Changed&priority=2&active=true",
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);
    let saved = rule::Entity::find_by_id(1).one(&db).await.unwrap().unwrap();
    assert_eq!(saved.account_id, 1004);
    assert_eq!(saved.exchange_mic.as_deref(), Some("M1001"));
    assert_eq!(saved.name, "Changed");

    let reply = post(
        &app,
        "/admin/rules/1/change",
        "account_id=1004&exchange_mic=&name=Changed&priority=2",
    )
    .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    let form = get(&app, "/admin/rules/1/change").await;
    selected(&form.body, "1004", "");
    let saved = rule::Entity::find_by_id(1).one(&db).await.unwrap().unwrap();
    assert_eq!(saved.exchange_mic, None);
}

#[tokio::test]
async fn template_configuration_runs_before_override_validation() {
    let db = database().await;
    let router = AdminApp::new(db).access_policy(common::TestPolicy)
        .configure_templates(|tera| {
            tera.register_filter("shout", |value: &str, _: tera::Kwargs, _: &tera::State| value.to_uppercase());
        })
        .template("index.html", "{% extends \"custom.html\" %}{% block content %}{{ admin_title | shout }}{% endblock %}")
        .template("custom.html", "<main>{% block content %}{% endblock %}</main>")
        .title("Example")
        .into_router().unwrap();
    let page = get(&Router::new().nest("/admin", router), "/admin").await;
    assert_eq!(page.status, StatusCode::OK);
    assert_eq!(page.body, "<main>EXAMPLE</main>");
}

#[tokio::test]
async fn bundled_assets_and_application_overrides() {
    let db = database().await;
    let app = Router::new().nest("/staff", admin(&db, "/staff").into_router().unwrap());
    let page = get(&app, "/staff").await;
    assert!(
        page.body
            .contains("/staff/_static/vendor/bootstrap-5.3.8/bootstrap.min.css")
    );
    for (file, kind) in [
        ("bootstrap-5.3.8/bootstrap.min.css", "text/css"),
        ("bootstrap-5.3.8/bootstrap.bundle.min.js", "text/javascript"),
        ("htmx-2.0.11/htmx.min.js", "text/javascript"),
    ] {
        let response = get(&app, &format!("/staff/_static/vendor/{file}")).await;
        assert_eq!(response.status, StatusCode::OK);
        assert!(
            response.headers["content-type"]
                .to_str()
                .unwrap()
                .starts_with(kind)
        );
        assert!(
            response.headers["cache-control"]
                .to_str()
                .unwrap()
                .contains("immutable")
        );
        assert!(response.body.len() > 1000);
    }
    assert_eq!(
        get(&app, "/staff/_static/unknown.js").await.status,
        StatusCode::NOT_FOUND
    );
    let app = Router::new().nest(
        "/staff",
        admin(&db, "/staff")
            .stylesheet("/custom.css")
            .script("/custom.js")
            .extra_stylesheet("/extra.css")
            .extra_script("/extra.js")
            .into_router()
            .unwrap(),
    );
    let page = get(&app, "/staff").await;
    assert!(!page.body.contains("bootstrap-5.3.8"));
    assert!(page.body.find("/custom.css").unwrap() < page.body.find("/extra.css").unwrap());
    assert!(page.body.find("/_static/admin.js").unwrap() < page.body.find("/extra.js").unwrap());
}

#[tokio::test]
async fn list_fragments_match_full_results_and_history_gets_a_page() {
    use axum::{body::Body, http::Request};
    let (app, _) = app().await;
    let uri = "/admin/rules?q=price&o=-priority&p=2";
    let full = get(&app, uri).await;
    let partial = common::send(
        &app,
        Request::get(uri)
            .header("HX-Request", "true")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(partial.status, StatusCode::OK);
    assert_eq!(partial.headers["x-reefadmin-fragment"], "true");
    assert_eq!(partial.headers["cache-control"], "no-store");
    assert!(
        partial.headers["vary"]
            .to_str()
            .unwrap()
            .contains("HX-Request")
    );
    assert!(!partial.body.contains("<!doctype"));
    assert!(full.body.contains(&partial.body));
    let history = common::send(
        &app,
        Request::get(uri)
            .header("HX-Request", "true")
            .header("HX-History-Restore-Request", "true")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(history.body.contains("<!doctype"));
    assert_eq!(full.body, history.body);
    let unknown = common::send(
        &app,
        Request::get("/admin/unknown")
            .header("HX-Request", "true")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn relationship_options_support_html_and_keep_json_contract() {
    use axum::{body::Body, http::Request};
    let (app, _) = app().await;
    let uri = "/admin/exchanges/autocomplete?q=London";
    let json = get(&app, uri).await;
    let items: serde_json::Value = serde_json::from_str(&json.body).unwrap();
    assert!(
        items
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "XLON")
    );
    let html = common::send(
        &app,
        Request::get(uri)
            .header("HX-Request", "true")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(html.status, StatusCode::OK);
    assert_eq!(html.headers["x-reefadmin-fragment"], "true");
    assert!(html.body.contains("value=\"XLON\""));
    assert!(!html.body.contains("<!doctype"));
}

#[tokio::test]
async fn enhanced_writes_use_host_sessions_csrf_and_permissions() {
    use axum::{body::Body, http::Request, middleware};
    let (app, db) = app().await;
    let app = app.layer(middleware::from_fn(common::protect));
    for (cookie, body, expected) in [
        ("", "_csrf=token-a", StatusCode::UNAUTHORIZED),
        ("admin_test_session=a", "", StatusCode::FORBIDDEN),
        (
            "admin_test_session=a",
            "_csrf=token-b",
            StatusCode::FORBIDDEN,
        ),
    ] {
        let request = Request::post("/admin/accounts/1/toggle/is_staff")
            .header("HX-Request", "true")
            .header("Cookie", cookie)
            .body(Body::from(body))
            .unwrap();
        assert_eq!(common::send(&app, request).await.status, expected);
        assert!(
            !account::Entity::find_by_id(1)
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .is_staff
        );
    }
    for token in ["a", "b"] {
        let page = common::send(
            &app,
            Request::get("/admin/accounts")
                .header("Cookie", format!("admin_test_session={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert!(
            page.body
                .contains(&format!("name=\"_csrf\" value=\"token-{token}\""))
        );
    }
    let request = Request::post("/admin/accounts/1/toggle/is_staff")
        .header("HX-Request", "true")
        .header("Cookie", "admin_test_session=a")
        .body(Body::from("_csrf=token-a"))
        .unwrap();
    let reply = common::send(&app, request).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.headers["x-reefadmin-fragment"], "true");
    assert!(reply.body.contains("Turn off"));
    assert!(!reply.body.contains("<!doctype"));
    assert!(
        account::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .is_staff
    );
    let request = Request::post("/admin/accounts/1/toggle/username")
        .header("HX-Request", "true")
        .header("Cookie", "admin_test_session=a")
        .body(Body::from("_csrf=token-a"))
        .unwrap();
    assert_eq!(
        common::send(&app, request).await.status,
        StatusCode::FORBIDDEN
    );
    for uri in ["/admin/accounts", "/admin/exchanges/autocomplete"] {
        let request = Request::get(uri)
            .header("HX-Request", "true")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            common::send(&app, request).await.status,
            StatusCode::UNAUTHORIZED
        );
    }
}

#[tokio::test]
async fn enhanced_toggle_rolls_back_observer_failure() {
    use axum::{body::Body, http::Request};
    use reefadmin::MutationObserver;
    use sea_orm::{ConnectionTrait, DatabaseTransaction, DbErr};
    use serde_json::Value;
    struct Reject;
    #[async_trait::async_trait]
    impl MutationObserver for Reject {
        async fn before(
            &self,
            _: &DatabaseTransaction,
            _: &str,
            row: &Value,
        ) -> Result<Value, DbErr> {
            Ok(row.clone())
        }

        async fn after(
            &self,
            db: &DatabaseTransaction,
            _: &str,
            _: &Value,
            _: Value,
        ) -> Result<(), DbErr> {
            db.execute_unprepared("UPDATE accounts SET username='should roll back'")
                .await?;
            Err(DbErr::Custom("Rejected by application".into()))
        }
    }
    let db = database().await;
    let before = account::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let app = Router::new().nest(
        "/admin",
        admin(&db, "/admin")
            .observe_mutations(Reject)
            .into_router()
            .unwrap(),
    );
    let request = Request::post("/admin/accounts/1/toggle/is_staff")
        .header("HX-Request", "true")
        .body(Body::empty())
        .unwrap();
    let response = common::send(&app, request).await;
    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!response.headers.contains_key("x-reefadmin-fragment"));
    assert!(!response.body.contains("Rejected by application"));
    assert!(response.body.contains("contact your administrator"));
    assert_eq!(
        account::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        before
    );
}

#[tokio::test]
async fn invalid_registration_options_fail_before_serving_requests() {
    let db = database().await;
    for entity in [
        EntityAdmin::from_entity::<account::Entity>("_private"),
        EntityAdmin::from_entity::<account::Entity>("accounts").search_fields(["missing"]),
        EntityAdmin::from_entity::<account::Entity>("accounts").list_filter(["missing"]),
        EntityAdmin::from_entity::<account::Entity>("accounts").ordering(["missing"]),
        EntityAdmin::from_entity::<account::Entity>("accounts").display_column("missing"),
        EntityAdmin::from_entity::<account::Entity>("accounts").fields(["missing"]),
        EntityAdmin::from_entity::<account::Entity>("accounts").choices("missing", [("x", "X")]),
        EntityAdmin::from_entity::<account::Entity>("accounts").computed(
            "username",
            "Collision",
            |_| String::new(),
        ),
    ] {
        assert!(matches!(
            AdminApp::new(db.clone())
                .access_policy(common::TestPolicy)
                .register(entity)
                .into_router(),
            Err(AdminError::Config(_))
        ));
    }
}

#[tokio::test]
async fn direct_requests_cannot_bypass_disabled_operations_or_field_allowlists() {
    let db = database().await;
    let app = Router::new().nest(
        "/admin",
        AdminApp::new(db.clone())
            .access_policy(common::TestPolicy)
            .register(
                EntityAdmin::from_entity::<account::Entity>("accounts")
                    .fields(["username"])
                    .can_add(false)
                    .can_delete(false)
                    .csv_export(false)
                    .toggles(["is_staff"])
                    .can_edit(false),
            )
            .register(EntityAdmin::from_entity::<account::Entity>("limited").fields(["username"]))
            .into_router()
            .unwrap(),
    );
    let before = account::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    for (path, body) in [
        ("/admin/accounts/1/change", "username=Hacked&is_staff=true"),
        ("/admin/accounts/1/toggle/is_staff", ""),
        ("/admin/accounts/1/delete", ""),
        ("/admin/accounts", "action=__delete__&selected=1"),
        ("/admin/accounts", "action=__csv__"),
    ] {
        assert_eq!(
            post(&app, path, body).await.status,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    assert_eq!(
        account::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        before
    );
    let changed = post(
        &app,
        "/admin/limited/1/change",
        "username=Allowed&is_staff=true&account_type=premium&unknown=1",
    )
    .await;
    assert_eq!(changed.status, StatusCode::SEE_OTHER);
    let after = account::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.username, "Allowed");
    assert_eq!(after.is_staff, before.is_staff);
    assert_eq!(after.account_type, before.account_type);
    assert_eq!(
        post(&app, "/admin/limited/999/change", "username=Missing")
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn custom_action_errors_redirects_and_invalid_ids_are_handled() {
    use reefadmin::{Action, ActionResult};
    let db = database().await;
    let app = Router::new().nest(
        "/staff",
        AdminApp::new(db.clone())
            .access_policy(common::TestPolicy)
            .prefix("/staff")
            .register(
                EntityAdmin::from_entity::<account::Entity>("accounts")
                    .action(Action::new("error", "Error", |_| async {
                        Ok(ActionResult::error("Not allowed <script>"))
                    }))
                    .action(Action::new("failed", "Failed", |_| async {
                        Err(AdminError::custom("Domain failure"))
                    }))
                    .action(Action::new("redirect", "Redirect", |_| async {
                        Ok(ActionResult::Redirect("/accounts".into()))
                    }))
                    .action(Action::new("typed", "Typed", |ctx| async move {
                        ctx.ids_as::<i64>()?;
                        Ok(ActionResult::success("Parsed"))
                    })),
            )
            .into_router()
            .unwrap(),
    );
    let before = account::Entity::find().all(&db).await.unwrap();
    for (action, message) in [("error", "Not allowed &lt;script&gt;"), ("typed", "Parsed")] {
        let reply = post(
            &app,
            "/staff/accounts",
            &format!("action={action}&selected=1"),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK);
        assert!(reply.body.contains(message), "{}", reply.body);
    }
    let failed = post(&app, "/staff/accounts", "action=failed&selected=1").await;
    assert_eq!(failed.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!failed.body.contains("Domain failure"));
    assert!(failed.body.contains("contact your administrator"));
    let redirect = post(&app, "/staff/accounts", "action=redirect&selected=1").await;
    assert_eq!(redirect.status, StatusCode::SEE_OTHER);
    assert_eq!(redirect.location(), "/staff/accounts");
    assert_eq!(
        post(&app, "/staff/accounts", "action=unknown&selected=1")
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(account::Entity::find().all(&db).await.unwrap(), before);
}

#[tokio::test]
async fn bulk_delete_deduplicates_ids_and_preserves_prior_commits_on_failure() {
    use reefadmin::MutationObserver;
    use sea_orm::{DatabaseTransaction, DbErr};
    use serde_json::Value;
    struct RejectSecond;
    #[async_trait::async_trait]
    impl MutationObserver for RejectSecond {
        async fn before(
            &self,
            _: &DatabaseTransaction,
            _: &str,
            row: &Value,
        ) -> Result<Value, DbErr> {
            if row["id"] == 2 {
                return Err(DbErr::Custom("Keep second row".into()));
            }
            Ok(Value::Null)
        }
        async fn after(
            &self,
            _: &DatabaseTransaction,
            _: &str,
            _: &Value,
            _: Value,
        ) -> Result<(), DbErr> {
            Ok(())
        }
    }
    let db = database().await;
    let app = Router::new().nest(
        "/admin",
        AdminApp::new(db.clone())
            .access_policy(common::TestPolicy)
            .observe_mutations(RejectSecond)
            .register(EntityAdmin::from_entity::<rule::Entity>("rules"))
            .into_router()
            .unwrap(),
    );
    let response = post(
        &app,
        "/admin/rules",
        "action=__delete__&selected=1&selected=1&selected=2",
    )
    .await;
    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        rule::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        rule::Entity::find_by_id(2)
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        rule::Entity::find_by_id(3)
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn empty_results_invalid_queries_and_untrusted_values_render_safely() {
    use sea_orm::{ActiveModelTrait, Set};
    let (app, db) = app().await;
    let mut model: account::ActiveModel = account::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into();
    model.username = Set("<script>alert('x')</script>".into());
    model.update(&db).await.unwrap();
    let page = get(&app, "/admin/accounts").await;
    assert!(!page.body.contains("<script>alert("));
    assert!(page.body.contains("&lt;script&gt;"));
    for query in [
        "q=nonexistent",
        "p=0",
        "p=999999",
        "o=missing&f_missing=anything",
    ] {
        assert_eq!(
            get(&app, &format!("/admin/accounts?{query}")).await.status,
            StatusCode::OK
        );
    }
    let page = get(&app, "/admin/accounts?q=nonexistent").await;
    assert!(page.text().contains("No accounts found."));
    let bad_choice = post(
        &app,
        "/admin/accounts/1/change",
        "username=Unchanged&account_type=invalid",
    )
    .await;
    assert_eq!(bad_choice.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        account::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .username,
        "<script>alert('x')</script>"
    );
}

#[tokio::test]
async fn exports_require_an_explicit_allowlist_even_for_direct_requests() {
    let db = database().await;
    for entity in [
        EntityAdmin::from_entity::<account::Entity>("accounts")
            .list_display(["username"])
            .fields(["username"]),
        EntityAdmin::from_entity::<account::Entity>("accounts").csv_export(true),
        EntityAdmin::from_entity::<account::Entity>("accounts")
            .export_fields(["username"])
            .export_fields(Vec::<String>::new()),
        EntityAdmin::from_entity::<account::Entity>("accounts")
            .csv_export(false)
            .export_fields(["username"]),
        EntityAdmin::from_entity::<account::Entity>("accounts")
            .export_fields(["username"])
            .csv_export(false),
    ] {
        let app = Router::new().nest(
            "/admin",
            AdminApp::new(db.clone())
                .access_policy(common::TestPolicy)
                .register(entity)
                .into_router()
                .unwrap(),
        );
        let page = get(&app, "/admin/accounts").await;
        assert_eq!(page.status, StatusCode::OK);
        assert!(!page.body.contains("value=\"__csv__\""));
        for body in [
            "action=__csv__",
            "action=__csv__&selected=1",
            "action=__csv__&selected=999",
        ] {
            assert_eq!(
                post(&app, "/admin/accounts", body).await.status,
                StatusCode::FORBIDDEN
            );
        }
    }
}

#[tokio::test]
async fn exports_include_only_configured_columns_in_order_without_computed_queries() {
    let db = database().await;
    let app = Router::new().nest(
        "/admin",
        AdminApp::new(db)
            .access_policy(common::TestPolicy)
            .register(
                EntityAdmin::from_entity::<account::Entity>("accounts")
                    .list_display(["id", "account_type", "is_staff"])
                    .fields(["account_type"])
                    .readonly_fields(["username"])
                    .export_fields(["username", "account_type"])
                    // Export must not evaluate an unrelated display expression, even during ID preflight.
                    .computed_sql("unrelated", "Unrelated", "SELECT absent FROM missing_table"),
            )
            .into_router()
            .unwrap(),
    );
    for body in ["action=__csv__", "action=__csv__&selected=1&selected=01"] {
        let reply = post(&app, "/admin/accounts", body).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        assert_eq!(reply.headers.get("cache-control").unwrap(), "no-store");
        assert_eq!(reply.body.lines().next(), Some("username,account_type"));
        assert!(reply.body.contains("alice,free"));
        assert!(!reply.body.contains("is_staff"));
        assert!(!reply.body.contains("created_at"));
        assert!(!reply.body.contains("unrelated"));
        assert_eq!(
            reply.body.lines().count(),
            if body.contains("selected") { 2 } else { 3 }
        );
    }
}

#[tokio::test]
async fn invalid_export_fields_fail_at_startup() {
    let db = database().await;
    for (fields, message) in [
        (vec!["missing"], "unknown or computed column missing"),
        (vec!["username", "username"], "duplicate column username"),
        (vec!["computed"], "unknown or computed column computed"),
    ] {
        let result = AdminApp::new(db.clone())
            .access_policy(common::TestPolicy)
            .register(
                EntityAdmin::from_entity::<account::Entity>("accounts")
                    .computed("computed", "Computed", |_| "value".into())
                    .export_fields(fields),
            )
            .into_router();
        assert!(
            matches!(result, Err(AdminError::Config(ref error)) if error.contains(message)),
            "{result:?}"
        );
    }
}

#[tokio::test]
async fn fieldsets_preserve_readonly_values_widget_precedence_and_failed_submissions() {
    let db = database().await;
    let app = Router::new().nest(
        "/admin",
        AdminApp::new(db.clone())
            .access_policy(common::TestPolicy)
            .register(
                EntityAdmin::from_entity::<account::Entity>("accounts").display_column("username"),
            )
            .register(
                EntityAdmin::from_entity::<rule::Entity>("rules")
                    .fields(["notes"])
                    .fieldset(Some("Details"), ["name", "account_id"])
                    .fieldset(Some("Settings"), ["priority", "active", "notes"])
                    .readonly_fields(["name"])
                    .foreign_key("account_id", "accounts")
                    .autocomplete_fields(["account_id"])
                    .choices(
                        "account_id",
                        [("1", "Primary account"), ("2", "Secondary account")],
                    )
                    .help_text("priority", "Small numbers first")
                    .field_label("notes", "Explanation"),
            )
            .into_router()
            .unwrap(),
    );
    let before = rule::Entity::find_by_id(1).one(&db).await.unwrap().unwrap();
    let form = get(&app, "/admin/rules/1/change").await;
    assert_eq!(form.status, StatusCode::OK);
    assert!(form.text().find("Details").unwrap() < form.text().find("Settings").unwrap());
    assert!(form.text().contains("Alice low"));
    assert!(!form.body.contains("name=\"name\""));
    assert!(
        form.body
            .contains("<option value=\"1\" selected>Primary account</option>")
    );
    assert!(!form.body.contains("data-admin-autocomplete="));
    assert!(form.body.contains("Small numbers first"));
    assert!(form.body.contains("Explanation"));

    let reply = post(
        &app,
        "/admin/rules/1/change",
        "name=forged&account_id=2&priority=bad&active=false&notes=Keep+%3Cthis%3E",
    )
    .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        reply
            .body
            .contains("<option value=\"2\" selected>Secondary account</option>")
    );
    assert!(reply.body.contains("value=\"bad\""));
    assert!(reply.body.contains("Enter a whole number."));
    assert!(reply.body.contains("Keep &lt;this&gt;"));
    assert!(reply.text().contains("Alice low"));
    assert!(!reply.body.contains("forged"));
    assert!(!reply.body.contains("value=\"true\" checked"));
    assert_eq!(
        rule::Entity::find_by_id(1).one(&db).await.unwrap().unwrap(),
        before
    );
}

#[tokio::test]
async fn csv_preserves_special_characters_empty_cells_and_record_boundaries() {
    use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};

    let db = database().await;
    let app = Router::new().nest(
        "/admin",
        AdminApp::new(db.clone())
            .access_policy(common::TestPolicy)
            .register(
                EntityAdmin::from_entity::<account::Entity>("accounts")
                    .search_fields(["username"])
                    .export_fields(["username", "account_type"]),
            )
            .into_router()
            .unwrap(),
    );
    for (value, escaped) in [
        ("comma,quote\"", "\"comma,quote\"\"\""),
        ("one\rtwo\nthree\r\nfour", "\"one\rtwo\nthree\r\nfour\""),
        ("海 é", "海 é"),
        ("", ""),
        ("=1+1", "=1+1"),
    ] {
        let mut row = account::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .into_active_model();
        row.username = Set(value.into());
        row.account_type = Set(String::new());
        row.update(&db).await.unwrap();
        let reply = post(&app, "/admin/accounts", "action=__csv__&selected=1").await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(
            reply.body,
            format!("username,account_type\r\n{escaped},\r\n")
        );
    }
    let empty = post(&app, "/admin/accounts", "action=__csv__&q=q%3Dmissing-row").await;
    assert_eq!(empty.status, StatusCode::OK);
    assert_eq!(empty.body, "username,account_type\r\n");
}

#[tokio::test]
async fn internal_read_and_template_errors_are_private_for_pages_and_fragments() {
    use axum::{body::Body, http::Request};
    use sea_orm::ConnectionTrait;

    let db = database().await;
    let broken_template = Router::new().nest(
        "/admin",
        admin(&db, "/admin")
            .template("list_results.html", "{{ admin_title | int }}")
            .into_router()
            .unwrap(),
    );
    let broken_database = Router::new().nest("/admin", admin(&db, "/admin").into_router().unwrap());
    for (app, drop_table) in [(&broken_template, false), (&broken_database, true)] {
        if drop_table {
            db.execute_unprepared("DROP TABLE accounts").await.unwrap();
        }
        for enhanced in [false, true] {
            let mut request = Request::get("/admin/accounts");
            if enhanced {
                request = request.header("HX-Request", "true");
            }
            let response = common::send(app, request.body(Body::empty()).unwrap()).await;
            assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.headers["cache-control"], "no-store");
            assert_eq!(
                response.body,
                "The request could not be completed. Try again or contact your administrator."
            );
        }
    }
}
