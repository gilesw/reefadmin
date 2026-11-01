#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${REEFADMIN_TEST_DATABASE_URL:?Set REEFADMIN_TEST_DATABASE_URL to a disposable PostgreSQL database}"
mkdir -p target
lock=target/coverage-loco.lock
mkdir "$lock" || { echo 'A Loco coverage run is active (or its lock needs removing after interruption).' >&2; exit 1; }
trap 'rmdir "$lock"' EXIT
report="$PWD/target/coverage-loco"
rm -rf "$report"
mkdir -p "$report"
manifest=examples/loco/Cargo.toml
# All example Rust, including its executable, remains in the denominator.
exclude='(^|/)(tests|target)/'
cargo llvm-cov clean --manifest-path "$manifest" --workspace
cargo llvm-cov --manifest-path "$manifest" --locked --lib --tests --no-report
cargo llvm-cov --manifest-path "$manifest" --locked --test postgres --no-report -- --ignored
cargo llvm-cov report --manifest-path "$manifest" --ignore-filename-regex "$exclude" --json --output-path "$report/coverage.json"
cargo llvm-cov report --manifest-path "$manifest" --ignore-filename-regex "$exclude" --html --output-dir "$report"
cargo llvm-cov report --manifest-path "$manifest" --ignore-filename-regex "$exclude" --fail-under-lines 80 | tee "$report/summary.txt"
touch "$report/complete"
