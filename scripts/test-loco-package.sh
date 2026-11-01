#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo package --locked --allow-dirty
fixture=$(mktemp -d "$PWD/target/loco-package.XXXXXX")
trap 'rm -rf "$fixture"' EXIT
tar -C examples/loco --exclude target --exclude '*.sqlite*' -cf - . | tar -C "$fixture" -xf -
python3 - "$fixture/Cargo.toml" "$PWD/target/package/reefadmin-0.1.0" <<'PY'
import json
from pathlib import Path
import sys
manifest = Path(sys.argv[1])
manifest.write_text(manifest.read_text().replace('path = "../.."', 'path = ' + json.dumps(sys.argv[2])))
PY
CARGO_TARGET_DIR="$PWD/examples/loco/target" cargo test --locked --manifest-path "$fixture/Cargo.toml"
