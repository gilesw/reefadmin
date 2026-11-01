use crate::backend::ColumnMeta;
use crate::{
    action::{Action, BoxFuture},
    backend::{Backend, SeaOrmBackend},
    validator::Validator,
};
use sea_orm::{
    ActiveModelBehavior, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, TryIntoModel,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use std::{collections::HashMap, future::Future, sync::Arc};

/// A row as read from the database, keyed by column name.
pub type Row = Map<String, Value>;

type RowFn = Arc<dyn Fn(&Row) -> String + Send + Sync>;
type DbFn = Arc<dyn Fn(DatabaseConnection, Row) -> BoxFuture<Result<String, DbErr>> + Send + Sync>;

#[derive(Clone)]
pub(crate) enum Computed {
    /// A SQL expression selected alongside the row. It may refer to the entity's table by name.
    Sql(String),
    Row(RowFn),
    Db(DbFn),
}

#[derive(Clone)]
pub(crate) struct ComputedColumn {
    pub name: String,
    pub label: String,
    pub kind: Computed,
}

#[derive(Default)]
pub(crate) struct FieldOptions {
    pub label: Option<String>,
    pub help: Option<String>,
    pub textarea: bool,
    pub choices: Option<Vec<(String, String)>>,
    /// Name of the registered entity this column refers to.
    pub foreign_key: Option<String>,
    pub autocomplete: bool,
    pub validators: Vec<Box<dyn Validator>>,
}

/// A read-only table of child rows shown on the parent's edit page.
pub struct Inline {
    pub(crate) child: String,
    pub(crate) fk_column: String,
    pub(crate) label: Option<String>,
    pub(crate) columns: Vec<String>,
    pub(crate) limit: u64,
}

impl Inline {
    /// `child` is the registered name of the child entity; `fk_column` is its column holding the parent's key.
    pub fn new(child: &str, fk_column: &str) -> Self {
        Self {
            child: child.to_string(),
            fk_column: fk_column.to_string(),
            label: None,
            columns: Vec::new(),
            limit: 50,
        }
    }

    pub fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    /// Columns to show; defaults to the child's list columns.
    pub fn columns<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.columns = columns.into_iter().map(Into::into).collect();
        self
    }

    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = limit.max(1);
        self
    }
}

pub(crate) struct Fieldset {
    pub title: Option<String>,
    pub fields: Vec<String>,
}

/// The admin registration for one SeaORM entity.
pub struct EntityAdmin {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) singular: String,
    pub(crate) group: Option<String>,
    pub(crate) backend: Arc<dyn Backend>,
    pub(crate) display_column: Option<String>,
    pub(crate) list_display: Vec<String>,
    pub(crate) search_fields: Vec<String>,
    pub(crate) search_sql: Vec<String>,
    pub(crate) list_filter: Vec<String>,
    pub(crate) ordering: Vec<String>,
    pub(crate) per_page: u64,
    pub(crate) readonly: Vec<String>,
    pub(crate) form_fields: Vec<String>,
    pub(crate) fieldsets: Vec<Fieldset>,
    pub(crate) options: HashMap<String, FieldOptions>,
    pub(crate) computed: Vec<ComputedColumn>,
    pub(crate) toggles: Vec<String>,
    pub(crate) can_add: bool,
    pub(crate) can_edit: bool,
    pub(crate) can_delete: bool,
    pub(crate) bulk_delete: bool,
    pub(crate) csv_export: bool,
    pub(crate) export_fields: Vec<String>,
    pub(crate) actions: Vec<Action>,
    pub(crate) inlines: Vec<Inline>,
}

fn strings<I, S>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    items.into_iter().map(Into::into).collect()
}

