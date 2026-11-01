# ReefAdmin

A Django-inspired admin for Axum and SeaORM, with Loco integration.

Register your entities to get server-rendered forms, searchable lists and bulk actions, using your application's existing accounts and permissions.

## Features

- Search, filtering, sorting, pagination and CSV export.
- Tera templates, bundled Bootstrap styling and HTMX enhancements.
- Forms with fieldsets, choices, validation and read-only fields.
- Foreign-key selects, autocomplete and read-only child tables.
- Computed columns, boolean toggles and custom bulk actions.
- String and integer primary keys; composite keys are list-only.
- PostgreSQL array fields entered as JSON arrays.
- SeaORM model hooks and transactional mutation observers.
- Required actor context and policies with database row scopes.

Tested with PostgreSQL and SQLite. Uploads, editable inlines and many-to-many controls are on the [roadmap](PLAN.md).

## Integration

Before you start, supply your application’s database connection, entity models, access policy, and session and CSRF middleware. The example below uses placeholders for these application components.

Add ReefAdmin to your application:

```toml
[dependencies]
reefadmin = "0.1"
```

Register an entity and mount the admin:

```rust,ignore
use reefadmin::{AdminApp, EntityAdmin};

let admin = AdminApp::new(db.clone())
    .access_policy(MyPolicy)
    .title("My site admin")
    .prefix("/admin")
    .register(
        EntityAdmin::from_entity::<posts::Entity>("posts")
            .display_column("title")
            .list_display(["title", "is_published"])
            .fields(["title", "is_published"])
            .export_fields(["title", "is_published"])
            .search_fields(["title"])
            .list_filter(["is_published"]),
    )
    .into_router()?;

// Use your application's staff and CSRF middleware.
let protected = admin
    .layer(axum::middleware::from_fn(require_csrf))
    .layer(axum::middleware::from_fn(require_staff));
let router = router.nest("/admin", protected);
```

`posts`, `MyPolicy`, `require_staff` and `require_csrf` are supplied by your application. The staff middleware must insert a trusted `AdminActor` request extension. Run session middleware before the staff check, and use the same mount prefix as `AdminApp`. In Loco, mount the router in an initializer using `ctx.db`. The repository includes a runnable `examples/loco/` application with host login, CSRF protection, scoped inventory and integration tests. Its README has setup instructions.

Your application owns accounts, policy decisions and CSRF protection. ReefAdmin requires a policy and enforces it for each operation and database row. See [permissions](docs/permissions.md) for the policy interface and an example.

Use these settings to limit the available operations and fields:

- `can_add`, `can_edit` and `can_delete` restrict operations for all users.
- `.fields(...)` selects form fields. `.list_display(...)` selects list columns.
- `.export_fields(...)` selects CSV columns. Export is disabled until you set this allowlist.

Check relationship labels for sensitive data too.

Bootstrap and HTMX are bundled. Use `.extra_stylesheet(...)` and `.extra_script(...)` for application assets, or override templates and branding. See [assets](docs/assets.md), [templates](docs/templates.md) and [CSRF integration](docs/htmx.md).

## Mutation observers

Register a `MutationObserver` with `.observe_mutations(observer)` to update derived data in the same transaction as an admin write. Use the supplied transaction; an error rolls back both changes.

Custom actions manage their own transactions and do not automatically invoke observers. Bulk deletion commits each row separately. See [architecture](docs/architecture.md) for callback details and side-effect handling.

To show a failed add or change write as a form error, register a `ValidationErrorMapper` with `.map_write_errors(mapper)`. The mapper receives the entity name and the error and returns field messages, or `None` for an internal failure. Mapped errors render the form again with HTTP 422; the write is already rolled back. Use the key `__all__` for a form-level message. Treat returned messages as public text. See [architecture](docs/architecture.md#request-errors).

## Migration from loco-admin

The package and Rust import name are now `reefadmin`. Update the dependency and change `use loco_admin::...` to `use reefadmin::...`. To keep existing imports, use a Cargo dependency alias:

```toml
[dependencies]
loco-admin = { package = "reefadmin", version = "0.1" }
```

Use `REEFADMIN_TEST_DATABASE_URL` for the test database and `X-ReefAdmin-Fragment` for fragment responses. The previous `LOCO_ADMIN_TEST_DATABASE_URL` setting remains a fallback; the new setting takes precedence. Responses also include `X-Loco-Admin-Fragment` for older host scripts. The repository is now [gilesw/reefadmin](https://github.com/gilesw/reefadmin).

Internal failures now return a generic HTTP 500 response and emit a `tracing` error event. Configure a subscriber in the host to collect diagnostics. Form validation still returns HTTP 422 with field errors. Custom actions should return `Ok(ActionResult::error(message))` for messages intended for users; returned `Err(...)` values are internal failures. To show selected write errors to users, register a `ValidationErrorMapper` with `.map_write_errors(mapper)`.

During 0.1.x, patch releases preserve the public Rust API. Breaking changes use a new 0.x minor version and include migration notes. Security fixes may change unsafe response behaviour.

## Development

Install [mise](https://mise.jdx.dev/getting-started.html) before running these commands from the repository directory. `setup` installs the pinned tools, including Rust 1.98.1. The crate uses Axum 0.8 and SeaORM 2.

```sh
mise run setup
mise run fmt
mise run check
```

`check` runs formatting, workflow and dependency lint, Clippy, tests, rustdoc and package verification. To include PostgreSQL, create a disposable test database. Replace `YOUR_ROLE` with a PostgreSQL role that can create schemas in that database, then run:

```sh
REEFADMIN_TEST_DATABASE_URL=postgres://YOUR_ROLE@localhost/reefadmin_test \
  mise run check:postgres
```

PostgreSQL tests use isolated schemas and clean them up after success or assertion failures.

For `mise run coverage:check`, set `REEFADMIN_TEST_DATABASE_URL` again or export it in your shell. This command enforces at least 80% production Rust line coverage. See [testing](docs/testing.md) for reports, scope and optional mutation checks.

Browser checks: run `mise run browser:setup`, then `mise run test:browser`. Node and Chromium are development tools only.

For the Loco consumer, run `mise run test:loco`, `mise run test:loco:browser` and `mise run test:loco:package`. With the disposable database configured, run `mise run test:loco:postgres` and `mise run coverage:loco`. The latter enforces a separate 80% gate for the example, including its executable.

Local dependency, static and dynamic security checks are described in [security checks](docs/security.md). They require no hosted scanning account.

Use `mise tasks ls` for individual checks. Optional Clippy complexity and size warnings are available through `mise run lint:metrics`.

For release preparation, see [Releasing ReefAdmin](docs/releasing.md). `mise run release:check` runs the release gates and a publication dry run without uploading the crate.

## Origins and upstream contributions

Adapted from [axum-admin](https://github.com/rinnguyen711/axum-admin). ReefAdmin keeps its entity-registration design and bulk-action model, with revised routing, forms and SeaORM integration.

Upstream provides built-in authentication, Casbin permissions and an ORM-agnostic adapter. ReefAdmin focuses on SeaORM and uses the host application's authentication and permissions. These design differences make a wholesale merge a larger discussion, but focused fixes may still be useful upstream.

Foreign-key selection, PostgreSQL bindings and transaction handling are potential contributions. Reproduce each issue on current upstream before proposing a fix. The original revision and implementation details are recorded in [architecture](docs/architecture.md).

## Licence

[MIT](LICENSE), with the original axum-admin attribution retained.
