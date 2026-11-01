#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mode=${1:-run}
case "$mode" in run|check|html) ;; *) echo 'Usage: coverage.sh [run|check|html]' >&2; exit 2 ;; esac
report_dir=target/coverage
# Only test-source paths are excluded; all production modules remain in scope.
exclude='(^|/)(tests|target)/'
minimum=80
mkdir -p target
if ! mkdir target/coverage.lock 2>/dev/null; then
  echo 'A coverage run is active (or target/coverage.lock needs removing after an interrupted run).' >&2
  exit 1
fi
trap 'rmdir target/coverage.lock' EXIT
if [ "$mode" = html ]; then
  test -f "$report_dir/complete" || { echo 'Run coverage successfully before rendering HTML.' >&2; exit 1; }
  cargo llvm-cov report --ignore-filename-regex "$exclude" --html --output-dir "$report_dir"
  exit
fi
rm -rf "$report_dir"
mkdir -p "$report_dir"
: "${REEFADMIN_TEST_DATABASE_URL:?Set REEFADMIN_TEST_DATABASE_URL to a disposable PostgreSQL database}"
cargo llvm-cov clean --workspace
cargo llvm-cov --locked --lib --tests --no-report
cargo llvm-cov --locked --test postgres --no-report -- --ignored
cargo llvm-cov report --ignore-filename-regex "$exclude" --json --output-path "$report_dir/coverage.json"
cargo llvm-cov report --ignore-filename-regex "$exclude" --html --output-dir "$report_dir"
# Produce reports before enforcing the floor, so failures remain inspectable.
if [ "$mode" = check ]; then
  cargo llvm-cov report --ignore-filename-regex "$exclude" --fail-under-lines "$minimum" | tee "$report_dir/summary.txt"
else
  cargo llvm-cov report --ignore-filename-regex "$exclude" | tee "$report_dir/summary.txt"
fi
touch "$report_dir/complete"
