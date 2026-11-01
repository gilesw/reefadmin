use super::*;

#[test]
fn built_in_validators() {
    assert!(Required.validate(" ").is_err());
    assert!(MinLength(3).validate("ab").is_err());
    assert!(MaxLength(3).validate("abcd").is_err());
    assert!(MinValue(1.0).validate("0").is_err());
    assert!(MaxValue(1.0).validate("2").is_err());
    assert!(MaxValue(1.0).validate("").is_ok());
    assert!(EmailFormat.validate("a@b.co").is_ok());
    assert!(EmailFormat.validate("a@b").is_err());
    let upper = RegexValidator::new("^[A-Z]+$", "Use capitals.").unwrap();
    assert_eq!(upper.validate("abc"), Err("Use capitals.".to_string()));
}

#[test]
fn length_and_numeric_limits_include_the_boundary() {
    assert!(Required.validate("value").is_ok());
    for value in ["é水🙂", "abc"] {
        assert!(MinLength(3).validate(value).is_ok());
        assert!(MaxLength(3).validate(value).is_ok());
    }
    assert!(MinLength(3).validate("é水").is_err());
    assert!(MaxLength(3).validate("é水🙂a").is_err());
    for value in ["1", " 1.0 ", ""] {
        assert!(MinValue(1.0).validate(value).is_ok());
        assert!(MaxValue(1.0).validate(value).is_ok());
    }
    assert!(MinValue(1.0).validate("2").is_ok());
    assert!(MaxValue(1.0).validate("0").is_ok());
    for value in ["bad", "NaN"] {
        assert!(MinValue(1.0).validate(value).is_err());
        assert!(MaxValue(1.0).validate(value).is_err());
    }
}

#[test]
fn optional_regex_and_email_fields_validate_nonempty_values() {
    assert!(RegexValidator::new("[", "invalid").is_err());
    let upper = RegexValidator::new("^[A-Z]+$", "Use capitals.").unwrap();
    assert!(upper.validate("ABC").is_ok());
    assert!(upper.validate("").is_ok());
    assert!(EmailFormat.validate("").is_ok());
    for value in [
        "missing-at",
        "@example.com",
        "a@.example",
        "a@example.",
        "a@b@c.com",
    ] {
        assert!(EmailFormat.validate(value).is_err(), "{value}");
    }
}

#[test]
fn email_syntax_rejects_malformed_addresses_without_changing_optional_values() {
    for value in [
        "",
        " \t\n",
        "user+tag@example.com",
        "用户@例子.公司",
        "é@example.com",
        "\"quoted local\"@example.com",
    ] {
        assert!(EmailFormat.validate(value).is_ok(), "{value:?}");
    }
    for value in [
        " user@example.com",
        "user@example.com ",
        "user\n@example.com",
        "first last@example.com",
        ".user@example.com",
        "user.@example.com",
        "first..last@example.com",
        "user@example..com",
        "user@-example.com",
        "user@example-.com",
        "user@local",
        "User <user@example.com>",
        "<user@example.com>",
        "user@[127.0.0.1]",
    ] {
        assert_eq!(
            EmailFormat.validate(value),
            Err("Enter a valid email address.".into()),
            "{value:?}"
        );
    }
}

#[test]
fn email_parser_enforces_local_part_and_domain_label_byte_limits() {
    for local in ["a".repeat(64), "é".repeat(32)] {
        assert!(
            EmailFormat
                .validate(&format!("{local}@example.com"))
                .is_ok()
        );
    }
    for local in ["a".repeat(65), "é".repeat(33)] {
        assert!(
            EmailFormat
                .validate(&format!("{local}@example.com"))
                .is_err()
        );
    }
    assert!(
        EmailFormat
            .validate(&format!("a@{}.com", "b".repeat(63)))
            .is_ok()
    );
    assert!(
        EmailFormat
            .validate(&format!("a@{}.com", "b".repeat(64)))
            .is_err()
    );
}
