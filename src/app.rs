use crate::{
    AccessEntity, AccessPolicy, AdminActor, MutationObserver, Operation, ValidationErrorMapper,
    backend::ColKind,
    entity::{Computed, EntityAdmin},
    error::AdminError,
    render::Renderer,
    router,
    urls::Urls,
};
use axum::Router;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use std::{collections::HashSet, sync::Arc};
use tera::{Context, Tera, context};

/// Builds the admin router.
pub struct AdminApp {
    db: DatabaseConnection,
    title: String,
    prefix: String,
    site_url: String,
    css_url: Option<String>,
    js_url: Option<String>,
    extra_stylesheets: Vec<String>,
    extra_scripts: Vec<String>,
    favicon_url: Option<String>,
    entities: Vec<EntityAdmin>,
    templates: Vec<(String, String)>,
    template_engine: Tera,
    observer: Option<Arc<dyn MutationObserver>>,
    policy: Option<Arc<dyn AccessPolicy>>,
    validation_errors: Option<Arc<dyn ValidationErrorMapper>>,
}

pub(crate) struct AdminState {
    pub title: String,
    pub site_url: String,
    pub css_url: String,
    pub js_url: String,
    pub extra_stylesheets: Vec<String>,
    pub extra_scripts: Vec<String>,
    pub favicon_url: Option<String>,
    pub urls: Urls,
    pub db: DatabaseConnection,
    pub entities: Vec<EntityAdmin>,
    pub renderer: Renderer,
    pub observer: Option<Arc<dyn MutationObserver>>,
    pub policy: Arc<dyn AccessPolicy>,
    pub validation_errors: Option<Arc<dyn ValidationErrorMapper>>,
}

#[derive(Serialize)]
struct NavEntity {
    name: String,
    label: String,
    url: String,
    add_url: Option<String>,
    active: bool,
}

#[derive(Serialize)]
struct NavGroup {
    label: Option<String>,
    entities: Vec<NavEntity>,
}

impl AdminState {
    pub(crate) fn entity(&self, name: &str) -> Option<&EntityAdmin> {
        self.entities.iter().find(|e| e.name == name)
    }

    fn nav(&self, current: &str, actor: &AdminActor) -> Vec<NavGroup> {
        let mut groups: Vec<NavGroup> = Vec::new();
        for entity in &self.entities {
            let identity = AccessEntity {
                name: &entity.name,
                table: entity.backend.table(),
            };
            if self
                .policy
                .scope(actor, identity, &Operation::View)
                .is_none()
            {
                continue;
            }
            let item = NavEntity {
                name: entity.name.clone(),
                label: entity.label.clone(),
                url: self.urls.list(&entity.name),
                add_url: (entity.addable()
                    && self
                        .policy
                        .scope(actor, identity, &Operation::Create)
                        .is_some())
                .then(|| self.urls.add(&entity.name)),
                active: entity.name == current,
            };
            match groups.iter_mut().find(|g| g.label == entity.group) {
                Some(group) => group.entities.push(item),
                None => groups.push(NavGroup {
                    label: entity.group.clone(),
                    entities: vec![item],
                }),
            }
        }
        groups
    }

    /// Values every page needs: titles, asset URLs and the sidebar.
    pub(crate) fn base(&self, current: &str, actor: &AdminActor) -> Context {
        context! {
            admin_title => &(self.title),
            home_url => &(self.urls.home()),
            site_url => &(self.site_url),
            css_url => &(self.css_url),
            js_url => &(self.js_url),
            extra_stylesheets => &self.extra_stylesheets,
            extra_scripts => &self.extra_scripts,
            favicon_url => &(self.favicon_url),
            htmx_url => &self.urls.static_file("vendor/htmx-2.0.11/htmx.min.js"),
            admin_css_url => &self.urls.static_file("admin.css"),
            admin_js_url => &(self.urls.static_file("admin.js")),
            nav => &(self.nav(current, actor)),
            current_entity => &(current),
        }
    }
}

