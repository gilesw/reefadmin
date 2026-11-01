use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use std::collections::BTreeMap;

/// Characters escaped in a path segment: everything except unreserved characters.
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

pub(crate) fn segment(value: &str) -> String {
    utf8_percent_encode(value, SEGMENT).to_string()
}

/// Builds every admin URL from the mount prefix.
#[derive(Clone)]
pub(crate) struct Urls {
    prefix: String,
}

impl Urls {
    pub(crate) fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.trim_end_matches('/').to_string(),
        }
    }

    pub(crate) fn home(&self) -> String {
        if self.prefix.is_empty() {
            "/".to_string()
        } else {
            self.prefix.clone()
        }
    }

    /// A path inside the admin, such as `/rules`.
    pub(crate) fn path(&self, path: &str) -> String {
        format!("{}{}", self.prefix, path)
    }

    pub(crate) fn static_file(&self, file: &str) -> String {
        format!("{}/_static/{file}", self.prefix)
    }

    pub(crate) fn list(&self, entity: &str) -> String {
        format!("{}/{entity}", self.prefix)
    }

    pub(crate) fn list_with(&self, entity: &str, query: &str) -> String {
        if query.is_empty() {
            self.list(entity)
        } else {
            format!("{}/{entity}?{query}", self.prefix)
        }
    }

    pub(crate) fn add(&self, entity: &str) -> String {
        format!("{}/{entity}/add", self.prefix)
    }

    pub(crate) fn autocomplete(&self, entity: &str) -> String {
        format!("{}/{entity}/autocomplete", self.prefix)
    }

    pub(crate) fn change(&self, entity: &str, id: &str) -> String {
        format!("{}/{entity}/{}/change", self.prefix, segment(id))
    }

    pub(crate) fn delete(&self, entity: &str, id: &str) -> String {
        format!("{}/{entity}/{}/delete", self.prefix, segment(id))
    }

    pub(crate) fn toggle(&self, entity: &str, id: &str, field: &str) -> String {
        format!("{}/{entity}/{}/toggle/{field}", self.prefix, segment(id))
    }
}

/// Parameters of a list page: search (`q`), sort (`o`), page (`p`) and filters (`f_<column>`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ListParams {
    pub search: String,
    pub order: Option<String>,
    pub page: u64,
    pub filters: BTreeMap<String, String>,
    pub notice: Option<String>,
}

impl ListParams {
    pub(crate) fn parse(raw: Option<&str>) -> Self {
        let mut params = Self {
            page: 1,
            ..Self::default()
        };
        let Some(raw) = raw else {
            return params;
        };
        for (key, value) in form_urlencoded::parse(raw.as_bytes()) {
            match key.as_ref() {
                "q" => params.search = value.into_owned(),
                "o" if !value.is_empty() => params.order = Some(value.into_owned()),
                "p" => params.page = value.parse().unwrap_or(1).max(1),
                "notice" => params.notice = Some(value.into_owned()),
                other => {
                    if let Some(column) = other.strip_prefix("f_")
                        && !value.is_empty()
                    {
                        params
                            .filters
                            .insert(column.to_string(), value.into_owned());
                    }
                }
            }
        }
        params
    }

    /// The query string for these parameters, without `notice`.
    pub(crate) fn encode(&self) -> String {
        let mut out = form_urlencoded::Serializer::new(String::new());
        if !self.search.is_empty() {
            out.append_pair("q", &self.search);
        }
        for (column, value) in &self.filters {
            out.append_pair(&format!("f_{column}"), value);
        }
        if let Some(order) = &self.order {
            out.append_pair("o", order);
        }
        if self.page > 1 {
            out.append_pair("p", &self.page.to_string());
        }
        out.finish()
    }

    pub(crate) fn with_page(&self, page: u64) -> Self {
        Self {
            page,
            notice: None,
            ..self.clone()
        }
    }

    pub(crate) fn with_order(&self, order: String) -> Self {
        Self {
            order: Some(order),
            page: 1,
            notice: None,
            ..self.clone()
        }
    }

    pub(crate) fn with_filter(&self, column: &str, value: Option<&str>) -> Self {
        let mut next = Self {
            page: 1,
            notice: None,
            ..self.clone()
        };
        match value {
            Some(value) => next.filters.insert(column.to_string(), value.to_string()),
            None => next.filters.remove(column),
        };
        next
    }

    /// Sort as `(column, descending)`.
    pub(crate) fn order_column(&self) -> Option<(String, bool)> {
        let order = self.order.as_deref()?;
        Some(match order.strip_prefix('-') {
            Some(column) => (column.to_string(), true),
            None => (order.to_string(), false),
        })
    }
}

#[cfg(test)]
#[path = "tests/urls.rs"]
mod tests;
