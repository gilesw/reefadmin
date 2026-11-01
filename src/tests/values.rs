use super::*;

fn meta(kind: ColKind, nullable: bool) -> ColumnMeta {
    ColumnMeta {
        name: "c".into(),
        json_key: "c".into(),
        kind,
        column_type: match kind {
            ColKind::Integer => ColumnType::BigInteger,
            ColKind::Float => ColumnType::Double,
            _ => ColumnType::Text,
        },
        nullable,
        enum_values: Vec::new(),
    }
}

#[test]
fn numeric_comparisons_preserve_database_width_and_reject_overflow() {
    let mut column = meta(ColKind::Integer, false);
    column.column_type = ColumnType::SmallInteger;
    assert_eq!(comparable(&column, "42"), Ok(Some(SeaValue::from(42i16))));
    assert!(comparable(&column, "32768").is_err());
    column.column_type = ColumnType::Integer;
    assert_eq!(comparable(&column, "42"), Ok(Some(SeaValue::from(42i32))));
    assert!(comparable(&column, "2147483648").is_err());
    column.column_type = ColumnType::BigInteger;
    assert_eq!(
        comparable(&column, "2147483648"),
        Ok(Some(SeaValue::from(2_147_483_648i64)))
    );
    assert!(comparable(&column, "9223372036854775808").is_err());
    column.column_type = ColumnType::Unsigned;
    assert!(comparable(&column, "-1").is_err());
    column.kind = ColKind::Float;
    column.column_type = ColumnType::Float;
    assert_eq!(comparable(&column, "1.5"), Ok(Some(SeaValue::from(1.5f32))));
    column.column_type = ColumnType::Double;
    assert_eq!(comparable(&column, "1.5"), Ok(Some(SeaValue::from(1.5f64))));
}

