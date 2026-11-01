# Working on ReefAdmin

## Scope and boundaries

ReefAdmin targets Loco applications and is built on SeaORM and Axum. Keep the core usable as a plain Axum router. The core must not depend on Loco.

The host application owns authentication, authorization and domain operations. Do not add a second login system. Never show an unprotected admin mount as a production integration example.

Keep changes compatible with existing consumers. See PLAN.md for scope and docs/architecture.md for boundaries and upstream provenance.

## Commands

| Task | Command |
| --- | --- |
| Install the pinned tools | `mise run setup` |
| Format | `mise run fmt` |
| Merge gate | `mise run check` |
| Merge gate with PostgreSQL | `mise run check:postgres` |
| Coverage gate | `mise run coverage:check` |
| Browser tests | `mise run test:browser` |

`check:postgres` is the gate CI runs. It adds the PostgreSQL integration tests, which are ignored by `check`. Both `check:postgres` and `coverage:check` need `REEFADMIN_TEST_DATABASE_URL` set to a disposable PostgreSQL database. See README.md for setup.

Run `mise run test:browser` after any UI change.

## Rules for code

- Clippy hygiene lints are enabled in the manifest. A warning fails `mise run lint`.
- `mise run lint:metrics` reports Clippy complexity and size warnings. These are advisory and not part of the merge gate.
- Keep the Rust version identical in Cargo.toml, rust-toolchain.toml and mise.toml.
- Add every new runtime asset to the Cargo package allowlist.
- The coverage gate requires 80% line coverage of production Rust. Do not exclude production code to reach it. See docs/testing.md for scope, report paths and the advisory mutation task.

## Rules for documentation

Review new or changed documentation and user-facing text against the [Google developer documentation style guide](https://developers.google.com/style) and ASD-STE100 clarity principles:

- Use short, direct sentences and clear procedural steps.
- Use one term for one thing.
- Keep technical accuracy, exact API names and security requirements.
- Keep the project's British spelling and established software terms.

This is not a formal ASD-STE100 compliance claim. PLAN.md slice 7 records the initial review scope and completion criteria.

## Rules for checks

Every quality and security check must run locally through mise with open-source tools. Do not require a hosted scanning account, a source upload or a GitHub-specific security service. CI calls the same mise tasks.
