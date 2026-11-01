# Architecture and origin

ReefAdmin is a server-rendered administration library for Axum and SeaORM, with Loco integration. Applications register entities through `EntityAdmin`, configure an `AdminApp`, and mount the resulting router behind their existing access controls. Loco integration uses the same Axum router; the library does not depend on Loco.

## Origin

ReefAdmin is adapted from axum-admin v0.1.1, commit `01dae80fb35270612d1f217b41383a0ceaeefe4e`. It retains the builder and entity-registration design, bulk-action model, CSV export and validators. Routing, templates, form handling and the SeaORM adapter have been substantially revised. It targets Axum 0.8 and SeaORM 2.

ReefAdmin contains adapted source; it does not depend on the axum-admin crate. There is no automatic upstream synchronisation or claimed upstream affiliation. Preserve the upstream MIT copyright notice and terms when distributing the library.

## Application boundary

The host owns users, sessions, authentication, authorization, CSRF protection, storage configuration and domain rules. It may supply a per-request `CsrfToken` extension for hidden form fields; validation remains in host middleware. See [HTMX integration](htmx.md). Protect all admin routes, including autocomplete, exports, inlines and bulk actions. Settings such as `can_edit` restrict all users. Use an access policy for decisions about individual users.

Application-specific registration and custom business operations belong in the host. The library owns generic entity forms, lists, filters, actions, rendering and extension interfaces. Custom actions receive the authenticated actor, a database connection and authorized selected IDs. [Permissions](permissions.md) describes required identity, policy scopes and callback responsibilities.

## Persistence

Generic writes invoke SeaORM active-model behaviour. Mutation observers run inside the same transaction as inserts, updates and deletes, including writes initiated by toggles and bulk deletion. Updates and deletes lock the affected row before observer work. An observer error rolls back that row's operation and observer database writes.

For inserts and updates, `before` receives proposed values. It can query existing state through the supplied transaction. For deletions, it receives the stored row. `after` receives the saved row, or null after deletion.

A database rollback cannot undo an email or an external API call. Send these after the transaction commits, or record pending work in an outbox table within the transaction.

Custom actions own their transactions and do not automatically invoke observers. Bulk selections are authorized before execution. Bulk deletion commits one row at a time; a later failure does not roll back earlier rows.

PostgreSQL arrays use JSON-array form input. Numeric comparison parameters preserve the column's integer/float width. Foreign-key selects retain the saved or submitted value even when it falls outside the first page of options.

## Request errors

Unexpected database, template, observer and action failures return a generic HTTP 500 response. Diagnostics are emitted through `tracing`; configure a subscriber in the host application. These errors do not expose their details in HTML or HTMX responses.

Field validation returns HTTP 422 with submitted values and field messages. Custom actions can return `Ok(ActionResult::error(message))` for an expected failure that users need to see. Treat those messages as public text. An `Err(AdminError)` is an internal failure, except `AccessDenied`, which returns HTTP 403.

## Rendering and assets

Templates and admin JavaScript are embedded in the crate. Tera 2 rendering is internal and does not dictate the host's template engine. Applications can configure title, mount prefix, site link, stylesheet, script, favicon and template overrides. Bootstrap and HTMX are bundled and served under the admin prefix. HTMX enhances list navigation, relationship lookup and inline toggles; full pages and ordinary form submissions remain available. See [template migration](templates.md) for overrides and engine configuration.

## Current limits

Composite-key entities are list-only. Uploads, editable inlines, many-to-many controls, domain-operation adapters and audit hooks remain planned extensions. PostgreSQL and SQLite are tested; support for other backends is not claimed.

## Verification

The regression suite covers unit behaviour, SQLite request flows and PostgreSQL-specific field types and transactions. `mise run check:postgres` runs the ordinary Rust gate with an explicitly configured test database. CI also runs `coverage:check` (80% production Rust line coverage) and browser checks. See [testing](testing.md) for details. See the [development instructions](../README.md#development) for setup and commands.
