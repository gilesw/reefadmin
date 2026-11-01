# ReefAdmin roadmap

Updated 1 November 2026.

Build a reusable admin for Loco applications, using SeaORM and mounting as an Axum router. Tera, Bootstrap and HTMX, the 80% coverage gate, and actor-aware authorization are implemented. Domain-operation adapters and audit hooks are next.

## Scope and boundaries

ReefAdmin owns entity registration, admin pages, field rendering, validation, search/filter/sort/pagination, relationship controls, action dispatch and extension interfaces. The host application owns users, sessions, domain rules, storage configuration and permission decisions. Administration must integrate with existing authentication rather than create a second account system.

Keep one crate supporting Axum 0.8 and SeaORM 2. The internal renderer uses Tera 2, with Bootstrap styling and HTMX for partial page updates. Hosts can use another template engine. Loco is an integration target, not a core dependency. Other ORMs, HTTP frameworks, a separate SPA and a standalone hosted service are outside the initial scope.

## Code review cleanup — 1 November 2026

Work in small slices, preserving host authentication, database row scopes and model hooks. No new framework layers or crate split are needed. Baseline: 66 Rust tests, nine browser scenarios and 93.67% production line coverage. `metrics:rust-code-analysis` identifies `build_fields` as the main hotspot (cognitive complexity 55, cyclomatic complexity 44); Clippy flags its 163 code lines and `render_list` at 118.

### Slice 1 — explicit CSV export fields — implemented

- Add `.export_fields(["column", ...])`, with no implicit column selection. An empty or absent allowlist disables export, even if `.csv_export(true)` is used. `.csv_export(false)` remains an overriding switch.
- Only real database columns are supported initially. Validate unknown, duplicate and computed field names at router construction; preserve configured order.
- Keep export permission, view scope, filters, ordering and selected-ID checks. The export query should select only allowed columns, without evaluating unrelated computed columns.
- Test default denial, action visibility, forged direct requests, field omission/order, configuration errors, disabled exports and scoped selections. Document the migration: existing registrations must explicitly opt in; form/list/read-only settings are independent.
- Run the Rust/PostgreSQL, coverage and browser gates. Check consumer compilation; leave their exports disabled until registrations explicitly choose fields.

Implemented with 70 passing Rust tests, nine browser scenarios and 93.79% production line coverage. Hidden fields cannot enter a CSV merely because they exist in the model.

### Slice 2 — batch presentation permission checks — implemented

Display permissions now use scoped primary-key queries per toggle field, in batches of up to 500 displayed IDs. Toggle scopes already include update scope, so duplicate checks are removed. Transaction-time write authorization is unchanged. Query-count tests verify two permission queries for both one-row and 50-row pages with two toggle fields, and four for 501 rows. Further tests cover hidden/denied controls, pagination, actors, ownership changes before writes, HTMX updates, string keys and PostgreSQL integer bindings. Verification: 75 Rust tests, nine browser scenarios and 93.83% production line coverage; Clippy, rustdoc and package verification pass.

### Slice 3 — simplify form and list construction — implemented

Form construction now separates relationship loading, widget selection, computed values and fieldset assembly. List headers and pagination have focused helpers; pagination builds only the first, last and nearby links instead of scanning every page. Regression tests cover widget precedence, read-only values, failed submissions and pagination at very large page counts.

`rust-code-analysis` reports `build_fields` cognitive complexity reduced from 55 to 15 and cyclomatic complexity from 44 to 21. Advisory Clippy reports no production-code warnings; two test helpers/scenarios still exceed its length threshold. Verification: 77 Rust tests, nine browser scenarios and 94.80% production line coverage; strict Clippy, rustdoc and package verification pass.

### Slice 4 — use focused parsing/writing crates — implemented

Replaced manual CSV escaping with `csv` 1.4 (MIT/Unlicense) and approximate email validation with `email_address` 0.2.9 (MIT, default Serde feature disabled). The validator interface, optional blank values, CSV column scopes and CRLF record separators remain unchanged. Email validation requires a bare address with a dotted domain; Unicode and quoted local parts are supported, while surrounding whitespace, display names and domain literals are rejected.

