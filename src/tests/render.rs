use super::Renderer;
use tera::{Tera, context};

#[test]
fn html_escaping_preserves_urls_and_requires_explicit_trust() {
    let renderer = Renderer::new(
        Tera::new(),
        vec![(
            "probe.html".into(),
            "{{ value }}|{{ value | safe }}|{{ url }}".into(),
        )],
    )
    .unwrap();
    let body = renderer
        .render(
            "probe.html",
            context! {
                value => "<b title=\"'\">&</b>",
                url => "/admin/items?q=a&b=c",
            },
        )
        .unwrap();
    assert_eq!(
        body,
        "&lt;b title=&quot;&#39;&quot;&gt;&amp;&lt;/b&gt;|<b title=\"'\">&</b>|/admin/items?q=a&amp;b=c"
    );
}

#[test]
fn overrides_resolve_together_and_missing_values_are_explicit() {
    let renderer = Renderer::new(Tera::new(), vec![
        ("layout.html".into(), "{% extends \"custom.html\" %}{% block content %}override{% endblock %}".into()),
        ("custom.html".into(), "shell:{% block notices %}{% endblock %}{% block title %}{% endblock %}{% block content %}{% endblock %}".into()),
        ("probe.html".into(), "{% if missing is defined %}unexpected{% endif %}{% if empty == none %}null{% endif %}".into()),
    ]).unwrap();
    assert_eq!(
        renderer.render("layout.html", context! {}).unwrap(),
        "shell:override"
    );
    assert_eq!(
        renderer
            .render("probe.html", context! { empty => &Option::<String>::None })
            .unwrap(),
        "null"
    );
    assert!(
        Renderer::new(
            Tera::new(),
            vec![("bad.html".into(), "{% extends \"absent.html\" %}".into())]
        )
        .is_err()
    );
}
