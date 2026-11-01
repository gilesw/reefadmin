use crate::backend::{ColKind, ColumnMeta};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use sea_orm::sea_query::{Alias, ColumnType, Expr, ExprTrait, Value as SeaValue};
use serde_json::Value;
use std::str::FromStr;

/// The string form of a value as used in URLs, filters and CSV.
pub(crate) fn key_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" | "t" => Some(true),
        "false" | "0" | "off" | "no" | "f" | "" => Some(false),
        _ => None,
    }
}

pub(crate) fn column_expr(table: &str, column: &str) -> Expr {
    Expr::col((Alias::new(table), Alias::new(column)))
}

fn parsed_comparison<T>(raw: &str, description: &str) -> Result<Option<SeaValue>, String>
where
    T: FromStr + Into<SeaValue>,
{
    raw.parse::<T>()
        .map(|value| Some(value.into()))
        .map_err(|_| format!("{raw} is not {description}"))
}

/// A bindable value for comparisons, or `None` when the column must be compared as text.
fn comparable(column: &ColumnMeta, raw: &str) -> Result<Option<SeaValue>, String> {
    let raw = raw.trim();
    // A prepared PostgreSQL query expects the column's wire representation.
    // Treating every integer as i64 breaks typed ORM lookups for i16/i32 keys.
    match column.column_type {
        ColumnType::TinyInteger => parsed_comparison::<i8>(raw, "a whole number"),
        ColumnType::SmallInteger => parsed_comparison::<i16>(raw, "a whole number"),
        ColumnType::Integer => parsed_comparison::<i32>(raw, "a whole number"),
        ColumnType::BigInteger => parsed_comparison::<i64>(raw, "a whole number"),
        ColumnType::TinyUnsigned => parsed_comparison::<u8>(raw, "a whole number"),
        ColumnType::SmallUnsigned => parsed_comparison::<u16>(raw, "a whole number"),
        ColumnType::Unsigned => parsed_comparison::<u32>(raw, "a whole number"),
        ColumnType::BigUnsigned => parsed_comparison::<u64>(raw, "a whole number"),
        ColumnType::Float => parsed_comparison::<f32>(raw, "a number"),
        ColumnType::Double => parsed_comparison::<f64>(raw, "a number"),
        _ => match column.kind {
            ColKind::Bool => parse_bool(raw)
                .map(|v| Some(SeaValue::from(v)))
                .ok_or_else(|| format!("{raw} is not yes or no")),
            ColKind::Text | ColKind::LongText => Ok(Some(SeaValue::from(raw.to_string()))),
            _ => Ok(None),
        },
    }
}

/// `table.column = raw`, with `raw` converted to the column's type.
pub(crate) fn eq_condition(table: &str, column: &ColumnMeta, raw: &str) -> Result<Expr, String> {
    let col = column_expr(table, &column.name);
    Ok(match comparable(column, raw)? {
        Some(value) => col.eq(value),
        None => col.cast_as(Alias::new("text")).eq(raw.trim().to_string()),
    })
}

/// `table.column IN (raws)`; values that do not parse are skipped.
pub(crate) fn in_condition(table: &str, column: &ColumnMeta, raws: &[String]) -> Expr {
    let col = column_expr(table, &column.name);
    let typed: Vec<Option<SeaValue>> = raws
        .iter()
        .filter_map(|raw| comparable(column, raw).ok())
        .collect();
    if typed.iter().all(Option::is_some) {
        col.is_in(typed.into_iter().flatten())
    } else {
        col.cast_as(Alias::new("text")).is_in(raws.iter().cloned())
    }
}

fn parse_naive_datetime(raw: &str) -> Option<NaiveDateTime> {
    [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M",
    ]
    .iter()
    .find_map(|format| NaiveDateTime::parse_from_str(raw, format).ok())
}