Regression tests cover CSV commas, quotes, CR/LF, Unicode, empty cells/results, email whitespace and malformed syntax, and byte-length boundaries. Spreadsheet formula handling remains a separate explicit export policy; CSV quoting preserves cell content. Verification: 80 Rust tests pass, including seven PostgreSQL scenarios; strict Clippy, rustdoc, package verification and the coverage gate pass (94.71%).

### Slice 5 — simplify partial model construction

Investigate SeaORM's column-setting APIs as a replacement for fabricated JSON placeholder values followed by unsetting fields. Prototype against renamed serde fields, dates, enums, arrays, defaults and model hooks on SQLite/PostgreSQL. Replace the workaround only if those behaviours remain correct; do not build a second ORM.

### Slice 6 — separate public errors from diagnostics — implemented

Unexpected request failures emit a `tracing` error event and return a generic HTTP 500 response. Database, template, observer and custom-action errors no longer appear in response bodies. Hosts configure their own tracing subscriber. Field validation keeps HTTP 422 and submitted values; explicit `ActionResult::Error` messages remain visible. An `AdminError::AccessDenied` still returns HTTP 403.

Regression checks cover full pages and HTMX fragments, read/template failures, database constraints, custom actions and observer rollback. See the README request errors section.

Hosts can register a `ValidationErrorMapper` to classify add and change write errors, including observer errors, as HTTP 422 form messages. The mapper receives the entity name and returns public messages; unmapped errors stay generic. Regression checks cover add, change, rollback, form-level messages and the generic fallback.

### Slice 7 — review documentation and interface language — implemented

