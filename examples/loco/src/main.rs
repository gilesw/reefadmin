use reefadmin_loco_example::{
    app, auth,
    models::{categories, products},
};
use sea_orm::{ActiveModelTrait, Set};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let secret = std::env::var("INVENTORY_SESSION_SECRET")
        .map_err(|_| "Set INVENTORY_SESSION_SECRET to a random secret (openssl rand -hex 32).")?;
    let url = std::env::var("INVENTORY_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://inventory.sqlite?mode=rwc".into());
    let port = std::env::var("INVENTORY_PORT")
        .unwrap_or_else(|_| "39128".into())
        .parse()?;
    let boot = app::boot(&url, &secret, port).await?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("create-user")
            if args.len() == 3 && matches!(args[2].as_str(), "staff" | "member") =>
        {
            let password = std::env::var("INVENTORY_INITIAL_PASSWORD")
                .map_err(|_| "Set INVENTORY_INITIAL_PASSWORD for this command.")?;
            let user = auth::create_user(
                &boot.app_context.db,
                &args[1],
                &password,
                args[2] == "staff",
            )
            .await?;
            let category = categories::ActiveModel {
                owner_id: Set(user.id),
                name: Set("Equipment".into()),
                ..Default::default()
            }
            .insert(&boot.app_context.db)
            .await?;
            products::ActiveModel {
                owner_id: Set(user.id),
                category_id: Set(category.id),
                name: Set("Sample product".into()),
                active: Set(true),
                ..Default::default()
            }
            .insert(&boot.app_context.db)
            .await?;
            println!("Created user and sample inventory.");
        }
        None | Some("serve") => {
            let listener =
                tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
            println!("Inventory: http://127.0.0.1:{port}/login");
            axum::serve(
                listener,
                boot.router.ok_or("Loco did not return a server router")?,
            )
            .await?;
        }
        _ => return Err("Usage: inventory [serve | create-user EMAIL staff|member]".into()),
    }
    Ok(())
}
