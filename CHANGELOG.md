# Changelog

## 0.1.1 — 2 November 2026

- Add `ValidationErrorMapper` and `AdminApp::map_write_errors` so hosts can show selected add and change write errors, including observer errors, as HTTP 422 form messages. Unmapped errors remain generic HTTP 500 responses.
- Remove the legacy `X-Loco-Admin-Fragment` header and `LOCO_ADMIN_TEST_DATABASE_URL` fallback. Use `X-ReefAdmin-Fragment` and `REEFADMIN_TEST_DATABASE_URL`. This is a one-off compatibility cleanup after the initial release; no deployed consumers rely on these aliases.
- Simplify the README and contributor instructions. Make the extracted-package check use the current package version.

## 0.1.0 — 1 November 2026

- Keep Clippy complexity checks advisory and remove the old rust-code-analysis task. Keep SAST and passive DAST optional; dependency auditing remains a release check. Add portable release verification and publication dry-run tasks.

- Add a runnable inventory example pinned to Loco 1.2.0, with application-owned login, revocable signed sessions, CSRF protection, ownership policies and a transactional archive action. Add HTTP, PostgreSQL, browser, executable and extracted-package checks.
- Add portable dependency audit, OpenGrep rule tests/scans and authenticated ZAP passive scanning through mise. Keep rule coverage and outstanding dependency maintenance warnings explicit.
- Move inline template styles into the bundled stylesheet so hosts can forbid inline styles with CSP.

- Clarify setup, permissions and transaction documentation. Use explicit action labels and describe unavailable admin sections without implying that no entities are registered.

- Use `csv` for export encoding and `email_address` for email validation. Non-empty email values now reject malformed dot/space syntax, display names, domain literals and oversized components; the validator interface and optional blank values are unchanged.

- Separate form widget/relationship construction and list presentation helpers; build pagination links without scanning every page.

- Require explicit `.export_fields(...)` for CSV export; validate field names and select only configured columns. Existing registrations default to no export.

- Rename the package and Rust imports to `reefadmin`. Existing consumers can keep `loco_admin` imports with a Cargo dependency alias. Use `REEFADMIN_TEST_DATABASE_URL` and `X-ReefAdmin-Fragment`; retain the previous names as compatibility aliases. Update the repository URL and CI database names to ReefAdmin.

- Log internal request failures through `tracing` and return a generic HTTP 500 response. Keep field-validation errors and explicit `ActionResult::Error` messages visible to users. See the README migration notes.

- Require host-supplied actor context and an explicit policy; enforce operation and database row scopes across reads and writes.
- Reject mixed unauthorized bulk selections before execution and pass the actor to custom actions. See [migration](docs/permissions.md#migration).

- Add an 80% production Rust coverage gate, combined SQLite/PostgreSQL reports, CI artifacts and advisory mutation checks.
- Expand parsing, validation, direct-request, bulk-failure and PostgreSQL regression tests; isolate test sources from coverage totals.
- Restore keyboard focus after delayed inline toggles.

- Bundle Bootstrap and HTMX, support extra application assets and theme variables.
- Enhance lists, relationship lookup and inline toggles with HTML fragments while preserving ordinary navigation and forms.
- Accept host-issued CSRF tokens through request extensions; add protected browser tests and CI checks.

- Replace MiniJinja with Tera 2, port template macros to components and add `configure_templates` for application filters/functions. Existing template overrides need Tera syntax.

- Introduce ReefAdmin, a standalone SeaORM/Axum admin adapted from axum-admin.
- Reconcile transaction-backed mutation observers, PostgreSQL arrays and typed numeric comparisons, foreign-key selection outside the first page, and optional favicon configuration.
- Add standalone build/package checks and a PostgreSQL regression test.
- Add pinned mise tasks shared with CI, Clippy hygiene and advisory complexity checks, dependency/workflow linting, rustdoc checks and an explicit package-content allowlist.
- Host applications continue to own authentication, policy decisions and CSRF protection.
