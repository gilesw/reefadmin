# Loco inventory example

This application demonstrates ReefAdmin in published Loco **1.2.0**, with Axum 0.8 and SeaORM 2. The host owns users, login, signed session cookies, session revocation, CSRF validation and ownership policies. ReefAdmin supplies the administration router. Loco is not a dependency of the core library.

## Run locally

From the repository root, install the pinned tools with `mise run setup`. Generate a session secret and set a local database URL:

```sh
export INVENTORY_SESSION_SECRET="$(openssl rand -hex 32)"
export INVENTORY_DATABASE_URL="sqlite://inventory.sqlite?mode=rwc"
```

Keep the same secret when running setup and starting the server. Changing it invalidates existing cookies. The application creates its tables through Loco migrations. It does not truncate or recreate existing databases.

Create an account explicitly. Enter a password of at least 12 characters; the shell example below uses Bash and hides input:

```bash
read -rs -p 'Initial password: ' INVENTORY_INITIAL_PASSWORD
printf '\n'
export INVENTORY_INITIAL_PASSWORD
mise exec -- cargo run --locked --manifest-path examples/loco/Cargo.toml -- \
  create-user alice@example.test staff
unset INVENTORY_INITIAL_PASSWORD
```

The command creates the account, a category and one sample product. Use `member` instead of `staff` to create an account without admin access. Repeat with another email to demonstrate isolated ownership. No account or password is created automatically.

Start the server:

```sh
mise exec -- cargo run --locked --manifest-path examples/loco/Cargo.toml -- serve
```

Open <http://127.0.0.1:39128/login>. The server binds to loopback. Sign in, then follow **Administration**. Staff can edit their own products, select their own categories, toggle active status, export explicitly allowed fields and archive selected products. Creation and deletion through the admin are disabled. User and session tables are never registered with ReefAdmin.

This is a local integration example, not a complete production identity service. A production host must use its established authentication, rate limiting, account recovery and HTTPS deployment. Cookie `Secure` follows the host scheme; the supplied configuration uses local HTTP. Login checks both Origin and a signed CSRF cookie; authenticated writes check a session-bound token. Expired and revoked sessions cannot reach admin handlers. The host response policy sets CSP, anti-framing and MIME-sniffing headers. Inline styles and inline scripts are forbidden; bundled assets are served from the same origin.

## Verify

Run from the repository root:

```sh
mise run test:loco
mise run browser:setup
mise run test:loco:browser
mise run test:loco:package
```

For PostgreSQL, set `REEFADMIN_TEST_DATABASE_URL` to a disposable database and run `mise run test:loco:postgres` and `mise run coverage:loco`. The latter enforces 80% line coverage across the example library and executable. The test creates and removes an isolated schema. It never uses `INVENTORY_DATABASE_URL`.

The HTTP tests boot Loco, run migrations and the initializer, and log in through `/login`. They verify permissions, foreign-key scopes, exports, CSRF, expired/revoked sessions, role changes and archive rollback. Browser tests cover login, edits, autocomplete, toggles, logout/session expiry and ordinary forms without JavaScript. Their users exist only in the ignored test fixture; the executable has no test login route.

`test:loco:package` copies this consumer to a temporary directory and runs it against the extracted Cargo archive. This catches missing packaged files and avoids relying on a sibling application repository.

## Application structure

- `src/app.rs`: Loco boot, routes and initializer registration.
- `src/initializers/admin.rs`: entity registration and protected `/staff` mount.
- `src/auth.rs`: the host's login and revocable signed sessions.
- `src/admin/`: staff/CSRF middleware and row policies.
- `src/domain.rs`: the transactional archive operation, which rechecks current access.
- `src/models.rs` and `src/migration.rs`: application data and migrations.

The initializer protects its router directly before nesting it. It does not rely on middleware installed on the parent before its routes existed. Custom actions own their transactions; generic model writes retain ReefAdmin's transaction handling.

Loco's JWT feature is deliberately disabled. The example uses Loco's password-hashing helpers and Axum's signed cookies; this removes the unused RSA dependency chain flagged by the dependency audit. The remaining `unsafe-libyaml` maintenance warning is documented in `docs/security.md` at the repository root.
