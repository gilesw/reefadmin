#![allow(dead_code)]

use axum::{
    Router,
    body::Body,
    extract::Request as AxumRequest,
    http::{HeaderMap, Method, Request, StatusCode, header},
    middleware::Next,
    response::Response,
};
use http_body_util::BodyExt;
use reefadmin::{Action, ActionResult, AdminApp, EntityAdmin, Inline, MaxLength};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, Schema, Set, sea_query::Expr,
};
use tower::ServiceExt;

pub mod exchange {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "exchanges")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub mic: String,
        pub name: String,
        pub country: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod account {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "accounts")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub username: String,
        pub account_type: String,
        pub is_staff: bool,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    #[async_trait::async_trait]
    impl ActiveModelBehavior for ActiveModel {
        async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
        where
            C: ConnectionTrait,
        {
            if insert && self.created_at.is_not_set() {
                self.created_at = sea_orm::Set(chrono::Utc::now().fixed_offset());
            }
            Ok(self)
        }
    }
}

pub mod rule {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "rules")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub account_id: i64,
        pub exchange_mic: Option<String>,
        pub name: String,
        pub priority: i32,
        pub active: bool,
        #[sea_orm(column_type = "Text", nullable)]
        pub notes: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod watch {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "watches")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub account_id: i64,
        #[sea_orm(primary_key, auto_increment = false)]
        pub exchange_mic: String,
        pub note: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// An in-memory database with alice (id 1, free, two rules) and bob (id 2, premium, one rule).
pub async fn database() -> DatabaseConnection {
    database_at("sqlite::memory:").await
}