#[test]
fn form_values_convert_by_kind() {
    assert_eq!(
        form_to_json(&meta(ColKind::Integer, false), " 42 "),
        Ok(Value::from(42))
    );
    assert!(form_to_json(&meta(ColKind::Integer, false), "x").is_err());
    assert!(form_to_json(&meta(ColKind::Integer, false), "").is_err());
    assert_eq!(
        form_to_json(&meta(ColKind::Integer, true), ""),
        Ok(Value::Null)
    );
    assert_eq!(
        form_to_json(&meta(ColKind::Text, false), ""),
        Ok(Value::from(""))
    );
    assert_eq!(
        form_to_json(&meta(ColKind::Bool, false), "on"),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        form_to_json(&meta(ColKind::DateTimeTz, false), "2026-10-03T12:30"),
        Ok(Value::from("2026-10-03T12:30:00+00:00"))
    );
    assert_eq!(
        form_to_json(&meta(ColKind::Json, false), r#"{"a":1}"#),
        Ok(serde_json::json!({"a": 1}))
    );
}

#[test]
fn stored_values_format_for_inputs_and_pages() {
    let ts = Value::from("2026-10-03T13:30:00+01:00");
    assert_eq!(
        json_to_input(ColKind::DateTimeTz, &ts),
        "2026-10-03T12:30:00"
    );
    assert_eq!(
        display(Some(ColKind::DateTimeTz), &ts),
        "2026-10-03 12:30 UTC"
    );
    assert_eq!(display(Some(ColKind::Bool), &Value::from(1)), "Yes");
    assert_eq!(truncate("abcdef".into(), 3), "abc…");
}

#[test]
fn boolean_aliases_are_case_insensitive_but_invalid_values_are_rejected() {
    for value in ["true", "1", "ON", "Yes", " t "] {
        assert_eq!(parse_bool(value), Some(true), "{value}");
    }
    for value in ["false", "0", "OFF", "No", " f ", "", "  "] {
        assert_eq!(parse_bool(value), Some(false), "{value}");
    }
    for value in ["2", "-1", "truthy", "null"] {
        assert_eq!(parse_bool(value), None);
        assert!(form_to_json(&meta(ColKind::Bool, false), value).is_err());
    }
    assert_eq!(as_bool(&Value::Null), None);
    assert_eq!(as_bool(&Value::Bool(false)), Some(false));
    assert_eq!(as_bool(&Value::from("yes")), Some(true));
}

#[test]
fn integer_bindings_check_each_database_width() {
    for (kind, maximum, overflow, expected) in [
        (ColumnType::TinyInteger, "127", "128", SeaValue::from(127i8)),
        (
            ColumnType::SmallInteger,
            "32767",
            "32768",
            SeaValue::from(32767i16),
        ),
        (
            ColumnType::Integer,
            "2147483647",
            "2147483648",
            SeaValue::from(i32::MAX),
        ),
        (
            ColumnType::BigInteger,
            "9223372036854775807",
            "9223372036854775808",
            SeaValue::from(i64::MAX),
        ),
        (
            ColumnType::TinyUnsigned,
            "255",
            "256",
            SeaValue::from(255u8),
        ),
        (
            ColumnType::SmallUnsigned,
            "65535",
            "65536",
            SeaValue::from(65535u16),
        ),
        (
            ColumnType::Unsigned,
            "4294967295",
            "4294967296",
            SeaValue::from(u32::MAX),
        ),
        (
            ColumnType::BigUnsigned,
            "18446744073709551615",
            "18446744073709551616",
            SeaValue::from(u64::MAX),
        ),
    ] {
        let mut column = meta(ColKind::Integer, false);
        column.column_type = kind;
        assert_eq!(comparable(&column, maximum), Ok(Some(expected)));
        assert!(comparable(&column, overflow).is_err());
        assert!(comparable(&column, "1.2").is_err());
    }
}

#[test]
fn numeric_and_empty_form_values_have_distinct_meanings() {
    for raw in ["NaN", "inf", "-inf", "1e999", "no"] {
        assert!(form_to_json(&meta(ColKind::Float, false), raw).is_err());
    }
    assert_eq!(
        form_to_json(&meta(ColKind::Float, false), " 1.25 "),
        Ok(Value::from(1.25))
    );
    assert_eq!(
        form_to_json(&meta(ColKind::Decimal, false), " 123.4500 "),
        Ok(Value::from("123.4500"))
    );
    assert!(form_to_json(&meta(ColKind::Decimal, false), "invalid").is_err());
    assert_eq!(
        form_to_json(&meta(ColKind::Text, false), "  "),
        Ok(Value::from("  "))
    );
    assert_eq!(
        form_to_json(&meta(ColKind::Text, true), "  "),
        Ok(Value::Null)
    );
    assert!(form_to_json(&meta(ColKind::Integer, false), "9223372036854775808").is_err());
}

#[test]
fn dates_times_and_offsets_validate_and_normalize() {
    for (kind, raw, expected) in [
        (ColKind::Date, "2024-02-29", "2024-02-29"),
        (ColKind::DateTime, "2026-11-01 12:34", "2026-11-01T12:34:00"),
        (
            ColKind::DateTimeTz,
            "2026-11-01T12:34:56+02:00",
            "2026-11-01T10:34:56+00:00",
        ),
        (ColKind::Time, "12:34", "12:34:00"),
        (ColKind::Time, "12:34:56", "12:34:56"),
    ] {
        assert_eq!(
            form_to_json(&meta(kind, false), raw),
            Ok(Value::from(expected))
        );
    }
    for (kind, raw) in [
        (ColKind::Date, "2025-02-29"),
        (ColKind::DateTime, "yesterday"),
        (ColKind::DateTimeTz, "2026-11-01T12:00:00+99:00"),
        (ColKind::Time, "25:00"),
    ] {
        assert!(form_to_json(&meta(kind, false), raw).is_err());
    }
    assert_eq!(
        json_to_input(ColKind::DateTime, &Value::from("2026-11-01 12:34")),
        "2026-11-01T12:34:00"
    );
    assert_eq!(
        display(Some(ColKind::DateTime), &Value::from("2026-11-01T12:34:56")),
        "2026-11-01 12:34"
    );
    for kind in [ColKind::DateTime, ColKind::DateTimeTz] {
        assert_eq!(
            json_to_input(kind, &Value::from("legacy invalid value")),
            "legacy invalid value"
        );
        assert_eq!(
            display(Some(kind), &Value::from("legacy invalid value")),
            "legacy invalid value"
        );
    }
}

#[test]
fn arrays_json_and_enums_reject_wrong_shapes_and_choices() {
    let array = meta(ColKind::Array, false);
    assert_eq!(form_to_json(&array, "[]"), Ok(serde_json::json!([])));
    assert_eq!(
        form_to_json(&meta(ColKind::Array, true), ""),
        Ok(Value::Null)
    );
    for raw in ["{}", "null", "\"text\"", "[broken"] {
        assert!(form_to_json(&array, raw).is_err());
    }
    assert!(form_to_json(&meta(ColKind::Json, false), "{broken").is_err());
    let mut choice = meta(ColKind::Enum, false);
    choice.enum_values = vec!["draft".into(), "published".into()];
    assert_eq!(form_to_json(&choice, " draft "), Ok(Value::from("draft")));
    assert!(form_to_json(&choice, "hidden").is_err());
    assert_eq!(json_to_input(ColKind::Array, &Value::from("[1]")), "[1]");
    assert_eq!(
        json_to_input(ColKind::Json, &serde_json::json!({"a":1})),
        "{\n  \"a\": 1\n}"
    );
}

#[test]
fn display_and_truncation_preserve_unicode_and_boundary_values() {
    assert_eq!(truncate("é水🙂".into(), 3), "é水🙂");
    assert_eq!(truncate("é水🙂".into(), 2), "é水…");
    assert_eq!(truncate("é".into(), 0), "…");
    assert_eq!(truncate(String::new(), 0), "");
    assert_eq!(key_string(&serde_json::json!([1, 2])), "[1,2]");
    assert_eq!(json_to_input(ColKind::Text, &Value::Null), "");
    assert_eq!(display(None, &Value::Null), "");
    assert_eq!(display(Some(ColKind::Bool), &Value::from(0)), "No");
    assert_eq!(
        display(Some(ColKind::Bool), &Value::from("unknown")),
        "unknown"
    );
}
