"""Post-deploy E2E smoke — PR #600 (V3 quote batching, searcher-rs).

Runs against the DEPLOYED webapp (public domain via Cloudflare tunnel -> VPS).
No local servers (RULE 01: no backend services on Windows).

Checks (webapp-testing reconnaissance-then-action):
  1. /opportunities loads and reaches networkidle.
  2. Console errors captured (fail-honest report, not a hard fail — WS hiccups
     are possible; we report them).
  3. DOM reconnaissance: does the opportunities surface render any real data
     (RULE 00: empty state must be shown honestly if there is no data).
  4. Screenshot as evidence artifact.

Usage:
  python post_deploy_smoke_600.py [--base https://arbx.ape-tv.net]
"""

import argparse
import json
import sys
from datetime import datetime, timezone

from playwright.sync_api import sync_playwright

BASE_DEFAULT = "https://arbx.ape-tv.net"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default=BASE_DEFAULT)
    args = ap.parse_args()

    report: dict = {
        "started_at": datetime.now(timezone.utc).isoformat(),
        "base": args.base,
        "checks": {},
    }
    console_errors: list[str] = []
    failed_requests: list[str] = []

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        page.on(
            "console",
            lambda m: console_errors.append(m.text) if m.type == "error" else None,
        )
        page.on(
            "requestfailed",
            lambda r: failed_requests.append(f"{r.method} {r.url} :: {r.failure}"),
        )

        url = f"{args.base}/opportunities"
        try:
            resp = page.goto(url, wait_until="domcontentloaded", timeout=45_000)
            report["checks"]["http_status"] = resp.status if resp else None
            page.wait_for_load_state("networkidle", timeout=30_000)
            report["checks"]["networkidle"] = True
        except Exception as e:  # noqa: BLE001 — fail-honest report
            report["checks"]["networkidle"] = False
            report["checks"]["navigation_error"] = str(e)

        report["checks"]["title"] = page.title()

        # DOM reconnaissance: any numeric cells / cards rendered?
        body_text = page.locator("body").inner_text(timeout=10_000)
        report["checks"]["body_length"] = len(body_text)
        # Fail-honest probes: look for surfaces, count what's actually there.
        for sel, name in [
            ("table tbody tr", "table_rows"),
            ("[data-testid*=card], [class*=card]", "cards"),
        ]:
            try:
                report["checks"][name] = page.locator(sel).count()
            except Exception:  # noqa: BLE001
                report["checks"][name] = "selector_error"

        shot = "post_deploy_smoke_600.png"
        page.screenshot(path=shot, full_page=True)
        report["screenshot"] = shot
        browser.close()

    report["console_errors"] = console_errors[:50]
    report["failed_requests"] = failed_requests[:50]
    report["finished_at"] = datetime.now(timezone.utc).isoformat()

    out = "post_deploy_smoke_600.json"
    with open(out, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, ensure_ascii=False)
    print(json.dumps(report["checks"], indent=2))
    print(f"console_errors={len(console_errors)} failed_requests={len(failed_requests)}")
    print(f"report={out} screenshot={shot}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
