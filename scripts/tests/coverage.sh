#!/usr/bin/env bash
# Test runner failure handling without recompiling the crate for each fault.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/scripts" "$fixture/bin"
cp "$repo/scripts/coverage.sh" "$fixture/scripts/coverage.sh"
cat > "$fixture/bin/cargo" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> calls
case "$*" in
  *'--lib --tests'*) [ "${FAULT:-}" != unit ] ;;
  *'--test postgres'*) [ "${FAULT:-}" != postgres ] ;;
  *'--json'*) printf '{}\n' > target/coverage/coverage.json ;;
  *'--html'*) mkdir -p target/coverage/html; touch target/coverage/html/index.html ;;
  *'--fail-under-lines'*) [ "${FAULT:-}" != threshold ] ;;
esac
MOCK
chmod +x "$fixture/bin/cargo"
export PATH="$fixture/bin:$PATH"
cd "$fixture"
export REEFADMIN_TEST_DATABASE_URL=postgres://test/fixture
bash scripts/coverage.sh check
[ -f target/coverage/complete ]
grep -q -- '--fail-under-lines 80' calls
grep -q -- '--test postgres --no-report -- --ignored' calls
# The previous environment variable remains a compatibility fallback.
env -u REEFADMIN_TEST_DATABASE_URL LOCO_ADMIN_TEST_DATABASE_URL=postgres://test/legacy bash scripts/coverage.sh check
[ -f target/coverage/complete ]
# Missing configuration must fail and discard old reports.
if env -u REEFADMIN_TEST_DATABASE_URL -u LOCO_ADMIN_TEST_DATABASE_URL bash scripts/coverage.sh check 2>/dev/null; then exit 1; fi
[ ! -f target/coverage/complete ]
for fault in unit postgres threshold; do
  : > calls
  if FAULT="$fault" bash scripts/coverage.sh check; then echo "Unexpected success: $fault" >&2; exit 1; fi
  [ ! -f target/coverage/complete ]
  if [ "$fault" = threshold ]; then
    [ -f target/coverage/coverage.json ]
    [ -f target/coverage/html/index.html ]
  else
    if grep -q '^llvm-cov report' calls; then exit 1; fi
  fi
  if bash scripts/coverage.sh html 2>/dev/null; then exit 1; fi
done
mkdir target/coverage.lock
if bash scripts/coverage.sh run 2>/dev/null; then exit 1; fi
[ -d target/coverage.lock ]
rmdir target/coverage.lock
: > calls
bash scripts/coverage.sh run
if grep -q -- '--fail-under-lines' calls; then exit 1; fi
: > calls
bash scripts/coverage.sh html
if grep -q -- '--test' calls; then exit 1; fi
printf 'Coverage runner failure checks passed.\n'