impl AdminApp {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            title: "Admin".to_string(),
            prefix: "/admin".to_string(),
            site_url: "/".to_string(),
            css_url: None,
            js_url: None,
            extra_stylesheets: Vec::new(),
            extra_scripts: Vec::new(),
            favicon_url: None,
            entities: Vec::new(),
            templates: Vec::new(),
            template_engine: Tera::new(),
            observer: None,
            policy: None,
            validation_errors: None,
        }
    }

    /// Required policy. Missing actors and default policy methods deny access.
    pub fn access_policy(mut self, policy: impl AccessPolicy + 'static) -> Self {
        self.policy = Some(Arc::new(policy));
        self
    }

    /// Maps selected model write errors to safe field-level form messages.
    /// Unmapped errors remain generic internal failures.
    pub fn validation_errors(mut self, mapper: impl ValidationErrorMapper + 'static) -> Self {
        self.validation_errors = Some(Arc::new(mapper));
        self
    }

    /// Runs observers in the same transaction as each entity write.
    /// Errors roll back that write and the observer’s database changes.
    /// Bulk deletion commits each row separately. Custom actions manage their own transactions.
    pub fn observe_mutations(mut self, observer: impl MutationObserver + 'static) -> Self {
        self.observer = Some(Arc::new(observer));
        self
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    /// Sets the mount path used for links and redirects. Nest the router at this path.
    pub fn prefix(mut self, prefix: &str) -> Self {
        self.prefix = prefix.trim_end_matches('/').to_string();
        self
    }

    /// Target of the "View site" link.
    pub fn site_url(mut self, url: &str) -> Self {
        self.site_url = url.to_string();
        self
    }

    /// Bootstrap 5.3 stylesheet URL.
    pub fn stylesheet(mut self, url: &str) -> Self {
        self.css_url = Some(url.to_string());
        self
    }

    /// Bootstrap 5.3 bundle script URL.
    pub fn script(mut self, url: &str) -> Self {
        self.js_url = Some(url.to_string());
        self
    }

    /// Adds a stylesheet after the default theme and before template style blocks.
    pub fn extra_stylesheet(mut self, url: &str) -> Self {
        self.extra_stylesheets.push(url.to_owned());
        self
    }

    /// Adds a script after the built-in scripts and before template script blocks.
    pub fn extra_script(mut self, url: &str) -> Self {
        self.extra_scripts.push(url.to_owned());
        self
    }

    /// SVG favicon URL; pages have none when unset.
    pub fn favicon(mut self, url: &str) -> Self {
        self.favicon_url = Some(url.to_string());
        self
    }

    pub fn register(mut self, entity: EntityAdmin) -> Self {
        self.entities.push(entity);
        self
    }

    /// Replaces a named Tera template or partial, or adds a new one.
    /// Overrides are resolved together before validation.
    pub fn template(mut self, name: &str, source: &str) -> Self {
        self.templates.push((name.to_string(), source.to_string()));
        self
    }

    /// Configures filters, functions and tests before the embedded templates are loaded.
    /// Named overrides should be supplied with [`Self::template`].
    pub fn configure_templates(mut self, configure: impl FnOnce(&mut Tera)) -> Self {
        configure(&mut self.template_engine);
        self
    }

    /// Validates registrations and builds the router. Mount it at the configured prefix.
    pub fn into_router(self) -> Result<Router, AdminError> {
        validate(&self.entities)?;
        let policy = self
            .policy
            .ok_or_else(|| AdminError::Config("An access policy is required.".into()))?;
        let urls = Urls::new(&self.prefix);
        let state = Arc::new(AdminState {
            policy,
            validation_errors: self.validation_errors,
            observer: self.observer,
            title: self.title,
            site_url: self.site_url,
            css_url: self
                .css_url
                .unwrap_or_else(|| urls.static_file("vendor/bootstrap-5.3.8/bootstrap.min.css")),
            js_url: self.js_url.unwrap_or_else(|| {
                urls.static_file("vendor/bootstrap-5.3.8/bootstrap.bundle.min.js")
            }),
            extra_stylesheets: self.extra_stylesheets,
            extra_scripts: self.extra_scripts,
            favicon_url: self.favicon_url,
            urls,
            db: self.db,
            entities: self.entities,
            renderer: Renderer::new(self.template_engine, self.templates)?,
        });
        Ok(router::routes(state))
    }
}

fn is_identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn validate(entities: &[EntityAdmin]) -> Result<(), AdminError> {
    validate_entity_names(entities)?;
    for entity in entities {
        validate_entity(entity, entities)?;
    }
    Ok(())
}

fn validate_entity_names(entities: &[EntityAdmin]) -> Result<(), AdminError> {
    let mut names = HashSet::new();
    for entity in entities {
        if !is_identifier(&entity.name) || entity.name.starts_with('_') {
            return Err(config_error(
                entity,
                "name must be letters, digits, '-' or '_'",
            ));
        }
        if !names.insert(entity.name.as_str()) {
            return Err(config_error(entity, "registered twice"));
        }
    }
    Ok(())
}

fn validate_entity(entity: &EntityAdmin, entities: &[EntityAdmin]) -> Result<(), AdminError> {
    validate_computed_columns(entity)?;
    validate_list_fields(entity)?;
    validate_export_fields(entity)?;
    validate_form_fields(entity)?;
    validate_search_and_filters(entity)?;
    validate_ordering(entity)?;
    validate_toggles(entity)?;
    validate_display_column(entity)?;
    validate_options(entity, entities)?;
    validate_inlines(entity, entities)?;
    validate_actions(entity)
}

fn validate_computed_columns(entity: &EntityAdmin) -> Result<(), AdminError> {
    for computed in &entity.computed {
        if !is_identifier(&computed.name) || computed.name.contains('-') {
            return Err(config_error(
                entity,
                format!("bad computed column name {}", computed.name),
            ));
        }
        if entity.column(&computed.name).is_some() {
            return Err(config_error(
                entity,
                format!("computed column {} shadows a column", computed.name),
            ));
        }
    }
    Ok(())
}

