# Local security checks

All checks run through mise without a scanning account or source upload. CI can invoke the same commands on any provider. Tool downloads and RustSec advisory updates require network access; source analysis uses local rules and DAST targets only the disposable loopback application.

| Command | Purpose | Failure policy |
| --- | --- | --- |
| `mise run security:audit` | cargo-audit 0.22.2 checks the core and Loco example lockfiles against RustSec | Known vulnerabilities fail; maintenance warnings remain visible |
| `mise run security:sast:test` | OpenGrep 1.33.1 checks positive and negative rule fixtures | Failed rule tests fail |
| `mise run security:sast` | OpenGrep scans core Rust, example Rust and admin JavaScript with vendored rules | Scan errors and error-level findings fail; audit warnings require review |
| `mise run security:dast` | ZAP 2.17.0 passively scans authenticated browser traffic | Browser/scanner failures, missing authenticated coverage or high-risk alerts fail |

Dependency auditing runs in CI and in `release:check`. SAST, its rule tests and passive DAST are optional investigations; they are not merge or release gates. Their exit status still identifies findings or execution failures when invoked. Keep Clippy and the explicit authorization tests as regular checks.

## Static analysis scope

OpenGrep is an LGPL-2.1 open-source fork of Semgrep. Semgrep Community Edition is also a local open-source option. The engine and the rule set are separate choices; using an open-source engine does not make every downloadable rule set open source.

The current rule set is deliberately small and is **not a comprehensive Rust SAST audit**:

- Trail of Bits' `panic-in-function-returning-result`, copied unmodified with its tests from commit `31390b3a99c04c81522d1b37c8d1900aa2dd4094` of [trailofbits/semgrep-rules](https://github.com/trailofbits/semgrep-rules). Its AGPLv3 licence is retained in `security/rules/trailofbits/LICENSE`.
- Four ReefAdmin rules check public HTTP 500 diagnostics, disabled TLS verification, directly interpolated unprepared SQL and JavaScript string evaluation. These are repository-specific patterns, not whole-program data-flow analysis.

Third-party rule files are development tools and are outside the Cargo package allowlist. The runtime crate retains its MIT licence. Rules and fixtures are stored under `security/rules/`; changes must pass `security:sast:test`. Update third-party rules by reviewing a pinned upstream revision and retaining its licence and tests. Do not use a moving remote rules URL in the scan task.

The scan covers `src/`, `examples/loco/src/` and `static/admin.js`. Test fixtures and build outputs are excluded from SAST, not from the production coverage denominator. Results go to `target/security/sast.json`. An empty report only means that these rules found no matches. It does not establish that authorization, SQL construction or escaping is correct.

## Dependency findings

The initial example audit found advisories in `rsa` and `crypto-bigint` pulled in by Loco 1.2.0's optional JWT feature. The example now uses signed cookies and database sessions, so that dependency chain is absent. No vulnerability suppression was added. These were dependency advisories, not a demonstrated exploit of Loco's HMAC JWT flow. Upstream [PR #1867](https://github.com/loco-rs/loco/pull/1867) removes that dependency path; it was merged on 19 October 2026 but is not in the pinned 1.2.0 release. Reassess when a new release is available.

`unsafe-libyaml` still produces maintenance warning **RUSTSEC-2024-0449**, through Loco's YAML parsing and generator dependencies. It is not suppressed. Track upstream changes; do not claim that the complete dependency tree is actively maintained. Dependency freshness and absence of known advisories are different checks.

## Dynamic analysis scope

The DAST task downloads the pinned ZAP cross-platform archive and checks SHA-256 `94c8f767b1c2e94f0db66b3ae56514d5e3f5a728ee1b6c798e0c8fe2d61fbff0`. It uses pinned Java and Node tools, starts a local ZAP proxy and runs the Loco browser suite through it. Install Chromium first with `mise run browser:setup`.

The application uses a new in-memory database with test-only users. The browser signs in through the host's real login form. The scanner must record a successful authenticated `/staff/products` response; scanning only the login page cannot pass. Ports 39128 and 39129 must be free. Neither an external target URL nor production credentials are accepted.

The task disables unsolicited ZAP update checks, waits for passive scanning to finish, writes reports and stops the proxy. Reports are under `target/security/dast/`: `alerts.json`, `report.html` and `zap.log`. The `complete` marker appears only after success and is removed before each run. Reports contain disposable fixture traffic and must still be treated as diagnostic data.

This task is passive DAST. It observes the browser scenarios; it does not fuzz every endpoint, run active attacks or prove business authorization. Medium and low findings remain visible for review. An active scan needs a separate configuration for the disposable application and is not included in this task.

Coverage, Clippy, mutation testing, security scanning and explicit permission tests provide different evidence. Keep the actor, row-scope, CSRF and rollback regression tests even when all scanners pass.
