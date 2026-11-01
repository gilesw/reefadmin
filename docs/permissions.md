# Permissions

The host authenticates users and resolves their current permissions. Loco Admin enforces those decisions through an `AccessPolicy` and a trusted `AdminActor` request extension. It has no user table, password store or login page.

## Mounting

1. Register `.access_policy(YourPolicy)` on `AdminApp`. Construction fails without it.
2. In host session middleware, authenticate the user and insert `AdminActor::new(user.id.to_string()).with_claims(...)` into the request extensions. Resolve claims from trusted application data, never request headers or form fields.
3. Keep the host's CSRF validation around all write routes. `CsrfToken` only supplies form fields; it does not validate requests.

Every admin handler, including assets, requires a nonempty actor ID and `can_enter`. Missing identity returns 401; denied entry or an unavailable operation returns 403. An inaccessible or missing record returns 404. Full-page, JSON and HTMX requests use the same checks.

## Policy

Every default method on `AccessPolicy` denies access. Its `scope` method returns `None` to deny an operation on an entity. To allow the operation, return a SeaORM `Condition` that selects the permitted rows. `Some(Condition::all())` explicitly permits every row. Use registered entity names to distinguish multiple registrations of the same table; qualify SQL columns when necessary.

```rust
use loco_admin::{AccessEntity, AccessPolicy, AdminActor, Operation, Row};
use sea_orm::{Condition, sea_query::{Alias, Expr, ExprTrait}};

struct OwnedPosts;
impl AccessPolicy for OwnedPosts {
    fn can_enter(&self, actor: &AdminActor) -> bool {
        actor.claims["staff"] == true
    }

    fn scope(&self, actor: &AdminActor, entity: AccessEntity<'_>, op: &Operation) -> Option<Condition> {
        if entity.name != "posts" || !matches!(op, Operation::View | Operation::Update) {
            return None;
        }
        let user_id = actor.id.parse::<i64>().ok()?;
        Some(Condition::all().add(
            Expr::col((Alias::new(entity.table), Alias::new("user_id"))).eq(user_id)
        ))
    }

    fn can_write(&self, actor: &AdminActor, _: AccessEntity<'_>, op: &Operation,
        _: Option<&Row>, proposed: &Row) -> bool {
        *op == Operation::Update
            && proposed.get("user_id").and_then(|v| v.as_i64()) == actor.id.parse().ok()
    }
}
```

A row scope is a SQL condition that selects records the actor can access. Loco Admin applies it before counting, paginating or returning records. Every operation must satisfy both its own scope and the `View` scope. `Toggle(field)` must also satisfy the `Update` scope.

Creating a row therefore also requires view permission. Named custom actions use `Action(name)`. CSV and autocomplete have separate `Export` and `Autocomplete` permissions. Global `can_add`, `can_edit`, `can_delete`, export and bulk-delete settings can further restrict access.

`can_write` inspects values keyed by database column name:

- Updates receive the current row and a complete proposed row.
- Inserts receive submitted values before model defaults and hooks run.
- Deletes receive the current row as both arguments.

The check runs inside the write transaction before mutation observers and model hooks. Updates and deletes lock and recheck the scoped row. Inserts and updates also verify the saved row against the operation scope; rejection rolls back the transaction.

Toggle controls use scoped primary-key lookups for the displayed page: one query per displayed, permitted toggle field for up to 500 rows, with additional batches for larger pages. These results are local to one render; writes still recheck authorization inside the transaction. Empty results, non-editable entities and denied toggle operations need no toggle-permission queries.

Navigation and controls reflect operation availability. A permitted scope can match no rows, so navigation may legitimately lead to an empty list. A record-specific write predicate may reject a submitted form even when the form is visible.

## CSV export fields

Exports use UTF-8 with CRLF record separators and standard CSV quoting via [`csv`](https://docs.rs/csv/1.4.0/csv/). Cell values are preserved, including values beginning with spreadsheet formula characters; quoting does not prevent a spreadsheet from treating a value as a formula.

CSV export is disabled until the entity declares `.export_fields(["column", ...])`. Only those database columns are selected and exported, in the configured order. List columns, form fields and read-only settings do not imply export fields. Computed fields, unknown names and duplicates fail configuration validation.

An empty allowlist disables export; `.csv_export(false)` overrides a nonempty allowlist regardless of builder call order. `.csv_export(true)` does not supply an allowlist. The action is hidden and direct export requests are rejected when disabled. Export permission and row scopes still apply, and CSV responses use `Cache-Control: no-store`.

## Relationships and bulk operations

Relationship labels, options and child tables use the target entity's view scope. Hidden targets have no label or detail link. An existing foreign-key value remains available to preserve the relationship during unrelated edits; that ID is already part of the visible parent row. Changing a registered foreign key requires its target to be visible, checked inside the write transaction. Exclude sensitive parent fields from displays and exports as needed.

Bulk requests validate every selected ID before invoking an action or deleting a row. One missing or inaccessible ID rejects the whole selection. IDs are normalized and deduplicated. Bulk deletion still commits each row separately: a later database error, observer failure or concurrent permission change can leave earlier deletions committed. It is not an atomic batch.

Custom actions receive `ActionContext.actor`, the database connection and authorized IDs. The initial permission check does not keep rows locked while the callback runs. If permissions can change, actions must check them again inside their own transaction. Pass the actor to application operations that need the user’s identity.

## Application code remains trusted

Custom SQL search/computed expressions, computed callbacks, actions, model hooks and observers can execute application SQL. Loco Admin does not rewrite or sandbox that code. Scope related queries yourself: an application-supplied aggregate or search expression could otherwise disclose information from hidden rows. Use the supplied transaction for observer database writes. Handle external effects after commit or through an outbox, as described in [persistence](architecture.md#persistence).

Policies are synchronous. Resolve database-backed grants in middleware once per request and carry them in actor claims. Your application decides when to refresh claims and how to revoke permissions. SQL row scopes are evaluated against current database state, while the actor's claims remain a snapshot for that request.

## Migration

To update an existing integration:

1. Register an access policy and supply actor context after authenticating the session.
2. If you construct `ActionContext` directly, populate its `actor` field. Custom action closures can read this field.
3. Choose an explicit `.export_fields(...)` allowlist for each entity that needs CSV export. Exports remain disabled without it.

Bulk requests now fail before execution if any selected ID is missing or inaccessible. Earlier versions skipped missing IDs. No account or database migration is needed.
