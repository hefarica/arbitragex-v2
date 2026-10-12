"""Precondition probe for the G4 bounded wait: is the served /status READY?

Division of responsibility (DEPLOY-G4-WAIT-01, 2026-10-12):

- THIS probe answers one transient question only: are all upstreams healthy and
  is the status timestamp current yet? A cold stack may be waited for.
- The strict identity verdict stays in `verify_deploy_identity.py` and is NEVER
  retried: a wrong served identity must fail fast (no amount of waiting fixes a
  mismatch), while an unready stack deserves a bounded wait.

Reads bounded JSON from stdin; prints one value-free word and nothing else.
No HTTP calls, writes, credentials or production mutation are performed here.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import sys

from verify_deploy_identity import MAX_BYTES, SERVICES, parse_payload

MAX_AGE_SECONDS = 60
MAX_FUTURE_SKEW_SECONDS = 30


def ready(payload: object, *, now: datetime | None = None) -> bool:
    """True only when the transient surface is ready: ok, 7 upstreams, fresh ts."""
    if not isinstance(payload, dict) or payload.get("ok") is not True:
        return False
    services = payload.get("services")
    if not isinstance(services, dict):
        return False
    for name in SERVICES:
        item = services.get(name)
        if not isinstance(item, dict) or item.get("ok") is not True:
            return False
        if type(item.get("status")) is not int or item["status"] != 200:
            return False
    ts = payload.get("ts")
    if not isinstance(ts, str):
        return False
    try:
        observed = datetime.fromisoformat(ts.replace("Z", "+00:00"))
    except ValueError:
        return False
    if observed.tzinfo is None:
        return False
    age = ((now or datetime.now(timezone.utc)) - observed).total_seconds()
    return -MAX_FUTURE_SKEW_SECONDS <= age <= MAX_AGE_SECONDS


def identity_mismatch(payload: object, sha: str, run_id: str, deployed_at: str) -> bool:
    """True when a SERVED marker is present and differs from the expected one.

    A mismatch is TERMINAL: waiting cannot turn a wrong served identity into the
    right one, so the bounded wait must stop immediately instead of burning its
    budget. An ABSENT marker is not a mismatch — it is an unready stack (the
    api-server may still be hydrating its deploy stamp), so it stays retryable.
    """
    if not isinstance(payload, dict):
        return False
    marker = payload.get("deploy")
    if not isinstance(marker, dict):
        return False
    return (marker.get("sha") != sha or marker.get("id") != run_id
            or marker.get("at") != deployed_at)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--deployed-at", required=True)
    args = parser.parse_args()
    try:
        payload = parse_payload(sys.stdin.buffer.read(MAX_BYTES + 1))
    except (ValueError, TypeError, UnicodeError):
        # Never echo the input body: same no-leak property as the identity verifier.
        print("not_ready:payload")
        return 1
    # Terminal first: a served-but-wrong marker must stop the bounded wait now.
    if identity_mismatch(payload, args.sha, args.run_id, args.deployed_at):
        print("terminal:served_identity_mismatch")
        return 2
    if ready(payload):
        print("ready")
        return 0
    print("not_ready:services_or_ts")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
