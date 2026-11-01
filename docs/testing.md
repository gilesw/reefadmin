# Testing

ReefAdmin requires at least **80% production Rust line coverage** from the combined unit, SQLite and PostgreSQL tests. Browser behaviour and authorization boundaries also need explicit tests; the percentage does not measure those guarantees.

## Commands

Install mise first. From the repository directory, run `mise run setup` to install the pinned tools. Create a disposable PostgreSQL database. Replace `YOUR_ROLE` below with a role that can create schemas in that database.

Run `mise run browser:setup` before your first browser test. Then run:

```sh
export REEFADMIN_TEST_DATABASE_URL=postgres://YOUR_ROLE@localhost/reefadmin_test
mise run check:postgres
mise run coverage:check
mise run test:browser
```

`mise run coverage` collects the same coverage without enforcing the floor; `mise run coverage:html` regenerates HTML from the last successful collection.

Reports are written to `target/coverage/summary.txt`, `target/coverage/coverage.json` and `target/coverage/html/index.html`. The dedicated CI coverage job keeps them for 14 days, including reports from runs that fail the coverage threshold. Browser and ordinary build checks remain separate jobs/steps. Configure `coverage` as a required branch-protection check in the repository settings.

## What is measured

The runner clears old profiles once, runs the library and integration tests, explicitly runs the ignored `postgres` target, then reports their combined coverage. The browser-server fixture remains ignored. The runner includes new integration test targets automatically.

All production Rust modules remain in scope. Test sources live under `tests/` or `src/tests/` and are excluded, along with build output. Cargo LLVM coverage excludes dependencies by default. Request and browser tests check embedded Tera, CSS and JavaScript. The Rust coverage percentage does not include these files. Line coverage is the stable-toolchain gate; no branch-coverage percentage is claimed.

An absent database setting, unavailable PostgreSQL, failed test or failed report command fails the run. Old reports are removed before collection, and concurrent collections are rejected. After a killed process, remove `target/coverage.lock` only after confirming no coverage run is active. `test:coverage-runner` checks failure propagation, report freshness and lock handling without rebuilding Rust for every fault.

## Baseline and improvements

Measured on 1 November 2026 with Rust 1.98.1, cargo-llvm-cov 0.9.1, macOS arm64 and local PostgreSQL. Both measurements exclude test code; the production denominator remained 2,773 lines.

| Area | Baseline | After regression additions |
| --- | ---: | ---: |
| Overall | 88.06% | 93.04% |
| Value parsing/formatting | 60.47% | 93.49% |
| Validators | 86.21% | 100% |
| URL/query handling | 97.35% | 100% |
| Registration validation | 80.97% | 89.43% |
| Persistence adapter | 77.78% | 80.00% |
| Forms | 91.47% | 91.81% |
| Lists/actions | 95.87% | 97.31% |

The first instrumented build took about 63 seconds across the two test phases; a later complete collection took 44.7 seconds while other checks were running. These are local observations, not CI timing guarantees.

Gate verification: a temporary 100% threshold exited with status 1; an unavailable PostgreSQL endpoint exited with status 101 and left no stale reports; the restored 80% check passed. The normal check also runs the coverage-runner fault tests.

After the language review, the suite has 22 unit tests, 51 SQLite request tests, seven PostgreSQL scenarios and nine Chromium scenarios. Combined production coverage is **94.71%** (3,115 of 3,289 lines); the earlier table records the test-tooling milestone before the permission implementation. New cases cover numeric/Unicode boundaries, malformed values, invalid registrations, direct writes to disabled operations, field allowlists, action failures, per-row bulk-delete commits and database constraints. PostgreSQL scenarios each use their own schema, removed after success or an assertion panic.

Remaining uncovered code includes metadata/default-value branches for less-used column types, builder options and configuration errors, plus form/SQL error paths. The HTML report identifies exact lines. Prioritise tests for behaviour that could fail. Do not add assertions solely to raise the coverage percentage. Authorization tests cover required identity, default denial, scoped reads and relationships, direct writes, mixed bulk selections, observers, concurrent actors and PostgreSQL rollback. Host session/CSRF and global capability tests remain separate.

