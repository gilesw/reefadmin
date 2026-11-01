# Changelog

## 0.1.0 unreleased

- Clarify setup, permissions and transaction documentation. Use explicit action labels and describe unavailable admin sections without implying that no entities are registered.

- Use `csv` for export encoding and `email_address` for email validation. Non-empty email values now reject malformed dot/space syntax, display names, domain literals and oversized components; the validator interface and optional blank values are unchanged.

- Separate form widget/relationship construction and list presentation helpers; build pagination links without scanning every page.

- Require explicit `.export_fields(...)` for CSV export; validate field names and select only configured columns. Existing registrations default to no export.

- Rename the package to `loco-admin` (Rust imports: `loco_admin`) and target Loco applications. Rename the test database setting to `LOCO_ADMIN_TEST_DATABASE_URL` and the fragment header to `X-Loco-Admin-Fragment`.

- Require host-supplied actor context and an explicit policy; enforce operation and database row scopes across reads and writes.
- Reject mixed unauthorized bulk selections before execution and pass the actor to custom actions. See [migration](docs/permissions.md#migration).

- Add an 80% production Rust coverage gate, combined SQLite/PostgreSQL reports, CI artifacts and advisory mutation checks.
- Expand parsing, validation, direct-request, bulk-failure and PostgreSQL regression tests; isolate test sources from coverage totals.
- Restore keyboard focus after delayed inline toggles.

- Bundle Bootstrap and HTMX, support extra application assets and theme variables.
- Enhance lists, relationship lookup and inline toggles with HTML fragments while preserving ordinary navigation and forms.
- Accept host-issued CSRF tokens through request extensions; add protected browser tests and CI checks.

- Replace MiniJinja with Tera 2, port template macros to components and add `configure_templates` for application filters/functions. Existing template overrides need Tera syntax.

- Introduce Loco Admin, a standalone SeaORM/Axum admin adapted from axum-admin.
- Reconcile transaction-backed mutation observers, PostgreSQL arrays and typed numeric comparisons, foreign-key selection outside the first page, and optional favicon configuration.
- Add standalone build/package checks and a PostgreSQL regression test.
- Add pinned mise tasks shared with CI, Clippy hygiene and advisory complexity checks, dependency/workflow linting, rustdoc checks and an explicit package-content allowlist.
- Host applications continue to own authentication, policy decisions and CSRF protection.