Review README, public Rust API documentation, guides in `docs/`, setup/migration instructions and user-facing labels, help text and errors. Use the [Google developer documentation style guide](https://developers.google.com/style) for software documentation and [ASD-STE100](https://www.asd-ste100.org/) principles for clear instructions. This is an editorial review, not a claim of formal ASD-STE100 compliance.

- Use short, direct sentences, active voice and consistent terms. Explain unfamiliar concepts before using them in instructions.
- Put prerequisites before steps. Make each step a clear action and state the expected result where useful.
- Follow Google's guidance for headings, code identifiers, links, UI references and accessible, inclusive language.
- Remove repetition, vague claims and unnecessary caveats. Preserve technical constraints, security boundaries and migration requirements.
- Preserve exact API names, commands and third-party attribution. Prefer technical accuracy and established project terminology when style guidance conflicts.

Reviewed on 1 November 2026:

- README and all six guides: `architecture.md`, `assets.md`, `htmx.md`, `permissions.md`, `templates.md` and `testing.md`.
- Public API documentation in `src/`, all 13 templates, `static/admin.js`, and built-in validation, action and request messages.
- Setup and migration instructions, with a check of documented mise task names and local Markdown link targets.

Corrections put prerequisites before commands, split dense permission descriptions, explain transaction boundaries and replace vague action labels. The empty admin page now describes unavailable sections without claiming that no entities are registered. The permissions example was missing `ExprTrait`; it now compiles against the locked dependencies.

References: [Google style highlights](https://developers.google.com/style/highlights), [Google procedures](https://developers.google.com/style/procedures), and the sentence guidance in [ASD-STE100 Issue 9](https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf). Retained project conventions include British spelling, established software terms and exact API identifiers. The review did not enforce the full STE dictionary or every sentence-length rule. Third-party licence text and bundled vendor code were left unchanged.

Verification: 80 Rust tests and nine browser scenarios pass. Strict Clippy, rustdoc, package verification and the 80% coverage gate pass; measured line coverage is 94.71%. The permissions example compiles separately. Integration snippets that require host models and middleware remain explicitly marked as examples, not standalone programs.

Slice 6 subsequently separated internal diagnostics from public errors. New or changed text must follow the review guidance in AGENTS.md. No style-lint dependency was added.

## Completed foundation

- Standalone Cargo package with embedded templates and JavaScript, upstream attribution and an explicit package allowlist.
- Entity forms, lists, filters, choices, validators, computed columns, foreign-key selection/autocomplete, bulk actions, CSV export and read-only child tables.
- SeaORM persistence hooks and transaction-backed mutation observers.
- PostgreSQL arrays and typed numeric bindings, with regression coverage for foreign-key values outside the first page.
- Pinned mise tasks shared with CI, strict Clippy hygiene checks, optional complexity reports, rustdoc and package verification.
- Unit/SQLite tests plus an explicit PostgreSQL regression test; existing consumer integrations validated.

See [architecture](docs/architecture.md) for current limits and transaction semantics. Successful extraction and testing do not imply that the remaining public-release gates are complete.

## Implemented: Tera, Bootstrap and HTMX

Each slice should be independently reviewable and leave a working admin. Keep ordinary links and form submissions available throughout. Do not change persistence or permission semantics as part of the UI work.

### 1. Switch the renderer to Tera — implemented

- Replace MiniJinja with Tera 2 and port the six embedded templates and render contexts.
- Preserve named template overrides, existing routes, form values and error responses. Document syntax changes for applications with overrides.
- Provide a way to register application filters/functions before templates are validated. Keep engine configuration separate from Loco.
- Define and test the escaping policy, including URLs, quotes, user content and explicitly trusted markup. Check missing/null values and template override inheritance.

Done when existing request tests pass through Tera, rendering regressions are covered, and MiniJinja is removed. No visual redesign or HTMX in this slice.

### 2. Bundle the default assets — implemented

- Vendor pinned Bootstrap 5.3 CSS and bundle JS, plus HTMX, with upstream license notices and a documented update procedure. Load HTMX when the first enhancement lands in slice 4.
- Serve embedded assets under the configured admin prefix. Preserve the existing admin.js URL; give vendor assets versioned URLs, correct content types and suitable cache headers.
- Make a new admin work without host-provided Bootstrap, a CDN or a consumer frontend build.
- Retain existing stylesheet/script replacement options. Add separate options for additional application CSS/JS and define their load order.

Done when a packaged crate serves every required asset from a non-default mount prefix, defaults work offline, and explicit host asset overrides still work.

### 3. Prepare templates and browser behaviour — implemented

- Split list results, pagination, notices and relationship options into reusable Tera partials with stable DOM targets.
- Add layout blocks for application styles/scripts and a small set of theme variables using Bootstrap's existing conventions.
- Make row selection and confirmation handlers work after content replacement, using delegated events where appropriate. Initialise and dispose Bootstrap widgets when needed.
- Keep standard Bootstrap markup; check labels, keyboard focus, empty states and narrow screens.

Done when full-page rendering still works, customisation has a small example, and replaced content retains its browser behaviour. Avoid introducing a general widget framework here.

### 4. Enhance list search, filters and pagination — implemented

- Load HTMX and add delayed search, filter changes, sorting and pagination that replace the results region.
- Render full pages for ordinary requests and partials for HTMX requests using the same query and rendering logic. Set appropriate cache variation for the response variants.
- Preserve query parameters and shareable URLs. Define history restoration so reload, back and forward display complete pages; reset pagination when search or filters change.
- Show loading and request-error states. Define whether row selection clears on a results change and make that visible.

Done when browser checks cover search/filter/page combinations, back/forward/reload and operation with JavaScript disabled. Request tests verify equivalent rows and permissions in full and partial responses.

### 5. Enhance relationship lookup — implemented

- Add HTML options for HTMX-driven autocomplete, while preserving the existing JSON endpoint contract for consumers.
- Cancel or supersede stale searches so a slow earlier response cannot replace newer results.
- Preserve selected IDs and labels, including saved values outside the first page. Use the same scoped lookup and validation as ordinary requests.
- Provide a usable ordinary form control when JavaScript is unavailable; do not require loading every related row.

Done when browser checks cover rapid typing, keyboard selection, empty/error results and existing selections. Retain regression coverage for values outside the initial option page.

### 6. Enhance inline toggles — implemented

- Return the updated cell/row and feedback after a successful toggle; keep the ordinary POST-and-redirect path.
- Carry the host's CSRF token on enhanced writes and apply the same authentication and authorization middleware. An HTMX header grants no access.
- Prevent duplicate clicks while a request is pending. Leave the displayed value unchanged on failure and show an actionable error.
- Handle expired sessions and denied requests explicitly, rather than inserting a login page into a table cell.

Done when tests cover success, rejection, missing/invalid CSRF, session expiry and mutation rollback. Keep create/edit/delete and bulk-action forms conventional for this milestone.

### 7. Verify integrations and document the UI contract — implemented

- Exercise protected plain Axum and Loco integrations with defaults, a custom mount prefix, template overrides, extra assets and custom Tera filters.
- Document asset configuration, fragment behaviour, CSRF integration and migration from MiniJinja overrides. Update architecture and README to describe implemented behaviour.
- Run browser checks with and without JavaScript, including keyboard navigation and focus after swaps. Add a small repeatable browser smoke suite for the enhanced flows through mise and CI; browser tooling must remain a development dependency.
- Verify licenses and packaged assets, and run the PostgreSQL gate plus existing consumer checks.

Done when consumers can upgrade without supplying default assets and both normal and enhanced requests behave correctly.

Validation: the Rust gate and PostgreSQL regression pass, along with nine Chromium scenarios covering enhanced and ordinary requests. Browser checks run through mise and CI; see [HTMX integration](docs/htmx.md).

Run `mise run check` for each slice, with focused regression tests for changed behaviour. Use `mise run check:postgres` for the final integration slice. Work in order; each completed slice is a safe stopping point.

Later, consider enhanced create/edit forms, modal workflows and richer widgets. Enhanced forms must deliberately handle HTTP 422 validation responses, submitted values, redirects and focus before they replace ordinary submissions. They are not required for this milestone.

## Test quality and coverage

T1–T5 are implemented: production line coverage is 93.67%, with 19 unit tests, 42 SQLite request tests, five PostgreSQL scenarios and nine Chromium scenarios. See [testing](docs/testing.md) for evidence and commands. The scoped mutation review now catches all 21 mutations, up from ten. The target is a dependable regression suite and an 80% production Rust line-coverage floor. This is ReefAdmin's policy, not an official Rust standard.

### T1. Measure a reproducible baseline — implemented

- Pin `cargo-llvm-cov` through mise and add `llvm-tools-preview` to the existing Rust toolchain configuration. Keep Rust versions aligned. Retain `cargo test`; Nextest is not required for coverage.
- Add a coverage runner shared by local tasks and CI. Clean old profiles once, run unit/SQLite tests, then explicitly run the ignored PostgreSQL test under the same instrumentation before generating the combined report.
- Select the PostgreSQL test target by name. Do not run all ignored tests: the ignored browser server is a long-running fixture, not a coverage test.
- Measure ReefAdmin's production Rust sources. Exclude test fixtures, test-only code, dependencies, generated output and vendored assets from the denominator. Inspect the report for inline unit-test code; if it is counted, separate test modules into test-only files rather than excluding their production modules.
- Generate summary, HTML and machine-readable reports under `target/`. Record the overall baseline, per-file results, missing paths, tool versions and runtime in `docs/testing.md`.

Done when a clean run includes both databases and produces an auditable report. Missing PostgreSQL configuration or failed tests must fail the combined run. No threshold is enabled until the baseline is known.

### T2. Close meaningful gaps — implemented

Use the report to prioritise untested behaviour, not to create a test for every line. Split the broad PostgreSQL scenario into focused tests with isolated schemas and reliable cleanup.

| Area | Required regression cases |
| --- | --- |
| Parsing and validation | Numeric boundaries/overflow, malformed dates and arrays, null versus empty values, invalid choices and unknown fields |
| Lists and relationships | Empty results, pagination boundaries, invalid sort/filter inputs, missing related records and selected values beyond the option limit |
| Writes and actions | Read-only field tampering, disabled operations through direct requests, missing IDs, duplicate selections and custom-action errors |
| Transactions | Create/update/delete/toggle observer failures, rollback of observer writes, and the documented per-row bulk-delete partial-failure behaviour |
| PostgreSQL | Typed numeric comparisons, arrays, string/integer keys, null handling and constraint failures against a real database |
| Rendering and HTTP | Untrusted field values, template failures, full-page/fragment equivalence, missing/invalid CSRF and expired sessions |

Use small unit tests for pure logic, real request/database tests for persistence and access behaviour, and browser tests for interaction. Assert status codes, visible results and database effects; avoid tests that only repeat implementation details. Add property tests for parsing or URL round trips only where generated cases add useful coverage.

Done when production line coverage reaches at least 80%, the important failure paths above are covered, and ordinary Rust/PostgreSQL/browser checks pass. Do not lower the threshold, hide production code or add empty assertions to meet the number.

### T3. Enforce the same rule locally and in CI — implemented

Tasks:

| Task | Purpose |
| --- | --- |
| `coverage` | Clean combined unit/SQLite/PostgreSQL run and reports; requires a disposable PostgreSQL database |
| `coverage:check` | The same run and reports, failing below 80% line coverage |
| `coverage:html` | Render the last collected coverage data as HTML without rerunning tests |

- Enforce `--fail-under-lines 80` against the combined production report. Use one definition of the source scope and threshold. ReefAdmin is one crate; no sharding or per-crate aggregation is needed.
- Add a dedicated coverage CI job with the pinned toolchain and PostgreSQL service. Keep existing Clippy, package and browser checks. Measure added runtime before considering replacing duplicate test executions.
- Retain HTML and machine-readable reports as CI artifacts, including below-threshold reports. Test failures must remain failures even when a report is produced. No external coverage service is required.
- Verify the gate with a temporary threshold above the measured result, then restore 80%. Verify that stale profiles, unavailable PostgreSQL and test failures cannot produce a successful check.
- Document `check:postgres`, `coverage:check` and `test:browser` as the complete local validation sequence. When a remote and branch protection are configured, make coverage a required check.

Done when a clean CI-equivalent run passes and a deliberately failing threshold exits unsuccessfully. Per-file results remain visible for review; 80% is an overall floor, not permission to leave critical modules untested.

### T4. Check assertion strength with mutation testing — implemented

- Add a pinned, optional `test:mutants` task using `cargo-mutants`, initially scoped to parsing, validation and URL logic.
- Review surviving mutations and add tests for real behavioural gaps. Record equivalent mutations, timeouts and unsupported cases separately from missed defects.
- Keep mutation testing advisory until its runtime and results are useful and repeatable. Do not introduce an arbitrary mutation-score gate or automatic schedule.

Done when the initial scoped run has been reviewed and useful findings have regression tests. Coverage measures executed code; mutation testing helps check whether the assertions detect changed behaviour.

### T5. Test authorization as its interfaces are implemented — implemented

Required actor context and deny-by-default policies now enforce operation and SQL row scopes. The host retains its account/session system. See [permissions](docs/permissions.md) for mixed-selection behaviour, transaction checks and callback boundaries.

- Require tests for missing actor context and denied admin entry, with access denied by default.
- Exercise per-entity, per-operation and per-record policies through direct requests, including forged IDs and records belonging to another user.
- Check list/detail pages, writes, toggles, bulk/custom actions, CSV exports, autocomplete and inlines. Test full-page and HTMX paths against the same rules.
- Assert denied reads do not disclose records and denied writes do not change the database or invoke mutation observers/domain operations. Define mixed-authorization bulk behaviour before implementing it.
- Test two users in successive/concurrent requests so actor context and CSRF tokens cannot leak between requests. Navigation visibility must agree with server enforcement.

Done when these tests ship with the policy implementation and pass alongside the coverage and browser gates. Reaching 80% earlier does not complete authorization work.

Order: T1 → T2 → T3, then the scoped T4 review. T5 ships with the permission contract. Keep changes to test tooling, new regression cases and permission APIs separately reviewable.

References: [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) for instrumentation, report merging and threshold flags; [cargo-mutants](https://mutants.rs/) for assertion-strength checks.

## Next milestone: Extension contracts

The actor/policy slice is implemented and tested. Domain-operation adapters, audit hooks and custom widgets remain. Design these against practical consumer workflows before committing to a stable API:

- Actor and request context for actions, policies and audit hooks without assuming a user model.
- Per-entity, per-operation and per-record authorization, including query scoping for lists, detail, autocomplete, inlines, exports and bulk actions.
- Server-enforced editable-field allowlists and protection for sensitive/read-only fields.
- Custom create/update/delete operations calling application domain functions, with transaction access. Structured validation errors for add and change are available through `ValidationErrorMapper`.
- Documented mutation observer semantics and an explicit boundary for external side effects.
- Audit hooks with actor, operation, record identity and appropriate before/after values; hosts choose storage and redaction.
- Custom widgets covering rendering, parsing, validation and required assets.
- An explicit mounting/protection contract, with tested session and CSRF examples and denied access when required context is missing.

Define bulk-operation atomicity and partial-failure reporting. Do not imply that arbitrary custom actions automatically participate in generic mutation transactions.

Completion: consumers can express their policies and domain operations without duplicating route parsing or forking the library. Tests cover denied direct requests, exports/autocomplete, transaction rollback and domain validation.

## Rich editing

1. File/image fields through a storage interface, with validation, size limits, previews and selection. Define orphan cleanup and failure handling because object storage is outside the database transaction.
2. Editable child records and ordered relationships for attachments and other related content.
3. Many-to-many controls with validated target selection and transactional updates.
4. Review actions with actor context, structured results and safe retries. Approval rules remain application-owned.
5. Optional geographic widget example using GeoJSON. Spatial storage and domain semantics stay outside the core.

Complex review pages may remain application-owned routes sharing admin navigation and styling. Completion means applications can implement these workflows without a library fork.

## Usability and documentation

Provide plain Axum and Loco examples demonstrating a protected mount, existing authentication, an entity, a relationship, validation and a custom action. Examples must not expose unauthenticated writes by default.

Retain bundled default assets, configurable templates and branding. Include keyboard-accessible forms, clear errors, mobile layouts and empty states.

Document model persistence hooks, mutation observers and custom operations separately. State composite-key, backend, relationship and upload limitations explicitly. Validate configuration early with actionable errors.

## Loco integration example

Implemented in `examples/loco/` against published Loco 1.2.0. Six HTTP integration tests boot the real application; a separate PostgreSQL test and three browser scenarios exercise the protected mount. The extracted crate also passes the consumer tests. See the example README for setup.

### Purpose and layout

The inventory application in `examples/loco/` has its own `Cargo.toml`, lockfile, migrations, configuration and tests. It stays outside the core crate dependency graph. Loco remains an example dependency, not a ReefAdmin runtime dependency or feature flag.

The example pins published Loco 1.2.0 and locks its dependencies, including Axum 0.8 and SeaORM 2. It uses the repository's pinned Rust toolchain.

Files:

- `src/app.rs`: Loco application hooks and initializer registration.
- `src/initializers/admin.rs`: build ReefAdmin from `ctx.db`, then mount the protected router at `/staff` through `Initializer::after_routes`.
- `src/admin/policy.rs`: translate application roles and ownership into ReefAdmin policies.
- `src/admin/middleware.rs`: adapt the application's resolved session to `AdminActor` and `CsrfToken`; enforce staff entry and CSRF checks.
- `src/models.rs` and `src/migration.rs`: application users, sessions, categories and products.
- `tests/`: boot the Loco application, exercise HTTP requests and check persisted changes.

The initializer runs after application routes have been constructed. Explicitly attach the required middleware to the admin router before nesting it; do not assume layers already attached to the parent will protect newly added routes. Verify the actual request order in tests.

### Application and protection contract

The inventory host owns its users and single login flow. It uses Loco password hashing, signed cookies and revocable database sessions. Loco's JWT feature is disabled: its published implementation pulls in crypto dependencies with known advisories. Resolve sessions through the host, load current role/ownership information and only then construct `AdminActor`. Never accept actor IDs or staff roles from request headers or form fields.

Have two staff users who own different products and one ordinary user. Products have a name, owner, category and active flag. Categories provide a foreign-key select and autocomplete. Hide authentication tables from the admin. Keep ownership fields read-only and prevent forged ownership changes; keep creation disabled until the application can assign and validate the owner securely.

Staff can view and edit their own products, toggle their active flag and export an explicit field allowlist. Unknown operations remain denied. A custom archive action calls an application function with the authenticated actor and authorized IDs; the function owns its transaction and rechecks access. This demonstrates the existing action API without implying that domain-operation adapters are implemented.

Run session resolution, staff authorization and CSRF validation before admin handlers. Use a host-issued unpredictable token bound to the session, include it through `CsrfToken` and validate it on every unsafe request. Reuse the host's CSRF facility where available; an example-specific adapter belongs in the host application. Fixed tokens and test identity injection must not exist in the runnable application.

Unauthenticated ordinary pages redirect to the application's login page. Enhanced requests use the host's explicit session-expiry response and must never insert login HTML into an admin fragment. Authenticated non-staff users receive HTTP 403. Missing or invalid CSRF tokens reject writes before any domain operation runs. No default administrator credentials or automatic login; local setup uses an explicit account-creation command and binds to loopback.

### Verification

| Area | Required evidence |
| --- | --- |
| Boot and mounting | Loco boots with migrations, runs the initializer, serves `/staff` and bundled assets, and retains ordinary application routes |
| Identity | Logged-out, expired-session and non-staff requests cannot read records or mutate data; successive users do not share actor state |
| Row scopes | Lists, detail, autocomplete, exports and direct-ID requests expose only permitted records; mixed unauthorized selections fail |
| Writes | Valid edits persist; missing/invalid/cross-session CSRF, forged ownership and unauthorized toggles leave data unchanged |
| Domain actions | Archive calls the host function, rejects forged IDs, rechecks ownership and rolls back on failure |
| Browser behaviour | Real host login, a protected edit, autocomplete, toggle, logout/session expiry and ordinary forms without JavaScript |
| Package compatibility | The example builds and tests against the extracted release-candidate crate, not only the repository source |

The boot/request tests use isolated SQLite databases. A disposable PostgreSQL test covers mounting and writes. Browser tests log in through the application's normal flow; test helpers seed users without bypassing authentication for security assertions.

### Delivery slices

1. Boot the pinned Loco application with migrations and a deny-by-default admin mount. Verify middleware order and protect every admin endpoint.
2. Connect host sessions, staff checks, CSRF and ownership policy. Add direct-request denial and rollback tests.
3. Add category lookup, export and the application archive action. Add browser checks and clear local setup instructions.
4. Add `mise run test:loco` and `mise run test:loco:browser` tasks and CI jobs. Run the application tests against the extracted Cargo package in a temporary directory. Keep example dependencies and assets out of the core archive; explicitly account for example Rust code when documenting coverage scope.

Completion means a fresh checkout can follow the documented setup, boot the protected example and run its checks without a sibling application repository. Record the exact Loco version and results. The tasks and files now exist. CI calls the same portable mise tasks; the core crate does not depend on Loco.

Design references: [Loco initializer and application hooks](https://github.com/loco-rs/loco/blob/master/src/app.rs), [Loco dependency manifest](https://github.com/loco-rs/loco/blob/master/Cargo.toml), and ReefAdmin's [permission contract](docs/permissions.md). Recheck the pinned release source during implementation.

## ReefAdmin release preparation

The crate and Rust import name are `reefadmin`. The test database environment variable and fragment response header use ReefAdmin names. Repository metadata points to `gilesw/reefadmin`.

The initial 0.1 release covers the implemented CRUD and policy contracts. Domain-operation adapters, audit hooks, rich editing and the optional partial-model simplification remain follow-up work. Hosts that require these features should retain application-owned routes and operations.

Verification includes core and Loco HTTP, PostgreSQL, browser and extracted-package tests. See docs/testing.md for measured coverage and docs/releasing.md for the current release procedure. Historical rust-code-analysis measurements above record earlier refactoring work; the tool has been removed from the active checks.

Before publishing, confirm crates.io name availability and credentials, commit the release candidate and run the portable release procedure in docs/releasing.md. The Loco consumer tests exercise a real host integration; arbitrary downstream applications still need their own checks. Publishing remains a separate explicit action.

## Public release gates

- Consumer checks pass with a reproducible dependency, rather than a sibling-directory assumption.
- Package and examples build independently of development workspaces.
- PostgreSQL-specific field types and transactional hooks have coverage.
- The combined production Rust line-coverage gate is at least 80%; browser checks and explicit authorization-denial tests pass independently.
- Permission/CSRF boundaries, output escaping, field allowlists, pagination limits and bulk actions are tested.
- Attribution, dependency licensing, crate-name availability, repository metadata, API docs and changelog are ready.
- Version compatibility and migration policies are documented.
- The documentation and interface-language review in slice 7 is complete.
- Portable release checks and an explicit manual publish/tag procedure are documented; no hosted release service is required.

Publish an initial 0.x release once the reusable baseline and protection contracts are dependable. Advanced widgets need not all block that release. Publishing requires a separate explicit release action.
