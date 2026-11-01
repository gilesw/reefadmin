# Working on ReefAdmin

## Boundaries

ReefAdmin targets Loco applications and is built on SeaORM and Axum. The core must work as a plain Axum router and must not depend on Loco.

The host application owns authentication, authorization and domain operations. Do not add a login system. Never show an unprotected admin mount as an integration example.

Keep changes compatible with existing consumers. See PLAN.md for scope and docs/architecture.md for boundaries and upstream provenance.

## Commands

| Task | Command |
| --- | --- |
| Install the pinned tools | `mise run setup` |
| Format | `mise run fmt` |
| Merge gate | `mise run check` |
| Merge gate with PostgreSQL (what CI runs) | `mise run check:postgres` |
| Coverage gate | `mise run coverage:check` |
| Browser tests | `mise run test:browser` |

`check:postgres` and `coverage:check` need `REEFADMIN_TEST_DATABASE_URL` set to a disposable PostgreSQL database. See README.md.

Run the browser tests after any UI change.

## Code

- Clippy hygiene lints are enabled. A warning fails `mise run lint`.
- `mise run lint:metrics` is advisory.
- Keep the Rust version identical in Cargo.toml, rust-toolchain.toml and mise.toml.
- Add every new runtime asset to the Cargo package allowlist.
- The coverage gate is 80% of production Rust lines. Do not exclude production code to reach it.

## Documentation

Follow the [Google developer documentation style guide](https://developers.google.com/style) and ASD-STE100 clarity principles:

- Short, direct sentences and clear procedural steps.
- One term for one thing.
- Exact API names and security requirements.
- British spelling.

## Checks

Every quality and security check runs locally through mise with open-source tools. Do not require a hosted scanning account, a source upload or a GitHub-specific service. CI calls the same tasks.
