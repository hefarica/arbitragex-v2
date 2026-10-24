#!/usr/bin/env python3
"""Per-entry expiry for audit allowlist ignores (A1 · audit gate).

WHY THIS EXISTS
---------------
`security.yml` carried ONE global expiry (`ALLOWLIST_EXPIRES`). While it held,
adding an `--ignore RUSTSEC-…` was free for years and no individual entry could
ever fail on its own: on 2026-10-21 the date stood at 2030-12-31 with 27 cargo
ignores under it. A single future date is not a caducidad, it is "never".

This check adds the missing instrument WITHOUT re-dating entries nobody has
re-justified: entries listed in the manifest carry their own `expires`, `owner`
and `reason`, and the job fails when one of them expires. Ignores that exist in
the workflow but are not in the manifest are **reported** as backlog (count and
ids) and never fail: hiding them would be the same lie in the other direction.

EXIT CODES
----------
0 — no enforced entry is expired, no manifest entry is stale.
1 — expired enforced entry, stale enforced entry, or malformed manifest.
"""
from __future__ import annotations

import argparse
import datetime
import json
import re
import sys
from pathlib import Path

IGNORE_RE = re.compile(r"--ignore\s+(RUSTSEC-\d{4}-\d{4})")


def load_manifest(path: Path, side: str) -> list[dict]:
    data = json.loads(path.read_text())
    entries = data.get(side)
    if not isinstance(entries, list):
        raise ValueError(f"manifest {path} has no '{side}' list")
    seen: set[str] = set()
    for entry in entries:
        for key in ("id", "reason", "owner", "expires"):
            if not isinstance(entry.get(key), str) or not entry[key].strip():
                raise ValueError(f"entry {entry!r} is missing a non-empty '{key}'")
        if entry["id"] in seen:
            raise ValueError(f"duplicate manifest id {entry['id']}")
        seen.add(entry["id"])
        try:
            datetime.date.fromisoformat(entry["expires"])
        except ValueError as error:
            raise ValueError(f"entry {entry['id']}: bad expires {entry['expires']!r}") from error
    return entries


def workflow_ignores(path: Path) -> set[str]:
    return set(IGNORE_RE.findall(path.read_text()))


def assess(entries: list[dict], ignores: set[str], today: datetime.date) -> tuple[list[str], list[str], list[str]]:
    """Return (errors, expired_ok, backlog)."""
    errors: list[str] = []
    expired: list[str] = []
    for entry in entries:
        expires = datetime.date.fromisoformat(entry["expires"])
        enforced = entry.get("enforced", True)
        missing = entry["id"] not in ignores
        if not enforced:
            continue
        if expires < today:
            message = (
                f"{entry['id']} expired {entry['expires']} (today {today.isoformat()}) — "
                f"owner {entry['owner']}: remediate or re-justify with a new date. "
                f"Reason on file: {entry['reason']}"
            )
            if missing:
                message += " [entry no longer present in the workflow — remove it from the manifest]"
            errors.append(message)
        elif missing:
            errors.append(
                f"{entry['id']} is enforced in the manifest but absent from the workflow — "
                "stale entry: remove it or restore the --ignore"
            )
        else:
            expired.append(f"{entry['id']} valid until {entry['expires']}")
    known = {entry["id"] for entry in entries}
    backlog = sorted(ignores - known)
    return errors, expired, backlog


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", default="scripts/ci/audit-entry-expiry.json")
    parser.add_argument("--workflow", default=".github/workflows/security.yml")
    parser.add_argument("--side", default="cargo", choices=["cargo", "npm"])
    parser.add_argument("--today", default=datetime.datetime.now(datetime.timezone.utc).date().isoformat())
    args = parser.parse_args()

    today = datetime.date.fromisoformat(args.today)
    entries = load_manifest(Path(args.manifest), args.side)
    ignores = workflow_ignores(Path(args.workflow))
    errors, valid, backlog = assess(entries, ignores, today)

    print(f"per-entry expiry ({args.side}) — manifest {len(entries)} entries, "
          f"workflow {len(ignores)} ignores, today {today.isoformat()}")
    for line in valid:
        print(f"  ok: {line}")
    if backlog:
        print(f"  backlog (un-dated, NOT enforced — {len(backlog)}): {', '.join(backlog)}")
    for message in errors:
        print(f"::error::{message}", file=sys.stderr)
    if errors:
        print(f"FAIL: {len(errors)} per-entry expiry problem(s)", file=sys.stderr)
        return 1
    print("PASS: no enforced allowlist entry is expired")
    return 0


if __name__ == "__main__":
    sys.exit(main())
