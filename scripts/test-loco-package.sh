#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo package --locked --allow-dirty
package_dir=$(cargo metadata --no-deps --format-version 1 --locked | python3 -c '
import json, pathlib, sys
metadata = json.load(sys.stdin)
package = next(p for p in metadata["packages"] if p["name"] == "reefadmin")
print(pathlib.Path(metadata["target_directory"]) / "package" / (package["name"] + "-" + package["version"]))
')
fixture=$(mktemp -d "$PWD/target/loco-package.XXXXXX")
trap 'rm -rf "$fixture"' EXIT
tar -C examples/loco --exclude target --exclude '*.sqlite*' -cf - . | tar -C "$fixture" -xf -
python3 - "$fixture/Cargo.toml" "$package_dir" <<'PY'
import json
from pathlib import Path
import sys
manifest = Path(sys.argv[1])
manifest.write_text(manifest.read_text().replace('path = "../.."', 'path = ' + json.dumps(sys.argv[2])))
PY
CARGO_TARGET_DIR="$PWD/examples/loco/target" cargo test --locked --manifest-path "$fixture/Cargo.toml"
