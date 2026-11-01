use crate::error::AdminError;
use std::collections::BTreeMap;
use tera::{Context, Tera};

const TEMPLATES: &[(&str, &str)] = &[
    ("csrf.html", include_str!("../templates/csrf.html")),
    ("row.html", include_str!("../templates/row.html")),
    (
        "relationship_options.html",
        include_str!("../templates/relationship_options.html"),
    ),
    (
        "list_results.html",
        include_str!("../templates/list_results.html"),
    ),
    (
        "pagination.html",
        include_str!("../templates/pagination.html"),
    ),
    ("notices.html", include_str!("../templates/notices.html")),
    ("layout.html", include_str!("../templates/layout.html")),
    ("index.html", include_str!("../templates/index.html")),
    ("list.html", include_str!("../templates/list.html")),
    ("form.html", include_str!("../templates/form.html")),
    ("delete.html", include_str!("../templates/delete.html")),
    ("macros.html", include_str!("../templates/macros.html")),
];

pub(crate) const ADMIN_JS: &str = include_str!("../static/admin.js");

pub(crate) struct Renderer {
    env: Tera,
}

impl Renderer {
    /// Resolves overrides before validating inheritance and component references.
    pub(crate) fn new(mut env: Tera, overrides: Vec<(String, String)>) -> Result<Self, AdminError> {
        let mut templates: BTreeMap<String, String> = TEMPLATES
            .iter()
            .map(|(name, source)| ((*name).to_owned(), (*source).to_owned()))
            .collect();
        templates.extend(overrides);
        env.add_raw_templates(templates)?;
        Ok(Self { env })
    }

    pub(crate) fn render(&self, name: &str, ctx: Context) -> Result<String, AdminError> {
        Ok(self.env.render(name, &ctx)?)
    }
}

#[cfg(test)]
#[path = "tests/render.rs"]
mod tests;
