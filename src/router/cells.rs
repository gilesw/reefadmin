use super::RequestState;
use crate::{
    Operation,
    backend::ColKind,
    entity::{Computed, EntityAdmin, Row},
    error::AdminError,
    sql,
    values::{as_bool, display, key_string, truncate},
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

const CELL_CHARS: usize = 100;

#[derive(Serialize)]
pub(crate) struct Cell {
    pub text: String,
    pub url: Option<String>,
    /// Set for boolean columns, so templates can show a badge.
    pub flag: Option<bool>,
    pub toggle_url: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct RowView {
    pub id: Option<String>,
    pub change_url: Option<String>,
    pub cells: Vec<Cell>,
}

/// The primary key of a row as used in URLs, if the key is a single column.
pub(crate) fn row_id(entity: &EntityAdmin, row: &Row) -> Option<String> {
    let pk = entity.single_pk()?;
    row.get(&pk.name).map(key_string)
}

/// Labels of referenced rows for each foreign-key column in `columns`.
pub(crate) async fn foreign_labels(
    state: &RequestState,
    entity: &EntityAdmin,
    rows: &[Row],
    columns: &[String],
) -> Result<HashMap<String, HashMap<String, String>>, AdminError> {
    let mut out = HashMap::new();
    for column in columns {
        let Some(target) = entity
            .options
            .get(column)
            .and_then(|o| o.foreign_key.as_deref())
            .and_then(|t| state.entity(t))
        else {
            continue;
        };
        let mut keys: Vec<String> = rows
            .iter()
            .filter_map(|r| r.get(column))
            .map(key_string)
            .filter(|k| !k.is_empty())
            .collect();
        keys.sort();
        keys.dedup();
        out.insert(
            column.clone(),
            sql::labels(&state.db, target, &keys, state.read_scope(target)).await?,
        );
    }
    Ok(out)
}

/// Text and link for a model column's value.
pub(crate) fn column_cell(
    state: &RequestState,
    entity: &EntityAdmin,
    column: &str,
    row: &Row,
    labels: &HashMap<String, HashMap<String, String>>,
) -> Cell {
    let meta = entity.column(column);
    let value = row.get(column).cloned().unwrap_or_default();
    let options = entity.options.get(column);
    let key = key_string(&value);
    let mut url = None;
    let text = if let Some(target) = options.and_then(|o| o.foreign_key.as_deref()) {
        if !key.is_empty()
            && labels
                .get(column)
                .is_some_and(|labels| labels.contains_key(&key))
        {
            url = Some(state.urls.change(target, &key));
        }
        labels
            .get(column)
            .and_then(|l| l.get(&key))
            .cloned()
            .unwrap_or(key)
    } else if let Some(choices) = options.and_then(|o| o.choices.as_ref()) {
        choices
            .iter()
            .find(|(v, _)| *v == key)
            .map(|(_, l)| l.clone())
            .unwrap_or(key)
    } else {
        display(meta.map(|m| m.kind), &value)
    };
    let flag = match meta.map(|m| m.kind) {
        Some(ColKind::Bool) => as_bool(&value),
        _ => None,
    };
    Cell {
        text,
        url,
        flag,
        toggle_url: None,
    }
}

/// One scoped key lookup per displayed toggle field, rather than per cell.
async fn toggle_permissions(
    state: &RequestState,
    entity: &EntityAdmin,
    rows: &[Row],
    columns: &[String],
    with_toggles: bool,
) -> Result<HashMap<String, HashSet<String>>, AdminError> {
    let mut permissions = HashMap::new();
    if !with_toggles || !entity.editable() || rows.is_empty() {
        return Ok(permissions);
    }
    let ids: Vec<String> = rows.iter().filter_map(|row| row_id(entity, row)).collect();
    if ids.is_empty() {
        return Ok(permissions);
    }
    for column in columns {
        if !entity.toggles.contains(column) || permissions.contains_key(column) {
            continue;
        }
        // Toggle scopes already intersect View and Update.
        let Ok(scope) = state.scope(entity, &Operation::Toggle(column.clone())) else {
            continue;
        };
        let allowed = sql::permitted_ids(&state.db, entity, &ids, scope).await?;
        permissions.insert(column.clone(), allowed);
    }
    Ok(permissions)
}

/// Table rows for `columns`, resolving foreign keys, choices and computed columns.
pub(crate) async fn render_rows(
    state: &RequestState,
    entity: &EntityAdmin,
    rows: Vec<Row>,
    columns: &[String],
    with_toggles: bool,
) -> Result<Vec<RowView>, AdminError> {
    let labels = foreign_labels(state, entity, &rows, columns).await?;
    let permissions = toggle_permissions(state, entity, &rows, columns, with_toggles).await?;
    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        let id = row_id(entity, &row);
        let mut cells = Vec::with_capacity(columns.len());
        for column in columns {
            let cell = if let Some(computed) = entity.computed_column(column) {
                let text = match &computed.kind {
                    Computed::Sql(_) => display(None, row.get(column).unwrap_or_default()),
                    Computed::Row(f) => f(&row),
                    Computed::Db(f) => f(state.db.clone(), row.clone()).await?,
                };
                Cell {
                    text,
                    url: None,
                    flag: None,
                    toggle_url: None,
                }
            } else {
                let mut cell = column_cell(state, entity, column, &row, &labels);
                if let Some(id) = &id
                    && permissions
                        .get(column)
                        .is_some_and(|allowed| allowed.contains(id))
                {
                    cell.toggle_url = Some(state.urls.toggle(&entity.name, id, column));
                }
                cell
            };
            cells.push(Cell {
                text: truncate(cell.text, CELL_CHARS),
                ..cell
            });
        }
        views.push(RowView {
            change_url: id.as_ref().map(|id| state.urls.change(&entity.name, id)),
            id,
            cells,
        });
    }
    Ok(views)
}
