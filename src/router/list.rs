use super::{
    Failure, Flash, PageResult, RequestState,
    cells::{render_rows, row_id},
    form_map, form_values, html,
};
use crate::{
    AdminError, Operation,
    action::{ActionContext, ActionResult},
    backend::ColKind,
    entity::{Computed, EntityAdmin, Row},
    sql::{self, ListSpec},
    urls::ListParams,
    values,
    values::{display, eq_condition, in_condition, key_string},
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, RawQuery},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use csv::{Terminator, WriterBuilder};
use sea_orm::Condition;
use sea_orm::sea_query::{Expr, ExprTrait};
use serde::{Deserialize, Serialize};
use std::iter;
use tera::context;

const NULL_FILTER: &str = "__null__";
const DELETE_ACTION: &str = "__delete__";
const CSV_ACTION: &str = "__csv__";
const FILTER_VALUES: u64 = 100;
const CSV_ROWS: u64 = 100_000;

#[derive(Serialize)]
struct Header {
    label: String,
    sort_url: Option<String>,
    sorted: Option<&'static str>,
}

#[derive(Serialize)]
struct FilterOption {
    label: String,
    url: String,
    active: bool,
}

#[derive(Serialize)]
struct FilterView {
    title: String,
    options: Vec<FilterOption>,
}

#[derive(Serialize)]
struct ActionOption {
    value: String,
    label: String,
    confirm: Option<String>,
}

#[derive(Serialize)]
struct PageLink {
    number: u64,
    url: String,
    active: bool,
}

pub(super) fn find<'a>(state: &'a RequestState, name: &str) -> Result<&'a EntityAdmin, Failure> {
    state.entity(name).ok_or(Failure::NotFound)
}

/// Sort columns for a list request: the requested one if sortable, else the entity's default.
fn order_for(entity: &EntityAdmin, params: &ListParams) -> Vec<(String, bool)> {
    let sortable = |name: &str| {
        entity.column(name).is_some()
            || entity
                .computed_column(name)
                .is_some_and(|c| matches!(c.kind, Computed::Sql(_)))
    };
    if let Some((column, desc)) = params.order_column()
        && sortable(&column)
    {
        return vec![(column, desc)];
    }
    entity
        .ordering
        .iter()
        .map(|o| match o.strip_prefix('-') {
            Some(column) => (column.to_string(), true),
            None => (o.clone(), false),
        })
        .collect()
}

/// Conditions for the active filters; unknown or unparsable filters are ignored.
fn filter_conditions(entity: &EntityAdmin, params: &ListParams) -> Vec<Expr> {
    let table = entity.backend.table();
    params
        .filters
        .iter()
        .filter(|(column, _)| entity.list_filter.contains(column))
        .filter_map(|(column, value)| {
            let meta = entity.column(column)?;
            if value == NULL_FILTER {
                Some(values::column_expr(table, column).is_null())
            } else {
                eq_condition(table, meta, value).ok()
            }
        })
        .collect()
}

fn list_spec(
    entity: &EntityAdmin,
    params: &ListParams,
    limit: u64,
    offset: u64,
    scope: Condition,
) -> ListSpec {
    ListSpec {
        scope,
        search: params.search.clone(),
        conditions: filter_conditions(entity, params),
        order: order_for(entity, params),
        limit,
        offset,
    }
}

