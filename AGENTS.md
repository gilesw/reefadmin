# Working on ReefAdmin

ReefAdmin targets Loco applications and is built on SeaORM and Axum. Keep the core usable as an Axum router without requiring a Loco dependency.

Preserve application-owned authentication, authorization and domain operations. Do not add a second login system. Never present an unprotected admin mount as a production integration example.

Keep changes compatible with existing consumers. See PLAN.md and docs/architecture.md for scope, boundaries and upstream provenance.

Use `mise run setup` to install the pinned tools, `mise run fmt` to format, and `mise run check` for the merge gate. With a disposable PostgreSQL test database, set `LOCO_ADMIN_TEST_DATABASE_URL` and run `mise run check:postgres`, the ordinary Rust gate used by CI. It adds the ignored PostgreSQL integration tests; see README.md.

The manifest enables Clippy hygiene lints; warnings fail `mise run lint`. Complexity/size reports (`mise run lint:metrics` and `mise run metrics:rust-code-analysis`) are advisory, not merge gates. Keep the Rust versions in Cargo.toml, rust-toolchain.toml and mise.toml aligned. New runtime assets must be included in the Cargo package allowlist.

Run `mise run coverage:check` with the same disposable database for the 80% production Rust line-coverage gate. Run `mise run test:browser` for UI changes. See docs/testing.md for scope, report paths and the advisory mutation task. Do not exclude production code to meet the threshold.

Review new or changed documentation and user-facing text using the [Google developer documentation style guide](https://developers.google.com/style) and ASD-STE100 clarity principles. Use short, direct sentences, consistent terms and clear procedural steps. Preserve technical accuracy, exact API names and security requirements. Retain the project’s British spelling and established software terms. This is not a formal ASD-STE100 compliance claim. See PLAN.md slice 7 for the initial review scope and completion criteria.
