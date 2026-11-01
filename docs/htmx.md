# HTMX and host integration

HTMX enhances search, filters, sorting, pagination, relationship lookup and inline toggles. Forms for adding, editing, deleting and bulk actions use ordinary submissions. Search and pagination update the URL; reload and history restoration receive a complete page. Changing results clears selected rows; toggling a row clears that row's selection.

List and autocomplete endpoints return HTML fragments when `HX-Request: true`. History restoration requests return complete pages. Ordinary autocomplete requests still return JSON. Response variants use `Vary` and `no-store`. Admin HTML is not cached, and HTMX's local history cache is disabled.

## Sessions and CSRF

Mount every admin route behind the host's session, staff and CSRF middleware, including fragment requests. `HX-Request` is only a rendering hint.

Configure an access policy and supply a trusted `AdminActor` as described in [permissions](permissions.md). After resolving the session, insert the host-issued token into each request before passing it to the admin:

```rust,ignore
request.extensions_mut().insert(reefadmin::CsrfToken {
    field_name: "_csrf".into(),
    value: session_csrf_token,
});
```

Use the field name expected by the host's CSRF validator, avoiding entity fields and admin parameters. ReefAdmin includes the token in ordinary write forms and HTMX toggle requests. It does not generate tokens, validate them or install session middleware. Missing extensions render no token; the host must reject writes when its required token is missing or invalid.

The same request-extension contract works with Axum middleware and a Loco initializer. Middleware must wrap the admin router and run after session resolution. Keep tokens per request, never in `AdminApp` configuration. Templates that replace forms must retain the `csrf.html` include.

## Failures and customisation

Responses to HTMX requests carry `X-ReefAdmin-Fragment: true`. The browser only inserts HTML from responses with this marker. This prevents a redirected login page from replacing a table. Errors leave existing content intact and offer a reload button. Hosts may send `HX-Redirect` for an intentional full-page login redirect. Ordinary redirects still work for conventional forms.

Custom fragment handlers must set the marker, keep normal access checks and return escaped HTML. Reuse the default layout's `admin-request-error` and `admin-status` elements for request errors and status announcements. This marker is not an authentication mechanism.

A new search replaces any search request still in progress. Pending toggle buttons are disabled; failed writes keep the displayed value. Generic write transactions and mutation observers are unchanged. The browser fixture tests host CSRF rejection, expired sessions and HTML responses that lack the fragment marker.

## Browser checks

Run `mise run browser:setup` once and `mise run test:browser` after UI changes. Playwright starts a loopback fixture on port 39127 with disposable SQLite data and test-only session/CSRF middleware. It exercises a custom mount prefix, template/filter overrides, extra CSS, partial updates, history, rapid requests, narrow screens and operation without JavaScript. CI runs these checks alongside `check:postgres`.
