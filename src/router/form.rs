use super::{
    Failure, Flash, PageResult, RequestState,
    cells::{column_cell, foreign_labels, render_rows},
    form_map, html,
    list::{find, id_of},
};
use crate::{
    Operation,
    backend::{ColKind, ColumnMeta},
    entity::{EntityAdmin, Row},
    sql::{self, ListSpec},
    values,
    values::{eq_condition, form_to_json, json_to_input, key_string},
};
use axum::{
    body::Bytes,
    extract::{Path, RawQuery},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::slice;
use tera::context;

const FK_OPTIONS: u64 = 1000;
const NONE_LABEL: &str = "---------";

#[derive(Serialize)]
struct FormField {
    name: String,
    label: String,
    widget: &'static str,
    value: String,
    checked: bool,
    display: String,
    display_url: Option<String>,
    options: Vec<(String, String)>,
    step: Option<&'static str>,
    required: bool,
    help: Option<String>,
    error: Option<String>,
    autocomplete_url: Option<String>,
}

#[derive(Serialize)]
struct FieldsetView {
    title: Option<String>,
    fields: Vec<FormField>,
}

#[derive(Serialize)]
struct InlineView {
    label: String,
    headers: Vec<String>,
    rows: Vec<super::cells::RowView>,
    add_url: Option<String>,
    list_url: String,
    total: u64,
}

enum Mode<'a> {
    Add,
    Change { id: &'a str, row: &'a Row },
}

impl Mode<'_> {
    fn is_add(&self) -> bool {
        matches!(self, Mode::Add)
    }
}

fn is_readonly(entity: &EntityAdmin, name: &str, mode: &Mode) -> bool {
    entity.readonly.iter().any(|r| r == name)
        || entity.computed_column(name).is_some()
        || match mode {
            Mode::Add => false,
            Mode::Change { .. } => {
                !entity.editable() || entity.backend.primary_key().iter().any(|pk| pk == name)
            }
        }
}

