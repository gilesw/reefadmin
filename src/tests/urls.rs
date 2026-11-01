use super::*;

#[test]
fn list_params_round_trip() {
    let params = ListParams::parse(Some("q=a+b&f_active=true&o=-name&p=3&x=1"));
    assert_eq!(params.search, "a b");
    assert_eq!(
        params.filters.get("active").map(String::as_str),
        Some("true")
    );
    assert_eq!(params.order_column(), Some(("name".to_string(), true)));
    assert_eq!(params.page, 3);
    assert_eq!(params.encode(), "q=a+b&f_active=true&o=-name&p=3");
    assert_eq!(params.with_filter("active", None).encode(), "q=a+b&o=-name");
}

#[test]
fn urls_respect_prefix_and_escape_ids() {
    let urls = Urls::new("/admin/");
    assert_eq!(urls.home(), "/admin");
    assert_eq!(
        urls.change("exchanges", "X/Y Z"),
        "/admin/exchanges/X%2FY%20Z/change"
    );
    assert_eq!(Urls::new("").home(), "/");
    assert_eq!(Urls::new("").list("rules"), "/rules");
}

#[test]
fn navigation_resets_page_and_notices_without_losing_other_filters() {
    let params = ListParams::parse(Some("q=a%26b&f_active=true&o=-name&p=9&notice=added"));
    let page = params.with_page(2);
    assert_eq!(page.page, 2);
    assert_eq!(page.notice, None);
    assert_eq!(page.search, "a&b");
    assert_eq!(page.order, params.order);
    assert_eq!(page.filters, params.filters);
    let sorted = params.with_order("name".into());
    assert_eq!(sorted.page, 1);
    assert_eq!(sorted.notice, None);
    assert_eq!(sorted.order_column(), Some(("name".into(), false)));
    assert_eq!(sorted.filters, params.filters);
    assert_eq!(sorted.search, params.search);
    let filtered = params.with_filter("country", Some("GB"));
    assert_eq!(filtered.page, 1);
    assert_eq!(filtered.notice, None);
    assert_eq!(filtered.filters.len(), 2);
    assert_eq!(params.page, 9);
    assert_eq!(
        ListParams::parse(Some(&filtered.encode())).filters,
        filtered.filters
    );
}

#[test]
fn malformed_and_repeated_query_parameters_are_predictable() {
    for raw in ["p=0", "p=-1", "p=bad", "p=18446744073709551616"] {
        assert_eq!(ListParams::parse(Some(raw)).page, 1);
    }
    let params = ListParams::parse(Some("q=first&q=last&f_active=&o=&ignored=value"));
    assert_eq!(params.search, "last");
    assert!(params.filters.is_empty());
    assert_eq!(params.order_column(), None);
    assert_eq!(params.encode(), "q=last");
    assert_eq!(ListParams::parse(None).encode(), "");
    assert_eq!(ListParams::parse(None).page, 1);
    assert_eq!(segment("a/b?x=1#é"), "a%2Fb%3Fx%3D1%23%C3%A9");
}
