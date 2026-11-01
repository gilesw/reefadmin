use crate::{auth, initializers::AdminInitializer, migration::Migrator};
use axum::routing::get;
use loco_rs::{
    Result,
    app::{AppContext, Hooks, Initializer},
    bgworker::Queue,
    boot::{BootResult, StartMode},
    config::Config,
    controller::{AppRoutes, Routes},
    environment::Environment,
    task::Tasks,
};
use std::path::Path;

pub struct App;
#[async_trait::async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        "reefadmin-inventory"
    }
    async fn boot(mode: StartMode, env: &Environment, config: Config) -> Result<BootResult> {
        loco_rs::boot::create_app::<Self, Migrator>(mode, env, config).await
    }
    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::empty().add_route(
            Routes::new()
                .add("/health", get(|| async { "ok" }))
                .add("/login", get(auth::login_page).post(auth::login))
                .add("/account", get(auth::account))
                .add("/logout", axum::routing::post(auth::logout)),
        )
    }
    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![Box::new(AdminInitializer)])
    }
    async fn connect_workers(_ctx: &AppContext, _queue: &Queue) -> Result<()> {
        Ok(())
    }
    fn register_tasks(_tasks: &mut Tasks) {}
    async fn truncate(_ctx: &AppContext) -> Result<()> {
        Err(loco_rs::Error::Message("Truncation is disabled.".into()))
    }
    async fn seed(_ctx: &AppContext, _path: &Path) -> Result<()> {
        Err(loco_rs::Error::Message(
            "Use the explicit create-user command.".into(),
        ))
    }
}

/// Boot the same Loco application for the executable and HTTP tests.
pub async fn boot(database_url: &str, secret: &str, port: u16) -> Result<BootResult> {
    if secret.len() < 64 || !secret.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(loco_rs::Error::Message(
            "Use at least 64 hexadecimal characters for the session secret.".into(),
        ));
    }
    let mut config: Config = serde_yaml::from_str(include_str!("../config/development.yaml"))
        .map_err(|e| loco_rs::Error::Message(e.to_string()))?;
    config.database.uri = database_url.into();
    config.server.port = port.into();
    config.settings = Some(serde_json::json!({"session_secret":secret}));
    App::boot(StartMode::ServerOnly, &Environment::Development, config).await
}