/// Fieldsets of the form for this mode, as `(title, field names)`.
fn layout(entity: &EntityAdmin, mode: &Mode) -> Vec<(Option<String>, Vec<String>)> {
    let sets: Vec<(Option<String>, Vec<String>)> = if !entity.fieldsets.is_empty() {
        entity
            .fieldsets
            .iter()
            .map(|f| (f.title.clone(), f.fields.clone()))
            .collect()
    } else if !entity.form_fields.is_empty() {
        vec![(None, entity.form_fields.clone())]
    } else {
        let mut names: Vec<String> = entity
            .backend
            .columns()
            .iter()
            .map(|c| c.name.clone())
            .collect();
        for name in &entity.readonly {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        vec![(None, names)]
    };
    let auto_pk = entity.backend.pk_auto_increment();
    sets.into_iter()
        .map(|(title, names)| {
            let names = names
                .into_iter()
                .filter(|name| match mode {
                    Mode::Add => {
                        !is_readonly(entity, name, mode)
                            && !(auto_pk && entity.backend.primary_key().contains(name))
                    }
                    Mode::Change { .. } => true,
                })
                .collect();
            (title, names)
        })
        .filter(|(_, names): &(Option<String>, Vec<String>)| !names.is_empty())
        .collect()
}

/// Names of the fields written by a submit in this mode.
fn writable(entity: &EntityAdmin, mode: &Mode) -> Vec<String> {
    layout(entity, mode)
        .into_iter()
        .flat_map(|(_, names)| names)
        .filter(|name| entity.column(name).is_some() && !is_readonly(entity, name, mode))
        .collect()
}

impl FormField {
    fn new(entity: &EntityAdmin, name: &str, error: Option<String>) -> Self {
        Self {
            name: name.to_owned(),
            label: entity.label_for(name),
            widget: "readonly",
            value: String::new(),
            checked: false,
            display: String::new(),
            display_url: None,
            options: Vec::new(),
            step: None,
            required: false,
            help: entity
                .options
                .get(name)
                .and_then(|options| options.help.clone()),
            error,
            autocomplete_url: None,
        }
    }

    fn select(&mut self, nullable: bool, choices: impl IntoIterator<Item = (String, String)>) {
        self.widget = "select";
        self.options = nullable
            .then(|| (String::new(), NONE_LABEL.to_owned()))
            .into_iter()
            .chain(choices)
            .collect();
    }

    fn scalar(&mut self, kind: ColKind, textarea: bool) {
        (self.widget, self.step) = match kind {
            ColKind::Bool => ("checkbox", None),
            ColKind::Integer => ("number", Some("1")),
            ColKind::Float | ColKind::Decimal => ("number", Some("any")),
            ColKind::Date => ("date", None),
            ColKind::DateTime | ColKind::DateTimeTz => ("datetime-local", Some("1")),
            ColKind::Time => ("time", None),
            ColKind::Json | ColKind::Array => ("json", None),
            ColKind::LongText => ("textarea", None),
            _ if textarea => ("textarea", None),
            _ => ("text", None),
        };
        if kind == ColKind::Bool {
            self.checked = values::parse_bool(&self.value).unwrap_or(false);
        }
        if kind == ColKind::DateTimeTz && self.help.is_none() {
            self.help = Some("Time in UTC.".to_owned());
        }
    }
}

/// Resolve saved or submitted relationship values using the target's view scope.
async fn relationship_widget(
    state: &RequestState,
    target: &EntityAdmin,
    meta: &ColumnMeta,
    autocomplete: bool,
    field: &mut FormField,
) -> Result<(), Failure> {
    if autocomplete {
        field.widget = "autocomplete";
        field.autocomplete_url = Some(state.urls.autocomplete(&target.name));
        if !field.value.is_empty() {
            let keys = [field.value.clone()];
            field.display = sql::labels(&state.db, target, &keys, state.read_scope(target))
                .await?
                .remove(&field.value)
                .unwrap_or_default();
            field.display_url =
                (!field.display.is_empty()).then(|| state.urls.change(&target.name, &field.value));
        }
        return Ok(());
    }
    let mut choices = sql::form_options(
        &state.db,
        target,
        &field.value,
        FK_OPTIONS,
        state.read_scope(target),
    )
    .await?;
    // Preserve an existing hidden reference during unrelated edits, without its label/link.
    if !field.value.is_empty() && !choices.iter().any(|(id, _)| id == &field.value) {
        choices.push((field.value.clone(), field.value.clone()));
    }
    field.select(meta.nullable, choices);
    Ok(())
}

/// Explicit choices take precedence over relationships, enum choices and scalar widgets.
async fn configure_widget(
    state: &RequestState,
    entity: &EntityAdmin,
    meta: &ColumnMeta,
    field: &mut FormField,
) -> Result<(), Failure> {
    let options = entity.options.get(&field.name);
    if let Some(choices) = options.and_then(|options| options.choices.as_ref()) {
        field.select(meta.nullable, choices.iter().cloned());
    } else if let Some(target) = options
        .and_then(|options| options.foreign_key.as_deref())
        .and_then(|name| state.entity(name))
    {
        relationship_widget(
            state,
            target,
            meta,
            options.is_some_and(|options| options.autocomplete),
            field,
        )
        .await?;
    } else if !meta.enum_values.is_empty() {
        field.select(
            meta.nullable,
            meta.enum_values
                .iter()
                .map(|value| (value.clone(), value.clone())),
        );
    } else {
        field.scalar(meta.kind, options.is_some_and(|options| options.textarea));
    }
    Ok(())
}

async fn computed_field_values(
    state: &RequestState,
    entity: &EntityAdmin,
    row: Option<&Row>,
    names: &[String],
) -> Result<HashMap<String, String>, Failure> {
    let Some(row) = row else {
        return Ok(HashMap::new());
    };
    let computed_names: Vec<String> = names
        .iter()
        .filter(|name| entity.computed_column(name).is_some())
        .cloned()
        .collect();
    if computed_names.is_empty() {
        return Ok(HashMap::new());
    }
    let views = render_rows(state, entity, vec![row.clone()], &computed_names, false).await?;
    Ok(computed_names
        .into_iter()
        .zip(views.into_iter().flat_map(|view| view.cells))
        .map(|(name, cell)| (name, cell.text))
        .collect())
}

async fn build_fields(
    state: &RequestState,
    entity: &EntityAdmin,
    mode: &Mode<'_>,
    values: &HashMap<String, String>,
    errors: &HashMap<String, String>,
) -> Result<Vec<FieldsetView>, Failure> {
    let row = match mode {
        Mode::Change { row, .. } => Some(*row),
        Mode::Add => None,
    };
    let fieldsets = layout(entity, mode);
    let names: Vec<String> = fieldsets
        .iter()
        .flat_map(|(_, names)| names.iter().cloned())
        .collect();
    let labels = match row {
        Some(row) => foreign_labels(state, entity, slice::from_ref(row), &names).await?,
        None => HashMap::new(),
    };
    let computed = computed_field_values(state, entity, row, &names).await?;
    let permitted = match mode {
        Mode::Change { id, .. } => state.permits_row(entity, id, &Operation::Update).await?,
        Mode::Add => true,
    };
    let mut sets = Vec::new();
    for (title, names) in fieldsets {
        let mut fields = Vec::new();
        for name in names {
            let mut field = FormField::new(entity, &name, errors.get(&name).cloned());
            match entity.column(&name) {
                None => field.display = computed.get(&name).cloned().unwrap_or_default(),
                Some(_) if is_readonly(entity, &name, mode) || !permitted => {
                    if let Some(row) = row {
                        let cell = column_cell(state, entity, &name, row, &labels);
                        field.display = cell.text;
                        field.display_url = cell.url;
                    }
                }
                Some(meta) => {
                    field.value = values.get(&name).cloned().unwrap_or_else(|| {
                        row.and_then(|row| row.get(&name))
                            .map(|value| json_to_input(meta.kind, value))
                            .unwrap_or_default()
                    });
                    field.required =
                        !meta.nullable && !meta.kind.is_textual() && meta.kind != ColKind::Bool;
                    configure_widget(state, entity, meta, &mut field).await?;
                }
            }
            fields.push(field);
        }
        sets.push(FieldsetView { title, fields });
    }
    Ok(sets)
}

fn query_pair(key: &str, value: &str) -> String {
    form_urlencoded::Serializer::new(String::new())
        .append_pair(key, value)
        .finish()
}

async fn inline_views(
    state: &RequestState,
    entity: &EntityAdmin,
    id: &str,
) -> Result<Vec<InlineView>, Failure> {
    let mut views = Vec::new();
    for inline in &entity.inlines {
        let Some(child) = state.entity(&inline.child) else {
            continue;
        };
        if state.scope(child, &Operation::View).is_err() {
            continue;
        }
        let Some(fk) = child.column(&inline.fk_column) else {
            continue;
        };
        let Ok(condition) = eq_condition(child.backend.table(), fk, id) else {
            continue;
        };
        let spec = ListSpec {
            scope: state.read_scope(child),
            search: String::new(),
            conditions: vec![condition],
            order: child
                .ordering
                .iter()
                .map(|o| match o.strip_prefix('-') {
                    Some(c) => (c.to_string(), true),
                    None => (o.clone(), false),
                })
                .collect(),
            limit: inline.limit,
            offset: 0,
        };
        let total = sql::count(&state.db, child, &spec).await?;
        let rows = sql::fetch(&state.db, child, &spec).await?;
        let columns: Vec<String> = if inline.columns.is_empty() {
            child
                .list_columns()
                .into_iter()
                .filter(|c| c != &inline.fk_column)
                .collect()
        } else {
            inline.columns.clone()
        };
        let add_url =
            (child.addable() && state.scope(child, &Operation::Create).is_ok()).then(|| {
                format!(
                    "{}?{}",
                    state.urls.add(&child.name),
                    query_pair(&inline.fk_column, id)
                )
            });
        let list_url = if child.list_filter.contains(&inline.fk_column) {
            let query = query_pair(&format!("f_{}", inline.fk_column), id);
            state.urls.list_with(&child.name, &query)
        } else {
            state.urls.list(&child.name)
        };
        views.push(InlineView {
            label: inline.label.clone().unwrap_or_else(|| child.label.clone()),
            headers: columns.iter().map(|c| child.label_for(c)).collect(),
            rows: render_rows(state, child, rows, &columns, false).await?,
            add_url,
            list_url,
            total,
        });
    }
    Ok(views)
}

async fn render_form(
    state: &RequestState,
    entity: &EntityAdmin,
    mode: Mode<'_>,
    values: &HashMap<String, String>,
    errors: &HashMap<String, String>,
    flash: Option<Flash>,
    status: StatusCode,
) -> PageResult {
    let fieldsets = build_fields(state, entity, &mode, values, errors).await?;
    let (title, action, delete_url, inlines, object_label) = match &mode {
        Mode::Add => (
            format!("Add {}", entity.singular),
            state.urls.add(&entity.name),
            None,
            Vec::new(),
            String::new(),
        ),
        Mode::Change { id, row } => (
            if entity.editable() && state.permits_row(entity, id, &Operation::Update).await? {
                format!("Change {}", entity.singular)
            } else {
                format!("View {}", entity.singular)
            },
            state.urls.change(&entity.name, id),
            (entity.deletable() && state.permits_row(entity, id, &Operation::Delete).await?)
                .then(|| state.urls.delete(&entity.name, id)),
            inline_views(state, entity, id).await?,
            object_label(entity, row, id),
        ),
    };
    let can_save = match &mode {
        Mode::Add => entity.addable(),
        Mode::Change { id, .. } => {
            entity.editable() && state.permits_row(entity, id, &Operation::Update).await?
        }
    };
    let body = state.renderer.render("form.html", {
        let mut ctx = state.base(&entity.name);
        ctx.extend(context! {
        entity_name => &(entity.name),
        entity_label => &(entity.label),
        singular => &(entity.singular),
        list_url => &(state.urls.list(&entity.name)),
        title,
        object_label,
        action,
        delete_url,
        fieldsets,
        inlines,
        can_save,
        can_add_another => &(entity.addable() && state.scope(entity, &Operation::Create).is_ok()),
        is_add => &(mode.is_add()),
        form_error => &(errors.get("__all__")),
        flash,
        });
        ctx
    })?;
    Ok(html(status, body))
}

fn object_label(entity: &EntityAdmin, row: &Row, id: &str) -> String {
    entity
        .display_column
        .as_deref()
        .and_then(|c| row.get(c))
        .map(key_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| id.to_string())
}

/// Converts and validates submitted fields; returns column values or per-field errors.
fn collect(
    entity: &EntityAdmin,
    mode: &Mode,
    submitted: &HashMap<String, String>,
) -> Result<Map<String, Value>, HashMap<String, String>> {
    let mut data = Map::new();
    let mut errors = HashMap::new();
    for name in writable(entity, mode) {
        let Some(meta) = entity.column(&name) else {
            continue;
        };
        let raw = submitted.get(&name).map(String::as_str).unwrap_or_default();
        let options = entity.options.get(&name);
        if let Some(options) = options {
            if let Some(message) = options
                .validators
                .iter()
                .find_map(|v| v.validate(raw).err())
            {
                errors.insert(name, message);
                continue;
            }
            if let Some(choices) = &options.choices {
                let blank_ok = raw.is_empty() && meta.nullable;
                if !blank_ok && !choices.iter().any(|(v, _)| v == raw) {
                    errors.insert(name, "Choose one of the listed options.".to_string());
                    continue;
                }
            }
        }
        match form_to_json(meta, raw) {
            Ok(value) => {
                data.insert(name, value);
            }
            Err(message) => {
                errors.insert(name, message);
            }
        }
    }
    if errors.is_empty() {
        Ok(data)
    } else {
        Err(errors)
    }
}

fn submit_redirect(
    state: &RequestState,
    entity: &EntityAdmin,
    form: &HashMap<String, String>,
    id: Option<&str>,
    notice: &str,
) -> Response {
    let target = if form.contains_key("_continue")
        && let Some(id) = id
    {
        state.urls.change(&entity.name, id)
    } else if form.contains_key("_addanother") && entity.addable() {
        state.urls.add(&entity.name)
    } else {
        state.urls.list(&entity.name)
    };
    Redirect::to(&format!("{target}?notice={notice}")).into_response()
}

pub(super) async fn add_page(
    state: RequestState,
    Path(entity): Path<String>,
    RawQuery(query): RawQuery,
) -> PageResult {
    let entity = find(&state, &entity)?;
    state.scope(entity, &Operation::Create)?;
    if !entity.addable() {
        return Err(Failure::Forbidden("Adding records is disabled."));
    }
    let initial: HashMap<String, String> = query
        .map(|q| form_map(q.as_bytes()))
        .unwrap_or_default()
        .into_iter()
        .filter(|(k, _)| entity.column(k).is_some())
        .collect();
    render_form(
        &state,
        entity,
        Mode::Add,
        &initial,
        &HashMap::new(),
        None,
        StatusCode::OK,
    )
    .await
}

pub(super) async fn add_submit(
    state: RequestState,
    Path(entity): Path<String>,
    body: Bytes,
) -> PageResult {
    let entity = find(&state, &entity)?;
    state.scope(entity, &Operation::Create)?;
    if !entity.addable() {
        return Err(Failure::Forbidden("Adding records is disabled."));
    }
    let form = form_map(&body);
    let data = match collect(entity, &Mode::Add, &form) {
        Ok(data) => data,
        Err(errors) => {
            return render_form(
                &state,
                entity,
                Mode::Add,
                &form,
                &errors,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await;
        }
    };
    match entity
        .backend
        .insert(
            &state.db,
            data,
            state.observer.as_deref(),
            &state.write_access(entity, Operation::Create)?,
        )
        .await
    {
        Ok(row) => {
            let id = id_of(entity, &row);
            Ok(submit_redirect(
                &state,
                entity,
                &form,
                id.as_deref(),
                "added",
            ))
        }
        Err(err) => Err(err.into()),
    }
}

pub(super) async fn change_page(
    state: RequestState,
    Path((entity, id)): Path<(String, String)>,
    RawQuery(query): RawQuery,
) -> PageResult {
    let entity = find(&state, &entity)?;
    state.scope(entity, &Operation::View)?;
    let row = state.get(entity, &id, &Operation::View).await?;
    let notice = query
        .map(|q| form_map(q.as_bytes()))
        .and_then(|mut q| q.remove("notice"));
    let flash = match notice.as_deref() {
        Some("added") => Some(Flash::success(format!(
            "The {} was added.",
            entity.singular
        ))),
        Some("changed") => Some(Flash::success(format!(
            "The {} was changed.",
            entity.singular
        ))),
        _ => None,
    };
    let mode = Mode::Change { id: &id, row: &row };
    render_form(
        &state,
        entity,
        mode,
        &HashMap::new(),
        &HashMap::new(),
        flash,
        StatusCode::OK,
    )
    .await
}

pub(super) async fn change_submit(
    state: RequestState,
    Path((entity, id)): Path<(String, String)>,
    body: Bytes,
) -> PageResult {
    let entity = find(&state, &entity)?;
    state.scope(entity, &Operation::Update)?;
    if !entity.editable() {
        return Err(Failure::Forbidden("Editing records is disabled."));
    }
    let row = state.get(entity, &id, &Operation::Update).await?;
    let form = form_map(&body);
    let mode = Mode::Change { id: &id, row: &row };
    let mut data = match collect(entity, &mode, &form) {
        Ok(data) => data,
        Err(errors) => {
            return render_form(
                &state,
                entity,
                mode,
                &form,
                &errors,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await;
        }
    };
    if !data.is_empty() {
        for pk in entity.backend.primary_key() {
            data.insert(pk.clone(), row.get(pk).cloned().unwrap_or_default());
        }
        if let Err(err) = entity
            .backend
            .update(
                &state.db,
                data,
                state.observer.as_deref(),
                &state.write_access(entity, Operation::Update)?,
            )
            .await
        {
            return Err(err.into());
        }
    }
    Ok(submit_redirect(&state, entity, &form, Some(&id), "changed"))
}

pub(super) async fn delete_page(
    state: RequestState,
    Path((entity, id)): Path<(String, String)>,
) -> PageResult {
    let entity = find(&state, &entity)?;
    state.scope(entity, &Operation::Delete)?;
    if !entity.deletable() {
        return Err(Failure::Forbidden("Deleting records is disabled."));
    }
    let row = state.get(entity, &id, &Operation::Delete).await?;
    let body = state.renderer.render("delete.html", {
        let mut ctx = state.base(&entity.name);
        ctx.extend(context! {
        entity_label => &(entity.label),
        singular => &(entity.singular),
        list_url => &(state.urls.list(&entity.name)),
        change_url => &(state.urls.change(&entity.name, &id)),
        action => &(state.urls.delete(&entity.name, &id)),
        object_label => &(object_label(entity, &row, &id)),
        });
        ctx
    })?;
    Ok(html(StatusCode::OK, body))
}

pub(super) async fn delete_submit(
    state: RequestState,
    Path((entity, id)): Path<(String, String)>,
) -> PageResult {
    let entity = find(&state, &entity)?;
    state.scope(entity, &Operation::Delete)?;
    if !entity.deletable() {
        return Err(Failure::Forbidden("Deleting records is disabled."));
    }
    state.get(entity, &id, &Operation::Delete).await?;
    let condition = sql::pk_condition(entity, &id).ok_or(Failure::NotFound)?;
    if !entity
        .backend
        .delete(
            &state.db,
            condition,
            state.observer.as_deref(),
            &state.write_access(entity, Operation::Delete)?,
        )
        .await?
    {
        return Err(Failure::NotFound);
    }
    Ok(Redirect::to(&format!("{}?notice=deleted", state.urls.list(&entity.name))).into_response())
}