fn parse_utc(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .or_else(|_| DateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f%:z"))
        .or_else(|_| DateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f %:z"))
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
        .or_else(|| parse_naive_datetime(raw).map(|naive| naive.and_utc()))
}

/// Converts a submitted form value into the JSON the model deserialises.
pub(crate) fn form_to_json(column: &ColumnMeta, raw: &str) -> Result<Value, String> {
    let trimmed = raw.trim();
    if column.kind == ColKind::Bool {
        return parse_bool(trimmed)
            .map(Value::Bool)
            .ok_or_else(|| "Choose yes or no.".to_string());
    }
    if trimmed.is_empty() {
        if column.nullable {
            return Ok(Value::Null);
        }
        if column.kind.is_textual() {
            return Ok(Value::String(raw.to_string()));
        }
        return Err("This field is required.".to_string());
    }
    match column.kind {
        ColKind::Integer => trimmed
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| "Enter a whole number.".to_string()),
        ColKind::Float => trimmed
            .parse::<f64>()
            .ok()
            .and_then(|v| serde_json::Number::from_f64(v).map(Value::Number))
            .ok_or_else(|| "Enter a number.".to_string()),
        ColKind::Decimal => trimmed
            .parse::<f64>()
            .map(|_| Value::String(trimmed.to_string()))
            .map_err(|_| "Enter a number.".to_string()),
        ColKind::Date => NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
            .map(|d| Value::String(d.format("%Y-%m-%d").to_string()))
            .map_err(|_| "Enter a date as YYYY-MM-DD.".to_string()),
        ColKind::DateTime => parse_naive_datetime(trimmed)
            .map(|dt| Value::String(dt.format("%Y-%m-%dT%H:%M:%S").to_string()))
            .ok_or_else(|| "Enter a date and time.".to_string()),
        ColKind::DateTimeTz => parse_utc(trimmed)
            .map(|dt| Value::String(dt.to_rfc3339()))
            .ok_or_else(|| "Enter a date and time.".to_string()),
        ColKind::Time => ["%H:%M:%S%.f", "%H:%M"]
            .iter()
            .find_map(|format| NaiveTime::parse_from_str(trimmed, format).ok())
            .map(|t| Value::String(t.format("%H:%M:%S").to_string()))
            .ok_or_else(|| "Enter a time as HH:MM.".to_string()),
        ColKind::Array => {
            let value: Value =
                serde_json::from_str(trimmed).map_err(|e| format!("Enter a JSON array: {e}"))?;
            if value.is_array() {
                Ok(value)
            } else {
                Err("Enter a JSON array.".into())
            }
        }
        ColKind::Json => {
            serde_json::from_str(trimmed).map_err(|e| format!("Enter valid JSON: {e}"))
        }
        ColKind::Enum if !column.enum_values.is_empty() => {
            if column.enum_values.iter().any(|v| v == trimmed) {
                Ok(Value::String(trimmed.to_string()))
            } else {
                Err("Choose one of the listed options.".to_string())
            }
        }
        _ => Ok(Value::String(raw.to_string())),
    }
}

/// Formats a stored value for an `<input>` element.
pub(crate) fn json_to_input(kind: ColKind, value: &Value) -> String {
    match (kind, value) {
        (_, Value::Null) => String::new(),
        (ColKind::DateTimeTz, Value::String(s)) => parse_utc(s)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
            .unwrap_or_else(|| s.clone()),
        (ColKind::DateTime, Value::String(s)) => parse_naive_datetime(s)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
            .unwrap_or_else(|| s.clone()),
        (ColKind::Json | ColKind::Array, Value::String(s)) => s.clone(),
        (ColKind::Json | ColKind::Array, other) => {
            serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string())
        }
        (_, other) => key_string(other),
    }
}

/// Formats a stored value as page text.
pub(crate) fn display(kind: Option<ColKind>, value: &Value) -> String {
    match (kind, value) {
        (_, Value::Null) => String::new(),
        (_, Value::Bool(b)) => if *b { "Yes" } else { "No" }.to_string(),
        (Some(ColKind::Bool), other) => match parse_bool(&key_string(other)) {
            Some(true) => "Yes".to_string(),
            Some(false) => "No".to_string(),
            None => key_string(other),
        },
        (Some(ColKind::DateTimeTz), Value::String(s)) => parse_utc(s)
            .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| s.clone()),
        (Some(ColKind::DateTime), Value::String(s)) => parse_naive_datetime(s)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| s.clone()),
        (_, other) => key_string(other),
    }
}

/// Shortens text for table cells.
pub(crate) fn truncate(text: String, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text
    } else {
        let mut short: String = text.chars().take(max_chars).collect();
        short.push('…');
        short
    }
}

/// Interprets a stored value as a boolean, for SQLite's 0/1 as well as real booleans.
pub(crate) fn as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(b) => Some(*b),
        Value::Null => None,
        other => parse_bool(&key_string(other)),
    }
}

#[cfg(test)]
#[path = "tests/values.rs"]
mod tests;
