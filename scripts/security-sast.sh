#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p target/security
# A local rule directory prevents registry downloads or source uploads.
opengrep scan --disable-version-check --config security/rules \
  --exclude src/tests --exclude '**/target/**' --exclude '**/tests/**' \
  --json --output target/security/sast.json src examples/loco/src static/admin.js
# Error-level findings fail; upstream audit warnings remain visible for review.
python3 - <<'PY'
import json
from pathlib import Path
report = json.loads(Path('target/security/sast.json').read_text())
for finding in report.get('results', []):
    print(f"{finding['path']}:{finding['start']['line']}: {finding['check_id']}: {finding['extra']['message']}")
if report.get('errors'):
    raise SystemExit('SAST reported scan errors; inspect target/security/sast.json')
print(f"SAST: {len(report.get('results', []))} findings; report: target/security/sast.json")
if any(item["extra"]["severity"] == "ERROR" for item in report.get("results", [])):
    raise SystemExit("SAST found error-level findings")
PY
