#!/usr/bin/env python3
"""G-SIM-1 registry slice interpreter (G-SIM1-AUTOREFRESH, 2026-09-26).

Single job: read the RAW registry rows for one gate and say — honestly and
derived-only — (a) whether a scheduled producer actually delivered in this run,
and (b) what is unmet, why, and the exact operator action that clears it.

Two modes, one interpretation contract:

    # delivery assertion used by the variance-benchmark workflow
    python3 scripts/ci/gsim1_evidence_state.py assert-fresh \
        --rows rows.jsonl --item variance_benchmark --max-age-minutes 90

    # operator-attention report + machine-readable flag
    python3 scripts/ci/gsim1_evidence_state.py attention \
        --rows rows.jsonl --now 2026-09-27T00:00:00Z --warn-days 7

`rows.jsonl` is one `row_to_json(readiness_evidence)` object per line, produced
by the workflow with a READ-ONLY psql query. Nothing here writes, and nothing
here invents a value: every token of the output comes from the row, the clock,
or the checklist constant below (which mirrors G_SIM_1_ITEM_KEYS in
backend/api-server/src/routes/readiness-evidence.ts — a test pins the mirror).

Exit codes
    0  interpretation produced (including "unmet" — failing the pipeline is not
       how a red gate is reported; the registry row and the issue are)
    1  the input could not be interpreted, or `assert-fresh` found that the
       producer did NOT deliver (the silent failure this script exists to catch)
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timedelta, timezone

# Mirror of G_SIM_1_ITEM_KEYS (backend/api-server/src/routes/readiness-evidence.ts).
ITEM_KEYS = (
    "unit_tests",
    "modules_merged",
    "fork_suite",
    "variance_benchmark",
    "dep_tree",
    "eth_callbundle_staging",
    "second_signoff",
)

# Mirror of FRESHNESS_DAYS (same file). A row older than this is NOT evidence.
FRESHNESS_DAYS = 30


def parse_ts(value: str) -> datetime:
    ts = value.strip()
    if ts.endswith("Z"):
        ts = ts[:-1] + "+00:00"
    dt = datetime.fromisoformat(ts)
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc)


def load_rows(path: str) -> list[dict]:
    rows: list[dict] = []
    with open(path, encoding="utf-8") as fh:
        for lineno, line in enumerate(fh, 1):
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"::error::rows file line {lineno} is not JSON: {exc}")
            if not isinstance(obj, dict):
                raise SystemExit(f"::error::rows file line {lineno} is not a JSON object")
            rows.append(obj)
    return rows


def classify(rows: list[dict], now: datetime) -> list[dict]:
    """Latest row per item_key → evidenced / stale / failed / missing."""
    cutoff = now - timedelta(days=FRESHNESS_DAYS)
    out: list[dict] = []
    for key in ITEM_KEYS:
        row = next((r for r in rows if r.get("item_key") == key), None)
        if row is None:
            out.append({"item_key": key, "state": "missing", "reason": None,
                        "verified_at": None, "age_days": None, "detail": None,
                        "evidence_ref": None})
            continue
        verified = parse_ts(str(row.get("verified_at")))
        age_days = (now - verified).total_seconds() / 86400.0
        fresh = verified > cutoff
        status = str(row.get("status", "")).strip()
        if status == "evidenced" and fresh:
            state = "evidenced"
        elif status == "failed":
            state = "failed"
        else:
            state = "stale"
        detail = row.get("detail") if isinstance(row.get("detail"), dict) else None
        out.append({
            "item_key": key,
            "state": state,
            "status": status,
            # `reason` explains a NON-evidenced row only; an evidenced row needs
            # no excuse and its evidence_ref is already in the table's provenance.
            "reason": None if state == "evidenced" else failure_reason(detail, row.get("evidence_ref")),
            "verified_at": verified.isoformat().replace("+00:00", "Z"),
            "age_days": round(age_days, 2),
            "expires_in_days": round(FRESHNESS_DAYS - age_days, 2),
            "detail": detail,
            "evidence_ref": row.get("evidence_ref"),
        })
    return out


def failure_reason(detail: dict | None, evidence_ref: object) -> str | None:
    """Derived reason for a non-evidenced row — never invented."""
    if detail:
        for key in ("error", "fail_reason", "note"):
            val = detail.get(key)
            if isinstance(val, str) and val.strip():
                return val.strip()
        labeled = detail.get("samples_labeled")
        required = detail.get("min_samples_required")
        if isinstance(labeled, int) and isinstance(required, int):
            parts = [f"samples_labeled={labeled} < min_samples={required}"]
            skips = detail.get("skips") if isinstance(detail.get("skips"), dict) else {}
            hist = [
                f"{k}={skips[k]}"
                for k in ("unsupported_adapter", "stale_timestamp", "pred_failed", "obs_failed")
                if isinstance(skips.get(k), int) and skips[k] > 0
            ]
            if hist:
                parts.append(", ".join(hist))
            population = detail.get("population_size")
            if isinstance(population, int):
                parts.append(f"population={population}")
            return "; ".join(parts)
    if isinstance(evidence_ref, str) and evidence_ref.strip():
        return evidence_ref.strip()
    return None


def operator_action(item: dict) -> str:
    """The concrete, copy-pasteable action that clears this item."""
    key = item["item_key"]
    state = item["state"]
    if key == "second_signoff":
        if state == "evidenced":
            return "no action (fresh)"
        verb = "EXPIRED" if state == "stale" else "NEVER RECORDED"
        return (
            f"**{verb} — operator/reviewer action required.** Re-run the numerical-correctness "
            "sign-off and record a fresh row (30-day freshness, strict):\n"
            "```bash\n"
            "# ON THE VPS (reads ARBX_ADMIN_TOKEN from the deployment .env)\n"
            "set -a; . /opt/arbitragex-v2/.env; set +a\n"
            'curl --fail-with-body -sS -X POST http://127.0.0.1:8080/admin/readiness-evidence \\\n'
            '  -H "Content-Type: application/json" -H "x-arbx-admin-token: ${ARBX_ADMIN_TOKEN}" \\\n'
            "  --data '{\"gate_id\":\"G-SIM-1\",\"item_key\":\"second_signoff\",\"status\":\"evidenced\","
            "\"evidence_ref\":\"<PR review / signoff record>\","
            "\"detail\":{\"scope\":\"profit-calculation numerical correctness\",\"signoff_ref\":\"<ref>\"},"
            "\"verified_by\":\"reviewer:<id>\"}'\n"
            "```\n"
            "Then confirm: `GET /api/v1/readiness/blockers` must stop listing `second_signoff`."
        )
    if key == "variance_benchmark":
        if state == "evidenced":
            return "no action (fresh)"
        reason = item.get("reason") or ""
        base = (
            "The scheduled producer `.github/workflows/gsim1-variance-benchmark.yml` runs the "
            "harness on the VPS and ALWAYS records a row — open the newest row's "
            "`evidence_ref` for the full run."
        )
        if "no-labelable-population" in reason:
            return base + (
                " Current cause: the freshness window held 0 A.3.a-encodable 2-leg topologies. "
                "Widening the window does not help (the harness can only pin `tip − 1100` blocks); "
                "the fix is encoder coverage (V3 legs) or accepting the honest empty population."
            )
        if "sample-floor" in reason or "samples_labeled" in reason:
            return base + (
                " Current cause: **sample-floor** — `samples_labeled` is below the floor. The floor "
                "is now the measured labelable population of the window, not the old absolute 100 "
                "(measured unreachable: the labelable population saturates at 21 distinct topologies "
                "over 7 days). See `docs/operations/SIMULATOR_V2_READINESS.md` §variance_benchmark "
                "for the measurement and the two honest ways forward: widen the encoder to V3 legs, "
                "or accumulate labeled pairs across scheduled runs."
            )
        if "drift-threshold" in reason:
            return base + (
                " Current cause: **drift-threshold** — enough pairs were labeled but the mean absolute "
                "B-vs-B+1 drift exceeds 5%. This is a real simulator-accuracy finding: do NOT relax "
                "`VARIANCE_MAX_MEAN_DRIFT_PCT`; investigate the REVM path (quote closure, fee tier, "
                "gas price used for the call)."
            )
        return base + " Inspect the row's `detail` for the recorded cause."
    if state == "evidenced":
        return "no action (fresh)"
    return (
        f"Producer: see the row's `evidence_ref`. {key} is `{state}`"
        + (f" ({item.get('reason')})" if item.get("reason") else "")
        + "."
    )


def render_markdown(items: list[dict], now: datetime, warn_days: int) -> str:
    unmet = [i for i in items if i["state"] != "evidenced"]
    expiring = [
        i for i in items
        if i["state"] == "evidenced"
        and i.get("expires_in_days") is not None
        and i["expires_in_days"] <= warn_days
    ]
    lines = [
        "# G-SIM-1 readiness — registry state",
        "",
        f"- generated_at: `{now.isoformat().replace('+00:00', 'Z')}`",
        f"- evidenced: **{len(items) - len(unmet)}/{len(items)}**",
        f"- freshness rule: `verified_at > now - {FRESHNESS_DAYS}d` (strict)",
        "",
        "| item | state | status | age (d) | expires in (d) | reason |",
        "|---|---|---|---|---|---|",
    ]
    for it in items:
        lines.append(
            "| `{k}` | {s} | {st} | {age} | {exp} | {r} |".format(
                k=it["item_key"],
                s=it["state"],
                st=it.get("status") or "—",
                age="—" if it["age_days"] is None else it["age_days"],
                exp=it.get("expires_in_days", "—") if it["age_days"] is not None else "—",
                r=(it["reason"] or "—").replace("|", "\\|")[:180],
            )
        )
    if unmet:
        lines += ["", "## Required operator action", ""]
        for it in unmet:
            lines += [f"### `{it['item_key']}` — {it['state']}", "", operator_action(it), ""]
    if expiring:
        lines += ["", "## Expiring soon (record fresh evidence before the strict 30-day cut)", ""]
        for it in expiring:
            lines.append(
                f"- `{it['item_key']}` expires in **{it['expires_in_days']} d** "
                f"(verified_at `{it['verified_at']}`)"
            )
    if not unmet and not expiring:
        lines += ["", "No unmet item, nothing expiring within the warning window."]
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="mode", required=True)

    a = sub.add_parser("assert-fresh", help="delivery assertion for the scheduled producer")
    a.add_argument("--rows", required=True)
    a.add_argument("--item", required=True)
    a.add_argument("--max-age-minutes", type=float, required=True)

    t = sub.add_parser("attention", help="operator-attention report")
    t.add_argument("--rows", required=True)
    t.add_argument("--now", required=True, help="ISO-8601 UTC instant")
    t.add_argument("--warn-days", type=int, default=7)
    t.add_argument("--out", default="", help="write the markdown here (default: stdout)")
    t.add_argument("--github-output", default="", help="append needs_attention/unmet to this GITHUB_OUTPUT file")

    args = ap.parse_args(argv)
    rows = load_rows(args.rows)

    if args.mode == "assert-fresh":
        row = next((r for r in rows if r.get("item_key") == args.item), None)
        if row is None:
            print(f"::error::G-SIM-1 item {args.item} has NO row — the producer did not deliver")
            return 1
        now = datetime.now(timezone.utc)
        verified = parse_ts(str(row.get("verified_at")))
        age_min = (now - verified).total_seconds() / 60.0
        status = row.get("status")
        print(
            f"G-SIM-1 {args.item}: status={status} verified_at={verified.isoformat()} "
            f"age={age_min:.1f}min (limit {args.max_age_minutes}min)"
        )
        if age_min > args.max_age_minutes:
            print(
                f"::error::the scheduled producer did not advance G-SIM-1 {args.item} "
                f"(row is {age_min:.1f} min old > {args.max_age_minutes} min). "
                "The benchmark run was NOT recorded — treat as a broken pipeline, not as a red gate."
            )
            return 1
        print(f"DELIVERY OK: row advanced at {verified.isoformat()} with status={status}")
        return 0

    now = parse_ts(args.now)
    items = classify(rows, now)
    md = render_markdown(items, now, args.warn_days)
    unmet = [i["item_key"] for i in items if i["state"] != "evidenced"]
    needs = bool(unmet)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as fh:
            fh.write(md)
    else:
        sys.stdout.write(md)
    if args.github_output:
        with open(args.github_output, "a", encoding="utf-8") as fh:
            fh.write(f"needs_attention={'true' if needs else 'false'}\n")
            fh.write(f"unmet_count={len(unmet)}\n")
            fh.write(f"unmet_items={','.join(unmet)}\n")
    print(f"needs_attention={'true' if needs else 'false'} unmet={unmet}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
