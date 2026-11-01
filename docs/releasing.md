# Releasing ReefAdmin

The initial release candidate is `reefadmin` 0.1.0. The core supports Axum 0.8 and SeaORM 2, with Rust 1.98.1 as the declared minimum. The standalone Loco 1.2.0 example stays outside the published crate. Hosts own authentication, authorization and CSRF validation.

## Prepare and verify

1. Run `mise run setup` and `mise run browser:setup`. Create a disposable PostgreSQL database with a role that can create schemas.
2. Export `REEFADMIN_TEST_DATABASE_URL` for that database, then run `mise run release:check`.
3. Review the changelog, public API and migration notes. Check the Cargo package with `cargo package --list --allow-dirty`. The archive must contain the templates, static assets and bundled third-party licences. It must exclude the Loco workspace, security rule licences and local reports.
4. Review dependency audit warnings. The Loco example currently has the unsuppressed `unsafe-libyaml` maintenance advisory RUSTSEC-2024-0449. Its optional JWT dependency chain is disabled. See [security checks](security.md).

`release:check` runs the Rust/PostgreSQL gates, Loco consumer tests, both browser suites, extracted-package tests, both 80% coverage gates, dependency auditing and `cargo publish --dry-run`. It permits uncommitted edits for candidate review and never uploads a package. Reports are described in [testing](testing.md). SAST, passive DAST, mutation checks and Clippy complexity warnings remain optional.

## Publish an approved candidate

Publishing and pushing the release tag are separate, explicit release actions. These steps work locally without a hosted CI or release service.

1. Confirm the `reefadmin` name is available on crates.io and that the publishing account has permission. A missing index entry is only an availability indication; a dry run does not establish publishing rights or reserve a name. Authenticate with `cargo login` or the configured Cargo credential provider. Never commit a token.
2. Set the changelog's release date. Change the README dependency examples to `reefadmin = "0.1"`. Keep the Loco example's path dependency for repository development.
3. Review and commit all release files, including the example lockfile. Ensure `git status --porcelain` is empty. Run `mise run release:check` on that commit. Any subsequent source change requires the relevant checks again.
4. Publish with `cargo publish --locked`. Do not use `--allow-dirty` or `--no-verify` for the real upload. If the command times out, check crates.io before retrying: an upload may have succeeded.
5. Confirm version 0.1.0 is visible on crates.io and that docs.rs builds its API documentation. Tag the tested commit with `git tag -a v0.1.0 -m 'ReefAdmin 0.1.0'`, then push the release commit and tag to the established remote.
6. Verify a consuming application resolves `reefadmin = "0.1"` from the registry. Record any release issue before starting the next version.

During 0.1.x, patch releases preserve the public Rust API. Breaking changes use a new 0.x minor version with migration notes. Security fixes may change unsafe response behaviour. An uploaded version cannot be overwritten; publish a corrected patch release if needed.
