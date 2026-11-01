# ReefAdmin

A Django-inspired admin for Axum and SeaORM, with Loco integration.

Register your entities to get server-rendered forms, searchable lists and bulk actions. ReefAdmin uses your application's existing accounts and permissions.

## Features

- Search, filtering, sorting, pagination and CSV export.
- Tera templates, bundled Bootstrap styling and HTMX enhancements.
- Forms with fieldsets, choices, validation and read-only fields.
- Foreign-key selects, autocomplete and read-only child tables.
- Computed columns, boolean toggles and custom bulk actions.
- String and integer primary keys. Composite keys are list-only.
- PostgreSQL array fields, entered as JSON arrays.
- SeaORM model hooks and transactional mutation observers.
- Required actor context and policies with database row scopes.

Tested with PostgreSQL and SQLite. Uploads, editable inlines and many-to-many controls are on the [roadmap](PLAN.md).

## Integration

Your application supplies the database connection, the entity models, an access policy, and session and CSRF middleware. In the example below, `posts`, `MyPolicy`, `require_staff` and `require_csrf` stand for those components.

1. Add the dependency:

   ```toml
   [dependencies]
   reefadmin = "0.1"
   ```

2. Register an entity and build the admin router:

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
   ```

3. Protect the router with your middleware and mount it at the same prefix:

   ```rust,ignore
   let protected = admin
       .layer(axum::middleware::from_fn(require_csrf))
       .layer(axum::middleware::from_fn(require_staff));
   let router = router.nest("/admin", protected);
   ```

The staff middleware must insert a trusted `AdminActor` request extension. Run session middleware before the staff check. In Loco, mount the router in an initializer using `ctx.db`.

The `examples/loco/` directory contains a runnable application with host login, CSRF protection, scoped inventory and integration tests. Its README has setup instructions.

### Permissions

Your application owns accounts, policy decisions and CSRF protection. ReefAdmin requires a policy and enforces it for each operation and each database row. See [permissions](docs/permissions.md) for the policy interface and an example.

These settings limit the available operations and fields for all users:

- `can_add`, `can_edit` and `can_delete` restrict operations.
- `.fields(...)` selects form fields. `.list_display(...)` selects list columns.
- `.export_fields(...)` selects CSV columns. Export is disabled until you set this allowlist.

Also check relationship labels for sensitive data.

### Assets and templates

Bootstrap and HTMX are bundled. Add application assets with `.extra_stylesheet(...)` and `.extra_script(...)`, or override templates and branding. See [assets](docs/assets.md), [templates](docs/templates.md) and [HTMX integration](docs/htmx.md).

## Mutation observers

Register a `MutationObserver` with `.observe_mutations(observer)` to update derived data in the same transaction as an admin write. Use the supplied transaction. An error rolls back both the admin write and the observer's changes.

Custom actions manage their own transactions and do not invoke observers. Bulk deletion commits each row separately. See [architecture](docs/architecture.md) for callback details and side-effect handling.

## Request errors

Internal failures return a generic HTTP 500 response and emit a `tracing` error event. Configure a subscriber in the host to collect diagnostics.

Form validation returns HTTP 422 with field errors. To show a failed add or change write as a form error, register a `ValidationErrorMapper` with `.map_write_errors(mapper)`. The mapper receives the entity name and the error. It returns field messages, or `None` to keep the generic response. Use the key `__all__` for a form-level message. A mapped error renders the form again with HTTP 422. The write is already rolled back. Treat returned messages as public text. See [architecture](docs/architecture.md#request-errors).

Custom actions return `Ok(ActionResult::error(message))` for messages intended for users. A returned `Err(...)` is an internal failure.

Fragment responses carry `X-ReefAdmin-Fragment`. See [HTMX integration](docs/htmx.md).

## Versioning

During 0.1.x, patch releases preserve the public Rust API. Breaking changes use a new 0.x minor version and include migration notes. Security fixes may change unsafe response behaviour.

## Development

Install [mise](https://mise.jdx.dev/getting-started.html), then run these commands from the repository directory:

```sh
mise run setup
mise run fmt
mise run check
```

`setup` installs the pinned tools, including Rust 1.98.1. The crate uses Axum 0.8 and SeaORM 2. `check` runs formatting, workflow and dependency lint, Clippy, tests, rustdoc and package verification. Use `mise tasks ls` to list individual checks.

### PostgreSQL

The PostgreSQL tests need a disposable test database. Replace `YOUR_ROLE` with a PostgreSQL role that can create schemas in that database, then run:

```sh
REEFADMIN_TEST_DATABASE_URL=postgres://YOUR_ROLE@localhost/reefadmin_test \
  mise run check:postgres
```

The tests use isolated schemas and remove them after success or assertion failures.

### Coverage

With the same variable set, `mise run coverage:check` enforces at least 80% line coverage of production Rust. See [testing](docs/testing.md) for reports, scope and optional mutation checks.

### Browser tests

Run `mise run browser:setup` once, then `mise run test:browser`. Node and Chromium are development tools only.

### Loco example

Run `mise run test:loco`, `mise run test:loco:browser` and `mise run test:loco:package`. With the disposable database configured, also run `mise run test:loco:postgres` and `mise run coverage:loco`. The coverage task enforces a separate 80% gate for the example, including its executable.

### Other checks

- Dependency, static and dynamic security checks run locally and need no hosted scanning account. See [security checks](docs/security.md).
- `mise run lint:metrics` reports advisory Clippy complexity and size warnings.
- `mise run release:check` runs the release gates and a publication dry run without uploading the crate. See [Releasing ReefAdmin](docs/releasing.md).

## Origins and upstream contributions

ReefAdmin is adapted from [axum-admin](https://github.com/rinnguyen711/axum-admin). It keeps the entity-registration design and bulk-action model, with revised routing, forms and SeaORM integration.

Upstream provides built-in authentication, Casbin permissions and an ORM-agnostic adapter. ReefAdmin focuses on SeaORM and uses the host application's authentication and permissions. A wholesale merge is therefore a larger discussion, but focused fixes may still be useful upstream. Foreign-key selection, PostgreSQL bindings and transaction handling are candidates. Reproduce each issue on current upstream before proposing a fix. The original revision and implementation details are recorded in [architecture](docs/architecture.md).

## Licence

[MIT](LICENSE), with the original axum-admin attribution retained.
