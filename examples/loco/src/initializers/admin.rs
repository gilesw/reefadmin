use crate::{
    admin::{middleware::protect, policy::InventoryPolicy},
    domain,
    models::{categories, products},
};
use axum::{Router, middleware};
use loco_rs::{
    Result,
    app::{AppContext, Initializer},
};
use reefadmin::{Action, ActionResult, AdminApp, EntityAdmin, MaxLength, Required};

pub struct AdminInitializer;
#[async_trait::async_trait]
impl Initializer for AdminInitializer {
    fn name(&self) -> String {
        "reefadmin".into()
    }
    async fn after_routes(&self, router: Router, ctx: &AppContext) -> Result<Router> {
        let admin = AdminApp::new(ctx.db.clone())
            .title("Inventory administration")
            .prefix("/staff")
            .site_url("/account")
            .access_policy(InventoryPolicy)
            .register(
                EntityAdmin::from_entity::<categories::Entity>("categories")
                    .display_column("name")
                    .search_fields(["name"])
                    .can_add(false)
                    .can_edit(false)
                    .can_delete(false),
            )
            .register(
                EntityAdmin::from_entity::<products::Entity>("products")
                    .display_column("name")
                    .list_display(["name", "category_id", "active"])
                    .fields(["name", "category_id", "active"])
                    .readonly_fields(["owner_id"])
                    .foreign_key("category_id", "categories")
                    .autocomplete_fields(["category_id"])
                    .search_fields(["name"])
                    .list_filter(["active"])
                    .toggles(["active"])
                    .export_fields(["name", "active"])
                    .can_add(false)
                    .can_delete(false)
                    .validator("name", Required)
                    .validator("name", MaxLength(120))
                    .action(Action::new(
                        "archive",
                        "Archive selected products",
                        |action| async move {
                            domain::archive(&action.db, &action.actor, &action.ids_as::<i32>()?)
                                .await?;
                            Ok(ActionResult::success("Products archived."))
                        },
                    )),
            )
            .into_router()
            .map_err(|e| loco_rs::Error::Message(e.to_string()))?;
        // The initializer runs after Loco's parent router layers are constructed.
        // Attach protection directly so every added route and asset is covered.
        let protected = admin.layer(middleware::from_fn_with_state(ctx.clone(), protect));
        Ok(router.nest("/staff", protected).layer(middleware::from_fn(
            crate::admin::middleware::security_headers,
        )))
    }
}
