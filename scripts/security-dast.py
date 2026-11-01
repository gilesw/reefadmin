#!/usr/bin/env python3
"""Run authenticated browser flows through a pinned, loopback-only ZAP proxy."""
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import tempfile
import time
import urllib.parse
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
VERSION = "2.17.0"
SHA256 = "94c8f767b1c2e94f0db66b3ae56514d5e3f5a728ee1b6c798e0c8fe2d61fbff0"
BASE = "http://127.0.0.1:39128"
PROXY = "http://127.0.0.1:39129"
REPORTS = ROOT / "target/security/dast"
TOOLS = ROOT / "target/security/tools"


def main():
    REPORTS.mkdir(parents=True, exist_ok=True)
    TOOLS.mkdir(parents=True, exist_ok=True)
    # Never leave a previous successful report looking current after a failed run.
    for name in ["alerts.json", "report.html", "complete"]:
        (REPORTS / name).unlink(missing_ok=True)
    archive = TOOLS / f"ZAP_{VERSION}_Crossplatform.zip"
    if not archive.exists():
        subprocess.run(["curl", "-fL", "--retry", "2", f"https://github.com/zaproxy/zaproxy/releases/download/v{VERSION}/{archive.name}", "-o", str(archive)], check=True)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
        raise SystemExit("ZAP archive checksum mismatch; remove the archive and retry.")
    install = TOOLS / "zap"
    with zipfile.ZipFile(archive) as bundle:
        bundle.extractall(install)
    key = secrets.token_hex(32)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def api(component, kind, operation, **params):
        query = urllib.parse.urlencode(dict(params, apikey=key))
        with opener.open(f"{PROXY}/JSON/{component}/{kind}/{operation}/?{query}", timeout=10) as reply:
            value = json.load(reply)
        if "code" in value:
            raise RuntimeError(f"ZAP API error: {value}")
        return value

    # Refuse to use an unrelated scanner or application already on these ports.
    import socket
    for port in [39128, 39129]:
        with socket.socket() as check:
            if check.connect_ex(("127.0.0.1", port)) == 0:
                raise SystemExit(f"Port {port} is already in use. Stop that process before scanning.")
    with tempfile.TemporaryDirectory(prefix="reefadmin-zap-") as state, (REPORTS / "zap.log").open("w") as log:
        process = subprocess.Popen([
            "java", "-Djava.awt.headless=true", "-Xmx512m", "-jar", str(install / f"ZAP_{VERSION}" / f"zap-{VERSION}.jar"),
            "-daemon", "-silent", "-host", "127.0.0.1", "-port", "39129", "-dir", state,
            "-config", f"api.key={key}", "-config", "autoupdate.checkOnStart=false",
            "-config", "autoupdate.installAddonUpdates=false", "-config", "autoupdate.downloadNewRelease=false",
        ], stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 120
            while True:
                try:
                    api("core", "view", "version")
                    break
                except (OSError, ValueError):
                    if process.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError("ZAP failed to start; inspect target/security/dast/zap.log")
                    time.sleep(1)
            api("context", "action", "newContext", contextName="inventory")
            api("context", "action", "includeInContext", contextName="inventory", regex=r"http://127\.0\.0\.1:39128/.*")
            api("context", "action", "setContextInScope", contextName="inventory", booleanInScope="true")
            api("pscan", "action", "setScanOnlyInScope", onlyInScope="true")
            env = dict(os.environ, REEFADMIN_ZAP_PROXY=PROXY)
            browser = subprocess.run(["npx", "playwright", "test", "--config", "playwright.loco.config.cjs"], cwd=ROOT, env=env)
            deadline = time.monotonic() + 120
            while int(api("pscan", "view", "recordsToScan")["recordsToScan"]):
                if time.monotonic() > deadline:
                    raise RuntimeError("ZAP passive scanning timed out")
                time.sleep(1)
            messages = api("core", "view", "messages", baseurl=BASE + "/staff/products", count=1000)["messages"]
            authenticated = any(
                "/staff/products" in message.get("requestHeader", "").split("\r\n")[0]
                and "200" in message.get("responseHeader", "").split("\r\n")[0]
                for message in messages
            )
            alerts = api("core", "view", "alerts", baseurl=BASE, count=10000)["alerts"]
            (REPORTS / "alerts.json").write_text(json.dumps(alerts, indent=2))
            with opener.open(f"{PROXY}/OTHER/core/other/htmlreport/?apikey={key}", timeout=30) as reply:
                (REPORTS / "report.html").write_bytes(reply.read())
            high = [alert for alert in alerts if alert["risk"] == "High"]
            for alert in alerts:
                print(f"{alert['risk']}: {alert['alert']} ({alert['url']})")
            print(f"ZAP: {len(alerts)} alerts, {len(high)} high risk; authenticated admin traffic: {authenticated}")
            if browser.returncode or not authenticated or high:
                raise SystemExit("DAST failed: browser failure, missing authenticated coverage or high-risk alerts. Inspect target/security/dast.")
            (REPORTS / "complete").touch()
        finally:
            process.terminate()
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == "__main__":
    main()
