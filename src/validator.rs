use email_address::{EmailAddress, Options};
use regex::Regex;

/// Validates a submitted form value before it is converted to the column type.
pub trait Validator: Send + Sync {
    fn validate(&self, value: &str) -> Result<(), String>;
}

/// Fails on an empty or whitespace-only value.
pub struct Required;

impl Validator for Required {
    fn validate(&self, value: &str) -> Result<(), String> {
        if value.trim().is_empty() {
            Err("This field is required.".to_string())
        } else {
            Ok(())
        }
    }
}

/// Fails if the value has fewer than `n` characters.
pub struct MinLength(pub usize);

impl Validator for MinLength {
    fn validate(&self, value: &str) -> Result<(), String> {
        if value.chars().count() < self.0 {
            Err(format!("Must be at least {} characters.", self.0))
        } else {
            Ok(())
        }
    }
}

/// Fails if the value has more than `n` characters.
pub struct MaxLength(pub usize);

impl Validator for MaxLength {
    fn validate(&self, value: &str) -> Result<(), String> {
        if value.chars().count() > self.0 {
            Err(format!("Must be at most {} characters.", self.0))
        } else {
            Ok(())
        }
    }
}

/// Fails if a non-empty value is not a number or is below `n`.
pub struct MinValue(pub f64);

impl Validator for MinValue {
    fn validate(&self, value: &str) -> Result<(), String> {
        if value.trim().is_empty() {
            return Ok(());
        }
        match value.trim().parse::<f64>() {
            Ok(n) if n >= self.0 => Ok(()),
            Ok(_) => Err(format!("Must be at least {}.", self.0)),
            Err(_) => Err("Must be a valid number.".to_string()),
        }
    }
}

/// Fails if a non-empty value is not a number or is above `n`.
pub struct MaxValue(pub f64);

impl Validator for MaxValue {
    fn validate(&self, value: &str) -> Result<(), String> {
        if value.trim().is_empty() {
            return Ok(());
        }
        match value.trim().parse::<f64>() {
            Ok(n) if n <= self.0 => Ok(()),
            Ok(_) => Err(format!("Must be at most {}.", self.0)),
            Err(_) => Err("Must be a valid number.".to_string()),
        }
    }
}

/// Fails if a non-empty value does not match the pattern.
pub struct RegexValidator {
    regex: Regex,
    message: String,
}

impl RegexValidator {
    pub fn new(pattern: &str, message: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            regex: Regex::new(pattern)?,
            message: message.to_string(),
        })
    }
}

impl Validator for RegexValidator {
    fn validate(&self, value: &str) -> Result<(), String> {
        if value.trim().is_empty() || self.regex.is_match(value) {
            Ok(())
        } else {
            Err(self.message.clone())
        }
    }
}

/// Validates a bare email address with a dotted domain, including Unicode addresses.
///
/// Accepts empty or whitespace-only values. Add [`Required`] to reject them.
/// Non-empty values are checked as submitted, without trimming or normalisation.
/// Display names and domain literals are rejected. This checks syntax, not delivery.
pub struct EmailFormat;

impl Validator for EmailFormat {
    fn validate(&self, value: &str) -> Result<(), String> {
        if value.trim().is_empty() {
            return Ok(());
        }
        let options = Options::default()
            .with_required_tld()
            .without_domain_literal()
            .without_display_text();
        let valid = EmailAddress::parse_with_options(value, options).is_ok();
        if valid {
            Ok(())
        } else {
            Err("Enter a valid email address.".to_string())
        }
    }
}

#[cfg(test)]
#[path = "tests/validator.rs"]
mod tests;
