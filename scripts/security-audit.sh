#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
status=0
cargo audit --file Cargo.lock || status=$?
cargo audit --file examples/loco/Cargo.lock || status=$?
exit "$status"