Query-count regressions use SeaORM's metric callback on real SQLite and PostgreSQL connections. A minimal list with two toggle fields runs four queries for both one and 50 rows (count, results and two permission lookups). A 501-row page runs six because IDs are batched in groups of 500. Tests also verify permission-key projection, denied controls, field-specific scopes, string keys and write-time authorization after ownership changes.

Form/list regression tests also cover fieldset ordering, explicit choices taking precedence over autocomplete, stored read-only values after failed submissions, submitted values/errors and bounded pagination at `u64::MAX`. The cleanup reduced `build_fields` cognitive complexity from 55 to 15. Advisory Clippy now reports only two test-code length warnings, with none in production code.

CSV request tests cover quotes, commas, mixed newlines, Unicode, empty cells/results and unchanged formula-like text. Email unit tests cover optional blanks, malformed addresses, display-name rejection, Unicode, quoted local parts and local/domain-label byte limits. The language review passed the Rust/PostgreSQL, strict Clippy, rustdoc, package and coverage checks. All nine browser scenarios also pass with the updated interface text.

## ReefAdmin release candidate

The rename and public-error review passed 81 Rust tests (22 unit, 52 SQLite request and seven PostgreSQL tests), all nine Chromium scenarios, strict Clippy, rustdoc and package verification. Production line coverage is **94.80%** (3,100 of 3,270 lines). The added request test verifies generic HTTP 500 responses for template and database read failures in full-page and HTMX requests. Existing write tests also check that internal observer and database messages stay private while transactions roll back.

A separate consumer crate compiles against the extracted `reefadmin` package with an explicit access policy. This checks package and import compatibility; it does not replace testing an existing host application's session, CSRF and domain integration.

The subsequent naming cleanup passed the complete PostgreSQL merge gate and all nine browser scenarios. Production line coverage is **94.82%** (3,110 of 3,280 lines). Test commands use `REEFADMIN_TEST_DATABASE_URL`.

## Loco integration example

The standalone application under `examples/loco/` boots Loco 1.2.0 with migrations, host-owned signed sessions and a protected ReefAdmin router. Run `mise run test:loco` for formatting, strict Clippy, six HTTP integration tests and an executable provisioning/startup test. Run `mise run test:loco:postgres` with the disposable database setting above for the PostgreSQL integration test. Each PostgreSQL run uses its own temporary schema.

`mise run test:loco:browser` covers three Chromium scenarios, including real login, scoped edits, autocomplete, toggles, logout, denied access, expired browser identity and operation without JavaScript. `mise run test:loco:package` runs the example tests against the extracted Cargo package.

`mise run coverage:loco` combines the example's ordinary and PostgreSQL tests and enforces a separate 80% production Rust line-coverage floor. The executable remains in scope. Reports are under `target/coverage-loco/`; CI retains these alongside the core reports. On 1 November 2026, example coverage was **91.40%** (425 of 465 lines). Core coverage after the SAST-driven control-flow cleanup was **94.79%** (3,109 of 3,280 lines).

See [Local security checks](security.md) for the portable dependency audit, static rule tests and authenticated passive ZAP scan. These complement the permission and rollback tests.

## Mutation checks

`mise run test:mutants` uses cargo-mutants 27.1.0 against a copy of the source tree. It is an optional advisory task, not a CI gate. It checks boolean parsing, truncation, minimum length and pagination, including structural mutations emitted by the tool for the selected files. It runs library tests, with two workers and explicit build/test timeouts.

Results are in `target/mutation/mutants.out/`. Review missed changes separately from unviable mutations or timeouts. The initial 21-mutation run caught ten and missed eleven: notably boolean false handling, the exact minimum-length boundary, page updates and clearing notices. Regression assertions were added for those behaviours. The repeat run caught all 21 mutations, with no misses, timeouts or unviable cases; each scoped pass took about three minutes locally.

A passing scoped run does not establish a whole-crate mutation score. Expand the scope when implementing new behaviour; do not exempt a surviving mutation without checking whether it is equivalent or exposes a real gap.

References: [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov), [cargo-mutants](https://mutants.rs/).