pub(crate) fn default_label(name: &str) -> String {
    let spaced = name.replace(['_', '-'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn default_singular(name: &str) -> String {
    let label = default_label(name).to_lowercase();
    if label.ends_with("ies") {
        format!("{}y", &label[..label.len() - 3])
    } else if label.ends_with('s') && !label.ends_with("ss") {
        label[..label.len() - 1].to_string()
    } else {
        label
    }
}

impl EntityAdmin {
    /// Registers entity `E` under `name`, which becomes its URL segment (for example `/admin/rules`).
    pub fn from_entity<E>(name: &str) -> Self
    where
        E: EntityTrait + Sync,
        E::Model: Serialize + DeserializeOwned + IntoActiveModel<E::ActiveModel> + Send + Sync,
        E::ActiveModel: ActiveModelBehavior + TryIntoModel<E::Model> + Send + Sync,
    {
        Self {
            name: name.to_string(),
            label: default_label(name),
            singular: default_singular(name),
            group: None,
            backend: Arc::new(SeaOrmBackend::<E>::new()),
            display_column: None,
            list_display: Vec::new(),
            search_fields: Vec::new(),
            search_sql: Vec::new(),
            list_filter: Vec::new(),
            ordering: Vec::new(),
            per_page: 50,
            readonly: Vec::new(),
            form_fields: Vec::new(),
            fieldsets: Vec::new(),
            options: HashMap::new(),
            computed: Vec::new(),
            toggles: Vec::new(),
            can_add: true,
            can_edit: true,
            can_delete: true,
            bulk_delete: true,
            csv_export: true,
            export_fields: Vec::new(),
            actions: Vec::new(),
            inlines: Vec::new(),
        }
    }

    /// Plural label used in the sidebar and page titles.
    pub fn label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    /// Singular, lowercase noun used in buttons and messages ("Add rule").
    pub fn singular(mut self, singular: &str) -> Self {
        self.singular = singular.to_string();
        self
    }

    /// Sidebar heading this entity is listed under.
    pub fn group(mut self, group: &str) -> Self {
        self.group = Some(group.to_string());
        self
    }

    /// Column used as the display label when another entity refers to this row.
    pub fn display_column(mut self, column: &str) -> Self {
        self.display_column = Some(column.to_string());
        self
    }

    /// Columns on the list page: model columns or computed column names.
    pub fn list_display<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.list_display = strings(columns);
        self
    }

    /// Model columns matched by the search box (case-insensitive substring).
    pub fn search_fields<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.search_fields = strings(columns);
        self
    }

    /// A SQL expression also matched by the search box, such as a subquery on a related table.
    pub fn search_sql(mut self, expression: &str) -> Self {
        self.search_sql.push(expression.to_string());
        self
    }

    /// Model columns offered as filters in the list page's sidebar.
    pub fn list_filter<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.list_filter = strings(columns);
        self
    }

    /// Default sort, for example `["-created_at", "name"]`. A leading `-` sorts descending.
    pub fn ordering<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.ordering = strings(columns);
        self
    }

    pub fn per_page(mut self, per_page: u64) -> Self {
        self.per_page = per_page.max(1);
        self
    }

    /// Fields shown on the edit page but never written. May include computed column names.
    pub fn readonly_fields<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.readonly = strings(columns);
        self
    }

    /// Fields on the create and edit forms, in order. Defaults to every column.
    pub fn fields<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.form_fields = strings(columns);
        self
    }

    /// Adds a group of form fields with an optional title.
    /// If you define any fieldsets, they replace the [`Self::fields`] layout.
    pub fn fieldset<I, S>(mut self, title: Option<&str>, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fieldsets.push(Fieldset {
            title: title.map(str::to_string),
            fields: strings(columns),
        });
        self
    }

    fn option(&mut self, column: &str) -> &mut FieldOptions {
        self.options.entry(column.to_string()).or_default()
    }

    pub fn field_label(mut self, column: &str, label: &str) -> Self {
        self.option(column).label = Some(label.to_string());
        self
    }

    pub fn help_text(mut self, column: &str, text: &str) -> Self {
        self.option(column).help = Some(text.to_string());
        self
    }

    pub fn textarea(mut self, column: &str) -> Self {
        self.option(column).textarea = true;
        self
    }

    /// Restricts a column to fixed `(value, label)` pairs, shown in a selection menu.
    pub fn choices<I, V, L>(mut self, column: &str, choices: I) -> Self
    where
        I: IntoIterator<Item = (V, L)>,
        V: Into<String>,
        L: Into<String>,
    {
        self.option(column).choices = Some(
            choices
                .into_iter()
                .map(|(v, l)| (v.into(), l.into()))
                .collect(),
        );
        self
    }

    /// Marks `column` as referring to the primary key of the registered entity `target`.
    pub fn foreign_key(mut self, column: &str, target: &str) -> Self {
        self.option(column).foreign_key = Some(target.to_string());
        self
    }

    /// Edits these foreign-key columns with a search box instead of a selection menu.
    pub fn autocomplete_fields<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for column in strings(columns) {
            self.option(&column).autocomplete = true;
        }
        self
    }

    pub fn validator(mut self, column: &str, validator: impl Validator + 'static) -> Self {
        self.option(column).validators.push(Box::new(validator));
        self
    }

    /// A list column computed in SQL, for example
    /// `"(SELECT COUNT(*) FROM rules WHERE rules.user_id = users.id)"`. It can be sorted.
    pub fn computed_sql(mut self, name: &str, label: &str, expression: &str) -> Self {
        self.computed.push(ComputedColumn {
            name: name.to_string(),
            label: label.to_string(),
            kind: Computed::Sql(expression.to_string()),
        });
        self
    }

    /// A list column computed from the row.
    pub fn computed<F>(mut self, name: &str, label: &str, f: F) -> Self
    where
        F: Fn(&Row) -> String + Send + Sync + 'static,
    {
        self.computed.push(ComputedColumn {
            name: name.to_string(),
            label: label.to_string(),
            kind: Computed::Row(Arc::new(f)),
        });
        self
    }

    /// A list column computed with a database query per row.
    pub fn computed_async<F, Fut>(mut self, name: &str, label: &str, f: F) -> Self
    where
        F: Fn(DatabaseConnection, Row) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String, DbErr>> + Send + 'static,
    {
        self.computed.push(ComputedColumn {
            name: name.to_string(),
            label: label.to_string(),
            kind: Computed::Db(Arc::new(move |db, row| Box::pin(f(db, row)))),
        });
        self
    }

    /// Boolean columns that get an on/off button on the list page.
    pub fn toggles<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.toggles = strings(columns);
        self
    }

    pub fn can_add(mut self, allowed: bool) -> Self {
        self.can_add = allowed;
        self
    }

    pub fn can_edit(mut self, allowed: bool) -> Self {
        self.can_edit = allowed;
        self
    }

    pub fn can_delete(mut self, allowed: bool) -> Self {
        self.can_delete = allowed;
        self
    }

    pub fn bulk_delete(mut self, enabled: bool) -> Self {
        self.bulk_delete = enabled;
        self
    }

    /// Ordered database columns included in CSV exports. No fields means no export.
    /// Independent of list, form and read-only fields; computed fields are unsupported.
    pub fn export_fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.export_fields = strings(fields);
        self
    }

    pub(crate) fn exportable(&self) -> bool {
        self.csv_export && !self.export_fields.is_empty()
    }

    /// Enables or disables configured exports. An explicit field allowlist is also required.
    pub fn csv_export(mut self, enabled: bool) -> Self {
        self.csv_export = enabled;
        self
    }

    pub fn action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }

    pub fn inline(mut self, inline: Inline) -> Self {
        self.inlines.push(inline);
        self
    }

    pub(crate) fn column(&self, name: &str) -> Option<&ColumnMeta> {
        self.backend.columns().iter().find(|c| c.name == name)
    }

    pub(crate) fn computed_column(&self, name: &str) -> Option<&ComputedColumn> {
        self.computed.iter().find(|c| c.name == name)
    }

    /// The single primary key column, if the key is not composite.
    pub(crate) fn single_pk(&self) -> Option<&ColumnMeta> {
        match self.backend.primary_key() {
            [pk] => self.column(pk),
            _ => None,
        }
    }

    pub(crate) fn label_for(&self, name: &str) -> String {
        if let Some(label) = self.options.get(name).and_then(|o| o.label.clone()) {
            return label;
        }
        if let Some(computed) = self.computed_column(name) {
            return computed.label.clone();
        }
        let base = name.strip_suffix("_id").unwrap_or(name);
        default_label(base)
    }

    pub(crate) fn list_columns(&self) -> Vec<String> {
        if self.list_display.is_empty() {
            self.backend
                .columns()
                .iter()
                .take(8)
                .map(|c| c.name.clone())
                .collect()
        } else {
            self.list_display.clone()
        }
    }

    pub(crate) fn display_column_name(&self) -> Option<&str> {
        self.display_column
            .as_deref()
            .or_else(|| self.single_pk().map(|c| c.name.as_str()))
    }

    pub(crate) fn editable(&self) -> bool {
        self.can_edit && self.single_pk().is_some()
    }

    pub(crate) fn addable(&self) -> bool {
        self.can_add && self.single_pk().is_some()
    }

    pub(crate) fn deletable(&self) -> bool {
        self.can_delete && self.single_pk().is_some()
    }
}

#[cfg(test)]
#[path = "tests/entity.rs"]
mod tests;