async fn filter_view(
    state: &RequestState,
    entity: &EntityAdmin,
    column: &str,
    params: &ListParams,
) -> Result<FilterView, Failure> {
    let Some(meta) = entity.column(column) else {
        return Err(Failure::NotFound);
    };
    let options = entity.options.get(column);
    let pairs: Vec<(String, String)> =
        if let Some(choices) = options.and_then(|o| o.choices.clone()) {
            choices.into_iter().map(|(v, l)| (l, v)).collect()
        } else if meta.kind == ColKind::Bool {
            vec![("Yes".into(), "true".into()), ("No".into(), "false".into())]
        } else if !meta.enum_values.is_empty() {
            meta.enum_values
                .iter()
                .map(|v| (v.clone(), v.clone()))
                .collect()
        } else {
            let values = sql::distinct_values(
                &state.db,
                entity,
                column,
                FILTER_VALUES,
                state.read_scope(entity),
            )
            .await?;
            let keys: Vec<String> = values.iter().map(key_string).collect();
            let target = options
                .and_then(|o| o.foreign_key.as_deref())
                .and_then(|t| state.entity(t));
            match target {
                Some(target) => {
                    let labels =
                        sql::labels(&state.db, target, &keys, state.read_scope(target)).await?;
                    let mut pairs: Vec<(String, String)> = keys
                        .into_iter()
                        .filter_map(|k| labels.get(&k).cloned().map(|label| (label, k)))
                        .collect();
                    pairs.sort_by_key(|(label, _)| label.to_lowercase());
                    pairs
                }
                None => values
                    .iter()
                    .map(|v| (display(Some(meta.kind), v), key_string(v)))
                    .collect(),
            }
        };
    let current = params.filters.get(column);
    let mut items = vec![FilterOption {
        label: "All".into(),
        url: state
            .urls
            .list_with(&entity.name, &params.with_filter(column, None).encode()),
        active: current.is_none(),
    }];
    let empty = meta
        .nullable
        .then(|| ("Empty".to_string(), NULL_FILTER.to_string()));
    for (label, value) in pairs.into_iter().chain(empty) {
        items.push(FilterOption {
            url: state.urls.list_with(
                &entity.name,
                &params.with_filter(column, Some(&value)).encode(),
            ),
            active: current == Some(&value),
            label,
        });
    }
    Ok(FilterView {
        title: entity.label_for(column),
        options: items,
    })
}

fn action_options(state: &RequestState, entity: &EntityAdmin) -> Vec<ActionOption> {
    let mut options = Vec::new();
    if entity.deletable() && entity.bulk_delete && state.scope(entity, &Operation::Delete).is_ok() {
        options.push(ActionOption {
            value: DELETE_ACTION.into(),
            label: format!("Delete selected {}", entity.label.to_lowercase()),
            confirm: Some(format!(
                "Delete the selected {}? This cannot be undone.",
                entity.label.to_lowercase()
            )),
        });
    }
    if entity.exportable() && state.scope(entity, &Operation::Export).is_ok() {
        options.push(ActionOption {
            value: CSV_ACTION.into(),
            label: "Export to CSV".into(),
            confirm: None,
        });
    }
    for action in &entity.actions {
        if state
            .scope(entity, &Operation::Action(action.name.clone()))
            .is_err()
        {
            continue;
        }
        options.push(ActionOption {
            value: action.name.clone(),
            label: action.label.clone(),
            confirm: action.confirm.clone(),
        });
    }
    options
}

fn notice_text(entity: &EntityAdmin, notice: Option<&str>) -> Option<Flash> {
    let text = match notice? {
        "added" => format!("The {} was added.", entity.singular),
        "changed" => format!("The {} was changed.", entity.singular),
        "deleted" => format!("The {} was deleted.", entity.singular),
        _ => return None,
    };
    Some(Flash::success(text))
}

fn list_headers(
    state: &RequestState,
    entity: &EntityAdmin,
    params: &ListParams,
    columns: &[String],
) -> Vec<Header> {
    let current_order = order_for(entity, params);
    columns
        .iter()
        .map(|column| {
            let sortable = entity.column(column).is_some()
                || entity
                    .computed_column(column)
                    .is_some_and(|c| matches!(c.kind, Computed::Sql(_)));
            let sorted = current_order
                .first()
                .filter(|(name, _)| name == column)
                .map(|(_, desc)| if *desc { "desc" } else { "asc" });
            let next = if sorted == Some("asc") {
                format!("-{column}")
            } else {
                column.clone()
            };
            Header {
                label: entity.label_for(column),
                sort_url: sortable.then(|| {
                    state
                        .urls
                        .list_with(&entity.name, &params.with_order(next).encode())
                }),
                sorted,
            }
        })
        .collect()
}