/// Creates the test tables in an empty database at `url` and seeds them.
pub async fn database_at(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    db.execute(&schema.create_table_from_entity(exchange::Entity))
        .await
        .unwrap();
    db.execute(&schema.create_table_from_entity(account::Entity))
        .await
        .unwrap();
    db.execute(&schema.create_table_from_entity(rule::Entity))
        .await
        .unwrap();
    db.execute(&schema.create_table_from_entity(watch::Entity))
        .await
        .unwrap();

    for (mic, name, country) in [
        ("XLON", "London Stock Exchange", Some("GB")),
        ("XAMS", "Euronext Amsterdam", Some("NL")),
        ("XOFF", "Off exchange", None),
    ] {
        exchange::ActiveModel {
            mic: Set(mic.into()),
            name: Set(name.into()),
            country: Set(country.map(Into::into)),
        }
        .insert(&db)
        .await
        .unwrap();
    }
    for (username, account_type) in [("alice", "free"), ("bob", "premium")] {
        account::ActiveModel {
            username: Set(username.into()),
            account_type: Set(account_type.into()),
            is_staff: Set(false),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
    }
    for (account_id, mic, name, priority, active) in [
        (1, Some("XLON"), "Alice low", 1, true),
        (1, None, "Alice high", 5, false),
        (2, Some("XAMS"), "Bob middle", 3, true),
    ] {
        rule::ActiveModel {
            account_id: Set(account_id),
            exchange_mic: Set(mic.map(Into::into)),
            name: Set(name.into()),
            priority: Set(priority),
            active: Set(active),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
    }
    watch::ActiveModel {
        account_id: Set(1),
        exchange_mic: Set("XLON".into()),
        note: Set("Watching London".into()),
    }
    .insert(&db)
    .await
    .unwrap();
    db
}

pub fn admin(db: &DatabaseConnection, prefix: &str) -> AdminApp {
    AdminApp::new(db.clone())
        .access_policy(TestPolicy)
        .title("Test admin")
        .prefix(prefix)
        .register(
            EntityAdmin::from_entity::<exchange::Entity>("exchanges")
                .group("Markets")
                .display_column("name")
                .list_display(["mic", "name", "country"])
                .search_fields(["mic", "name"])
                .list_filter(["country"])
                .ordering(["mic"])
                .per_page(2),
        )
        .register(
            EntityAdmin::from_entity::<exchange::Entity>("locked-exchanges")
                .group("Markets")
                .label("Locked exchanges")
                .can_add(false)
                .can_delete(false)
                .readonly_fields(["name"]),
        )
        .register(
            EntityAdmin::from_entity::<account::Entity>("accounts")
                .export_fields(["id", "username", "account_type"])
                .group("People")
                .display_column("username")
                .list_display([
                    "username",
                    "account_type",
                    "is_staff",
                    "rule_count",
                    "shout",
                    "active_rules",
                ])
                .computed_sql(
                    "rule_count",
                    "Rules",
                    "SELECT COUNT(*) FROM rules WHERE rules.account_id = accounts.id",
                )
                .computed("shout", "Shout", |row| {
                    row.get("username")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_uppercase()
                })
                .computed_async("active_rules", "Active rules", |db, row| async move {
                    let id = row.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
                    let count = rule::Entity::find()
                        .filter(rule::Column::AccountId.eq(id))
                        .filter(rule::Column::Active.eq(true))
                        .count(&db)
                        .await?;
                    Ok(count.to_string())
                })
                .choices(
                    "account_type",
                    [
                        ("free", "Free"),
                        ("premium", "Premium"),
                        ("friend", "Friend"),
                    ],
                )
                .validator("username", MaxLength(20))
                .search_fields(["username"])
                .list_filter(["account_type", "is_staff"])
                .readonly_fields(["created_at", "rule_count"])
                .toggles(["is_staff"])
                .inline(Inline::new("rules", "account_id").columns(["name", "priority", "active"]))
                .action(
                    Action::new("make_premium", "Make premium", |ctx| async move {
                        let ids = ctx.ids_as::<i64>()?;
                        let result = account::Entity::update_many()
                            .col_expr(account::Column::AccountType, Expr::value("premium"))
                            .filter(account::Column::Id.is_in(ids))
                            .exec(&ctx.db)
                            .await?;
                        Ok(ActionResult::success(format!(
                            "Upgraded {} account(s).",
                            result.rows_affected
                        )))
                    })
                    .confirm("Upgrade the selected accounts?"),
                ),
        )
        .register(
            EntityAdmin::from_entity::<rule::Entity>("rules")
                .export_fields([
                    "id",
                    "account_id",
                    "exchange_mic",
                    "name",
                    "priority",
                    "active",
                ])
                .list_display(["name", "account_id", "exchange_mic", "priority", "active"])
                .foreign_key("account_id", "accounts")
                .foreign_key("exchange_mic", "exchanges")
                .autocomplete_fields(["exchange_mic"])
                .search_fields(["name"])
                .search_sql("SELECT username FROM accounts WHERE accounts.id = rules.account_id")
                .list_filter(["account_id", "active"])
                .ordering(["-priority"]),
        )
        .register(EntityAdmin::from_entity::<watch::Entity>("watches"))
}

/// The admin nested at `/admin`, as the app mounts it.
pub async fn app() -> (Router, DatabaseConnection) {
    let db = database().await;
    let router = admin(&db, "/admin").into_router().unwrap();
    (Router::new().nest("/admin", router), db)
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}

impl Reply {
    pub fn location(&self) -> &str {
        self.headers
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
    }

    /// The body with tags removed and whitespace collapsed.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let mut in_tag = false;
        for c in self.body.chars() {
            match c {
                '<' => {
                    in_tag = true;
                    out.push(' ');
                }
                '>' => in_tag = false,
                c if !in_tag => out.push(c),
                _ => {}
            }
        }
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

pub async fn send(app: &Router, mut request: Request<Body>) -> Reply {
    request
        .extensions_mut()
        .insert(reefadmin::AdminActor::new("test"));
    send_raw(app, request).await
}

pub async fn send_raw(app: &Router, request: Request<Body>) -> Reply {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Reply {
        status,
        headers,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

pub async fn get(app: &Router, uri: &str) -> Reply {
    send(app, Request::get(uri).body(Body::empty()).unwrap()).await
}

pub async fn post(app: &Router, uri: &str, body: &str) -> Reply {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, request).await
}

/// Every `href` and `action` attribute value in the page.
pub fn links(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for attr in [
        "href=\"",
        "action=\"",
        "formaction=\"",
        "src=\"",
        "data-admin-autocomplete=\"",
    ] {
        let mut rest = body;
        while let Some(start) = rest.find(attr) {
            let after = &rest[start + attr.len()..];
            let end = after.find('"').unwrap();
            out.push(after[..end].replace("&amp;", "&"));
            rest = &after[end..];
        }
    }
    out
}

/// Test-only session/CSRF middleware. Real applications use their session backend.
pub async fn protect(mut request: AxumRequest, next: Next) -> Response {
    use axum::{body::to_bytes, response::IntoResponse};
    let cookie = request
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = if cookie
        .split(';')
        .any(|part| part.trim() == "admin_test_session=a")
    {
        "token-a"
    } else if cookie
        .split(';')
        .any(|part| part.trim() == "admin_test_session=b")
    {
        "token-b"
    } else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if request.method() == Method::POST {
        let (parts, body) = request.into_parts();
        let bytes = to_bytes(body, 1024 * 1024).await.unwrap();
        if !form_urlencoded::parse(&bytes).any(|(key, value)| key == "_csrf" && value == token) {
            return StatusCode::FORBIDDEN.into_response();
        }
        request = Request::from_parts(parts, Body::from(bytes));
    }
    request.extensions_mut().insert(reefadmin::CsrfToken {
        field_name: "_csrf".into(),
        value: token.into(),
    });
    request
        .extensions_mut()
        .insert(reefadmin::AdminActor::new(token));
    next.run(request).await
}

/// Deliberately permissive fixture policy; never installed by the library.
pub struct TestPolicy;
impl reefadmin::AccessPolicy for TestPolicy {
    fn can_enter(&self, _: &reefadmin::AdminActor) -> bool {
        true
    }
    fn scope(
        &self,
        _: &reefadmin::AdminActor,
        _: reefadmin::AccessEntity<'_>,
        _: &reefadmin::Operation,
    ) -> Option<sea_orm::Condition> {
        Some(sea_orm::Condition::all())
    }
    fn can_write(
        &self,
        _: &reefadmin::AdminActor,
        _: reefadmin::AccessEntity<'_>,
        _: &reefadmin::Operation,
        _: Option<&reefadmin::Row>,
        _: &reefadmin::Row,
    ) -> bool {
        true
    }
}
