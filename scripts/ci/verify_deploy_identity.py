"""Validate a served API deployment marker, not profitability or image contents.

Reads bounded JSON from stdin, emits only a narrow non-sensitive receipt.
No HTTP calls, writes, credentials or production mutation are performed here.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import re
import sys

MAX_BYTES = 262144
SERVICES = ("selector-api", "sim-ctl", "recon", "relays-client", "searcher-rs", "math-engine", "token-enricher")


def utc_timestamp(value: object) -> datetime:
    if not isinstance(value, str) or not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,6})?Z", value):
        raise ValueError("invalid_utc_timestamp")
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate_json_key")
        result[key] = value
    return result


def reject_constant(_value: str) -> None:
    raise ValueError("non_finite_json")


def parse_payload(raw: bytes) -> object:
    if not raw or len(raw) > MAX_BYTES:
        raise ValueError("status_payload_size_invalid")
    return json.loads(raw.decode("utf-8"), object_pairs_hook=unique_object, parse_constant=reject_constant)


def validate(payload: object, sha: str, run_id: str, deployed_at: str, *, now: datetime | None = None) -> dict[str, object]:
    if not re.fullmatch(r"[0-9a-f]{40}", sha) or not re.fullmatch(r"[1-9][0-9]*", run_id):
        raise ValueError("expected_identity_invalid")
    deployed = utc_timestamp(deployed_at)
    if not isinstance(payload, dict) or payload.get("ok") is not True:
        raise ValueError("api_status_not_healthy")
    marker = payload.get("deploy")
    if not isinstance(marker, dict) or marker.get("sha") != sha or marker.get("id") != run_id or marker.get("at") != deployed_at:
        raise ValueError("served_deployment_identity_mismatch")
    observed = utc_timestamp(payload.get("ts"))
    now = now or datetime.now(timezone.utc)
    age = (now - observed).total_seconds()
    if age > 60 or age < -30 or observed < deployed:
        raise ValueError("status_timestamp_not_current")
    services = payload.get("services")
    if not isinstance(services, dict):
        raise ValueError("services_evidence_missing")
    for service in SERVICES:
        item = services.get(service)
        if not isinstance(item, dict) or item.get("ok") is not True or type(item.get("status")) is not int or item["status"] != 200:
            raise ValueError("upstream_not_healthy:" + service)
    return {"schema_version": 1, "verification": "served_api_marker_and_upstream_health",
            "sha": sha, "run_id": run_id, "deployed_at": deployed_at, "observed_at": payload["ts"],
            "verified_services": list(SERVICES), "image_content_attested": False,
            "persistence_verified": False, "execution_verified": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--deployed-at", required=True)
    args = parser.parse_args()
    try:
        payload = parse_payload(sys.stdin.buffer.read(MAX_BYTES + 1))
        result = validate(payload, args.sha, args.run_id, args.deployed_at)
    except (ValueError, TypeError, KeyError, UnicodeError):
        # Never print the input body or an exception containing upstream content.
        print("::error::Served API identity/health evidence missing, stale or invalid", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