/// First/last pages and a bounded window around the current page, in order.
fn page_numbers(page: u64, pages: u64) -> impl Iterator<Item = u64> {
    let start = Ord::max(page.saturating_sub(3), 2);
    let end = Ord::min(page.saturating_add(3), pages.saturating_sub(1));
    iter::once(1)
        .chain(start..=end)
        .chain((pages > 1).then_some(pages))
}

fn pagination_links(
    state: &RequestState,
    entity: &EntityAdmin,
    params: &ListParams,
    page: u64,
    pages: u64,
) -> Vec<PageLink> {
    page_numbers(page, pages)
        .map(|number| PageLink {
            number,
            url: state
                .urls
                .list_with(&entity.name, &params.with_page(number).encode()),
            active: number == page,
        })
        .collect()
}

async fn render_list(
    state: &RequestState,
    entity: &EntityAdmin,
    params: ListParams,
    flash: Option<Flash>,
    partial: bool,
) -> PageResult {
    let flash = flash.or_else(|| notice_text(entity, params.notice.as_deref()));
    let per_page = entity.per_page;
    let count_spec = list_spec(
        entity,
        &params,
        per_page,
        0,
        state.scope(entity, &Operation::View)?,
    );
    let total = sql::count(&state.db, entity, &count_spec).await?;
    let pages = Ord::max(total.div_ceil(per_page), 1);
    let page = Ord::min(params.page, pages);
    let spec = list_spec(
        entity,
        &params,
        per_page,
        (page - 1) * per_page,
        state.scope(entity, &Operation::View)?,
    );
    let rows = sql::fetch(&state.db, entity, &spec).await?;
    let columns = entity.list_columns();
    let rows = render_rows(state, entity, rows, &columns, true).await?;

    let headers = list_headers(state, entity, &params, &columns);

    let mut filters = Vec::new();
    for column in &entity.list_filter {
        filters.push(filter_view(state, entity, column, &params).await?);
    }

    let page_links = pagination_links(state, entity, &params, page, pages);

    let actions = action_options(state, entity);
    let selectable = entity.single_pk().is_some() && !actions.is_empty();
    let mut search_params = params.clone();
    search_params.search.clear();
    let hidden: Vec<(String, String)> =
        form_urlencoded::parse(search_params.with_page(1).encode().as_bytes())
            .into_owned()
            .collect();

    let body = state.renderer.render(
        if partial { "list_results.html" } else { "list.html" },
        {
            let mut ctx = state.base(&entity.name);
            ctx.extend(context! {
            entity_name => &(entity.name),
            entity_label => &(entity.label),
            singular => &(entity.singular),
            list_url => &(state.urls.list(&entity.name)),
            add_url => &((entity.addable() && state.scope(entity, &Operation::Create).is_ok()).then(|| state.urls.add(&entity.name))),
            headers,
            rows,
            total,
            page,
            pages,
            page_links,
            prev_url => &((page > 1).then(|| state.urls.list_with(&entity.name, &params.with_page(page - 1).encode()))),
            next_url => &((page < pages).then(|| state.urls.list_with(&entity.name, &params.with_page(page + 1).encode()))),
            search => &(params.search),
            searchable => &(!entity.search_fields.is_empty() || !entity.search_sql.is_empty()),
            search_hidden => &(hidden),
            filters,
            actions,
            selectable,
            query => &(params.encode()),
            flash,
            });
            ctx
        },
    )?;
    let mut response = html(StatusCode::OK, body);
    response.headers_mut().insert(
        header::VARY,
        HeaderValue::from_static("HX-Request, HX-History-Restore-Request"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if partial {
        response
            .headers_mut()
            .insert("x-reefadmin-fragment", HeaderValue::from_static("true"));
    }
    Ok(response)
}

pub(super) async fn list_page(
    state: RequestState,
    Path(entity): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> PageResult {
    let entity = find(&state, &entity)?;
    render_list(
        &state,
        entity,
        ListParams::parse(query.as_deref()),
        None,
        is_partial(&headers),
    )
    .await
}

async fn export_csv(
    state: &RequestState,
    entity: &EntityAdmin,
    params: &ListParams,
    selected: &[String],
) -> PageResult {
    let mut spec = list_spec(
        entity,
        params,
        CSV_ROWS,
        0,
        state.scope(entity, &Operation::Export)?,
    );
    if !selected.is_empty()
        && let Some(pk) = entity.single_pk()
    {
        spec.conditions
            .push(in_condition(entity.backend.table(), pk, selected));
    }
    let rows = sql::fetch_export(&state.db, entity, &spec).await?;
    let columns: Vec<&str> = entity.export_fields.iter().map(String::as_str).collect();
    let mut writer = WriterBuilder::new()
        .terminator(Terminator::CRLF)
        .from_writer(Vec::new());
    writer
        .write_record(&columns)
        .map_err(|error| AdminError::custom(error.to_string()))?;
    for row in rows {
        writer
            .write_record(
                columns
                    .iter()
                    .map(|column| row.get(*column).map(key_string).unwrap_or_default()),
            )
            .map_err(|error| AdminError::custom(error.to_string()))?;
    }
    let out = writer
        .into_inner()
        .map_err(|error| AdminError::custom(error.to_string()))?;
    let disposition = format!("attachment; filename=\"{}.csv\"", entity.name);
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (header::CONTENT_DISPOSITION, disposition),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        out,
    )
        .into_response())
}

pub(super) async fn bulk_action(
    state: RequestState,
    Path(entity): Path<String>,
    body: Bytes,
) -> PageResult {
    let entity = find(&state, &entity)?;
    let form = form_map(&body);
    let selected = form_values(&body, "selected");
    let params = ListParams::parse(form.get("q").map(String::as_str));
    let action = form.get("action").map(String::as_str).unwrap_or_default();

    let operation = match action {
        CSV_ACTION => Operation::Export,
        DELETE_ACTION => Operation::Delete,
        name => Operation::Action(name.to_owned()),
    };
    state.scope(entity, &operation)?;
    if operation == Operation::Export && !entity.exportable() {
        return Err(Failure::Forbidden(
            "CSV export requires configured export fields.",
        ));
    }
    // Validate the entire selection before calling handlers or deleting any row.
    let mut authorized = Vec::new();
    for id in selected {
        let id = if operation == Operation::Export {
            sql::export_id(&state.db, entity, &id, state.scope(entity, &operation)?)
                .await?
                .ok_or(Failure::NotFound)?
        } else {
            let row = state.get(entity, &id, &operation).await?;
            if operation == Operation::Delete {
                let access = state.write_access(entity, operation.clone())?;
                access.check(&state.db, Some(&row), &row).await?;
            }
            row_id(entity, &row).ok_or(Failure::NotFound)?
        };
        if !authorized.contains(&id) {
            authorized.push(id);
        }
    }
    let selected = authorized;
    if action == CSV_ACTION {
        return export_csv(&state, entity, &params, &selected).await;
    }
    if selected.is_empty() {
        let flash = Flash::error("Select at least one row first.");
        return render_list(&state, entity, params, Some(flash), false).await;
    }
    if action == DELETE_ACTION {
        if !(entity.deletable() && entity.bulk_delete) {
            return Err(Failure::Forbidden("Deleting records is disabled."));
        }
        let mut deleted = 0;
        for id in &selected {
            if let Some(condition) = sql::pk_condition(entity, id)
                && entity
                    .backend
                    .delete(
                        &state.db,
                        condition,
                        state.observer.as_deref(),
                        &state.write_access(entity, Operation::Delete)?,
                    )
                    .await?
            {
                deleted += 1;
            }
        }
        let flash = Flash::success(format!(
            "Deleted {deleted} {}.",
            if deleted == 1 {
                entity.singular.clone()
            } else {
                entity.label.to_lowercase()
            }
        ));
        return render_list(&state, entity, params, Some(flash), false).await;
    }
    let Some(custom) = entity.actions.iter().find(|a| a.name == action) else {
        return Err(Failure::BadRequest("Unknown action.".into()));
    };
    let ctx = ActionContext {
        actor: state.actor.clone(),
        db: state.db.clone(),
        ids: selected,
    };
    let flash = match (custom.handler)(ctx).await {
        Ok(ActionResult::Success(message)) => Flash::success(message),
        Ok(ActionResult::Error(message)) => Flash::error(message),
        Ok(ActionResult::Redirect(path)) => {
            return Ok(Redirect::to(&state.urls.path(&path)).into_response());
        }
        Err(err) => return Err(err.into()),
    };
    render_list(&state, entity, params, Some(flash), false).await
}

pub(super) async fn toggle(
    state: RequestState,
    Path((entity, id, field)): Path<(String, String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> PageResult {
    let entity = find(&state, &entity)?;
    if !entity.editable() || !entity.toggles.contains(&field) {
        return Err(Failure::Forbidden(
            "This field cannot be changed from the list.",
        ));
    }
    state.scope(entity, &Operation::Update)?;
    let operation = Operation::Toggle(field.clone());
    let row = state.get(entity, &id, &operation).await?;
    let current = row.get(&field).and_then(values::as_bool).unwrap_or(false);
    let mut data = serde_json::Map::new();
    for pk in entity.backend.primary_key() {
        data.insert(pk.clone(), row.get(pk).cloned().unwrap_or_default());
    }
    data.insert(field, serde_json::Value::Bool(!current));
    entity
        .backend
        .update(
            &state.db,
            data,
            state.observer.as_deref(),
            &state.write_access(entity, operation)?,
        )
        .await?;
    if is_partial(&headers) {
        let row = state.get(entity, &id, &Operation::View).await?;
        let columns = entity.list_columns();
        let mut rows = render_rows(&state, entity, vec![row], &columns, true).await?;
        let row = rows.remove(0);
        let selectable = entity.single_pk().is_some() && !action_options(&state, entity).is_empty();
        let body = state
            .renderer
            .render("row.html", context! { row, selectable })?;
        let mut response = html(StatusCode::OK, body);
        response
            .headers_mut()
            .insert("x-reefadmin-fragment", HeaderValue::from_static("true"));
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return Ok(response);
    }
    let query = form_map(&body).remove("q").unwrap_or_default();
    let params = ListParams::parse(Some(&query));
    Ok(Redirect::to(&state.urls.list_with(&entity.name, &params.encode())).into_response())
}

#[derive(Deserialize)]
pub(super) struct AutocompleteQuery {
    #[serde(default)]
    q: String,
}

#[derive(Serialize)]
pub(super) struct AutocompleteItem {
    id: String,
    label: String,
}

pub(super) async fn autocomplete(
    state: RequestState,
    Path(entity): Path<String>,
    Query(query): Query<AutocompleteQuery>,
    headers: HeaderMap,
) -> Result<Response, Failure> {
    let entity = find(&state, &entity)?;
    let items: Vec<AutocompleteItem> = sql::options(
        &state.db,
        entity,
        Some(&query.q),
        20,
        state.scope(entity, &Operation::Autocomplete)?,
    )
    .await?
    .into_iter()
    .map(|(id, label)| AutocompleteItem { id, label })
    .collect();
    let mut response = if is_partial(&headers) {
        let body = state
            .renderer
            .render("relationship_options.html", context! { items })?;
        let mut response = html(StatusCode::OK, body);
        response
            .headers_mut()
            .insert("x-reefadmin-fragment", HeaderValue::from_static("true"));
        response
    } else {
        Json(items).into_response()
    };
    response.headers_mut().insert(
        header::VARY,
        HeaderValue::from_static("HX-Request, HX-History-Restore-Request"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

/// The URL primary key of a stored row, used after inserts.
pub(super) fn id_of(entity: &EntityAdmin, row: &Row) -> Option<String> {
    row_id(entity, row)
}

fn is_partial(headers: &HeaderMap) -> bool {
    headers
        .get("hx-request")
        .is_some_and(|value| value == "true")
        && !headers
            .get("hx-history-restore-request")
            .is_some_and(|value| value == "true")
}

#[cfg(test)]
#[path = "../tests/list.rs"]
mod tests;