fn validate_list_fields(entity: &EntityAdmin) -> Result<(), AdminError> {
    for (names, label) in [
        (&entity.list_display, "list_display"),
        (&entity.readonly, "readonly_fields"),
    ] {
        for name in names {
            let known = entity.column(name).is_some() || entity.computed_column(name).is_some();
            if !known {
                return Err(config_error(
                    entity,
                    format!("{label}: unknown column {name}"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_export_fields(entity: &EntityAdmin) -> Result<(), AdminError> {
    let mut seen = HashSet::new();
    for name in &entity.export_fields {
        if entity.column(name).is_none() {
            return Err(config_error(
                entity,
                format!("export_fields: unknown or computed column {name}"),
            ));
        }
        if !seen.insert(name) {
            return Err(config_error(
                entity,
                format!("export_fields: duplicate column {name}"),
            ));
        }
    }
    Ok(())
}

fn validate_form_fields(entity: &EntityAdmin) -> Result<(), AdminError> {
    let form_names = entity
        .form_fields
        .iter()
        .chain(entity.fieldsets.iter().flat_map(|f| f.fields.iter()));
    for name in form_names {
        let column = entity.column(name).is_some();
        let readonly_computed =
            entity.computed_column(name).is_some() && entity.readonly.contains(name);
        if !column && !readonly_computed {
            return Err(config_error(
                entity,
                format!("fields: unknown column {name}"),
            ));
        }
    }
    Ok(())
}

fn validate_search_and_filters(entity: &EntityAdmin) -> Result<(), AdminError> {
    for name in entity.search_fields.iter().chain(entity.list_filter.iter()) {
        if entity.column(name).is_none() {
            return Err(config_error(
                entity,
                format!("unknown column {name} in search or filter"),
            ));
        }
    }
    Ok(())
}

fn validate_ordering(entity: &EntityAdmin) -> Result<(), AdminError> {
    for name in &entity.ordering {
        let column = name.trim_start_matches('-');
        let is_column = entity.column(column).is_some();
        let is_sql_computed = entity
            .computed_column(column)
            .is_some_and(|c| matches!(c.kind, Computed::Sql(_)));
        if !is_column && !is_sql_computed {
            return Err(config_error(
                entity,
                format!("ordering: cannot sort by {name}"),
            ));
        }
    }
    Ok(())
}

fn validate_toggles(entity: &EntityAdmin) -> Result<(), AdminError> {
    for name in &entity.toggles {
        if entity.column(name).map(|c| c.kind) != Some(ColKind::Bool) {
            return Err(config_error(
                entity,
                format!("toggles: {name} is not a boolean column"),
            ));
        }
    }
    Ok(())
}

fn validate_display_column(entity: &EntityAdmin) -> Result<(), AdminError> {
    if let Some(display) = &entity.display_column
        && entity.column(display).is_none()
    {
        return Err(config_error(
            entity,
            format!("display_column: unknown column {display}"),
        ));
    }
    Ok(())
}

fn validate_options(entity: &EntityAdmin, entities: &[EntityAdmin]) -> Result<(), AdminError> {
    for (name, options) in &entity.options {
        let known = entity.column(name).is_some() || entity.computed_column(name).is_some();
        if !known {
            return Err(config_error(
                entity,
                format!("options set on unknown column {name}"),
            ));
        }
        if let Some(target) = &options.foreign_key {
            let Some(target_entity) = entities.iter().find(|e| &e.name == target) else {
                return Err(config_error(
                    entity,
                    format!("{name} refers to unregistered entity {target}"),
                ));
            };
            if target_entity.single_pk().is_none() {
                return Err(config_error(
                    entity,
                    format!("{name} refers to {target}, which has a composite key"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_inlines(entity: &EntityAdmin, entities: &[EntityAdmin]) -> Result<(), AdminError> {
    for inline in &entity.inlines {
        let Some(child) = entities.iter().find(|e| e.name == inline.child) else {
            return Err(config_error(
                entity,
                format!("inline refers to unregistered entity {}", inline.child),
            ));
        };
        if child.column(&inline.fk_column).is_none() {
            return Err(config_error(
                entity,
                format!("inline: {} has no column {}", child.name, inline.fk_column),
            ));
        }
        for name in &inline.columns {
            let known = child.column(name).is_some() || child.computed_column(name).is_some();
            if !known {
                return Err(config_error(
                    entity,
                    format!("inline: {} has no column {name}", child.name),
                ));
            }
        }
    }
    Ok(())
}

fn validate_actions(entity: &EntityAdmin) -> Result<(), AdminError> {
    let mut action_names = HashSet::new();
    for action in &entity.actions {
        if !is_identifier(&action.name) || action.name.starts_with('_') {
            return Err(config_error(
                entity,
                format!("bad action name {}", action.name),
            ));
        }
        if !action_names.insert(action.name.as_str()) {
            return Err(config_error(
                entity,
                format!("action {} registered twice", action.name),
            ));
        }
    }
    Ok(())
}

fn config_error(entity: &EntityAdmin, message: impl Into<String>) -> AdminError {
    AdminError::Config(format!("{}: {}", entity.name, message.into()))
}
