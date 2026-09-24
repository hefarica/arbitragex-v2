"""Versioned provenance contracts and sink reconciliation. Audit only.

Hashes establish consistency/integrity of supplied bytes, not the truth of an
oracle, successful EVM execution, a signature, or a durable database COMMIT.
Production evidence still requires independent source/receipt attestation.
"""
from __future__ import annotations
import hashlib
import json
import re
from collections import defaultdict
from typing import Any, Iterable

HASH = re.compile(r"^[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
BLOCK_HASH = re.compile(r"^0x[0-9a-fA-F]{64}$")
VALUE_STATES = frozenset({"observed", "computed", "estimated", "stale"})
EMPTY_STATES = frozenset({"missing", "not_computed", "not_applicable", "disabled", "invalid"})
STATES = VALUE_STATES | EMPTY_STATES
DEFAULT_STAGES = ("ingested", "evaluated", "persisted", "published", "api_served", "normalized", "rendered")
CONTROL_STAGES = ("requested", "persisted", "applied", "reflected")


def canonical_bytes(value: Any) -> bytes:
    """ARBX-CJSON-1: ASCII object keys, no float, no unsafe JS integers.

    Financial values are exact decimal strings. This is an explicit restricted
    JSON profile, NOT a claim to implement general RFC 8785/JCS.
    """
    nodes = 0
    def visit(v: Any, depth: int = 0) -> None:
        nonlocal nodes
        nodes += 1
        if nodes > 100_000 or depth > 32:
            raise ValueError("payload complexity limit")
        if isinstance(v, dict):
            for k, x in v.items():
                if not isinstance(k, str) or not k.isascii() or len(k) > 256:
                    raise ValueError("ASCII object key required (max 256 chars)")
                visit(x, depth + 1)
        elif isinstance(v, list):
            for x in v:
                visit(x, depth + 1)
        elif isinstance(v, bool) or v is None:
            return
        elif isinstance(v, int):
            if abs(v) > (1 << 53) - 1:
                raise ValueError("large integers must be decimal strings")
        elif isinstance(v, str):
            v.encode("utf-8", "strict")
        else:
            raise ValueError("unsupported JSON value; finite decimals must be strings")
    visit(value)
    return json.dumps(value, sort_keys=True, ensure_ascii=False,
                      allow_nan=False, separators=(",", ":")).encode("utf-8")


def digest(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def flatten(value: Any, prefix: str = "") -> dict[str, Any]:
    """Inventory every observed field, including null, empty arrays and objects."""
    result: dict[str, Any] = {}
    def walk(v: Any, path: str, depth: int) -> None:
        if depth > 32 or len(result) > 100_000:
            raise ValueError("payload complexity limit")
        if isinstance(v, dict) and v:
            for k, x in v.items():
                walk(x, f"{path}.{k}" if path else str(k), depth + 1)
        elif isinstance(v, list) and v:
            for i, x in enumerate(v):
                walk(x, f"{path}[{i}]", depth + 1)
        else:
            result[path or "$root"] = v
    walk(value, prefix, 0)
    return result


def validate_envelope(envelope: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if envelope.get("schema") != "arbx.field-lineage.v1":
        errors.append("schema_unknown")
    for k in ("event_id", "strategy_id"):
        if not isinstance(envelope.get(k), str) or not envelope[k]:
            errors.append(f"{k}_missing")
    for k in ("context_id", "route_hash", "config_hash", "strategy_revision"):
        if not HASH.fullmatch(str(envelope.get(k, ""))):
            errors.append(f"{k}_missing_or_invalid")
    if not COMMIT.fullmatch(str(envelope.get("source_commit", ""))):
        errors.append("source_commit_missing_or_invalid")
    fields = envelope.get("fields")
    if not isinstance(fields, dict) or not fields:
        return errors + ["fields_missing"]
    for name, f in fields.items():
        pre = f"{name}:"
        if not isinstance(f, dict):
            errors.append(pre + "invalid_field_record"); continue
        state, value = f.get("state"), f.get("value")
        if state not in STATES:
            errors.append(pre + "state_unknown")
        if state in EMPTY_STATES and value is not None:
            errors.append(pre + "absence_must_not_carry_a_number")
        if state in VALUE_STATES and value is None:
            errors.append(pre + "value_missing")
        if state in EMPTY_STATES | {"stale"} and not f.get("reason"):
            errors.append(pre + "reason_missing")
        if not isinstance(f.get("unit"), str) or not f["unit"]:
            errors.append(pre + "unit_missing")
        if state in VALUE_STATES:
            src = f.get("source")
            if not isinstance(src, dict) or not src.get("reference"):
                errors.append(pre + "source_missing"); continue
            if src.get("kind") not in {"onchain", "offchain", "calculation", "operator_config"}:
                errors.append(pre + "source_kind_unknown")
            asof = src.get("as_of_ms")
            if isinstance(asof, bool) or not isinstance(asof, int) or asof <= 0:
                errors.append(pre + "source_time_missing")
            if src.get("kind") == "onchain":
                if not BLOCK_HASH.fullmatch(str(src.get("block_hash", ""))):
                    errors.append(pre + "unverifiable_block_hash")
                if not isinstance(src.get("chain_id"), int) or src["chain_id"] <= 0:
                    errors.append(pre + "chain_id_missing")
            if state == "computed":
                hashes = src.get("input_hashes")
                if not src.get("transform_id") or not src.get("transform_version"):
                    errors.append(pre + "transform_missing")
                if not isinstance(hashes, list) or not hashes or any(not HASH.fullmatch(str(x)) for x in hashes):
                    errors.append(pre + "input_provenance_missing")
            if state == "estimated" and not src.get("model_version"):
                errors.append(pre + "estimate_model_missing")
    try:
        canonical_bytes(envelope)
    except (ValueError, UnicodeError) as exc:
        errors.append("canonical_profile:" + str(exc))
    return errors


def make_receipt(event_id: str, stage: str, context_id: str, payload: Any,
                 at_ms: int, parent: dict[str, Any] | None = None,
                 transform_id: str = "identity.v1") -> dict[str, Any]:
    """Create a receipt from bytes actually observed; not an external attestation."""
    if not event_id or not stage or not HASH.fullmatch(context_id):
        raise ValueError("receipt identity missing")
    if isinstance(at_ms, bool) or not isinstance(at_ms, int) or at_ms <= 0:
        raise ValueError("positive receipt timestamp required")
    if parent and (parent["event_id"], parent["context_id"]) != (event_id, context_id):
        raise ValueError("cross-opportunity/context evidence refused")
    body = {"schema": "arbx.receipt.v1", "event_id": event_id, "stage": stage,
            "context_id": context_id, "at_ms": at_ms, "transform_id": transform_id,
            "parent_hash": parent["receipt_hash"] if parent else None,
            "input_hash": parent["output_hash"] if parent else None,
            "output_hash": digest(payload), "payload": payload}
    return {**body, "receipt_hash": digest(body)}


def verify_receipt(receipt: dict[str, Any]) -> list[str]:
    errors = []
    required = {"schema", "event_id", "stage", "context_id", "at_ms", "transform_id", "parent_hash", "input_hash", "output_hash", "payload", "receipt_hash"}
    if not isinstance(receipt, dict):
        return ["receipt_not_an_object"]
    for key in sorted(required - set(receipt)):
        errors.append(key + "_missing")
    for key in ("output_hash", "receipt_hash"):
        if not HASH.fullmatch(str(receipt.get(key, ""))):
            errors.append(key + "_invalid")
    for key in ("parent_hash", "input_hash"):
        value = receipt.get(key)
        if value is not None and not HASH.fullmatch(str(value)):
            errors.append(key + "_invalid")
    if receipt.get("schema") != "arbx.receipt.v1":
        errors.append("schema_unknown")
    for key in ("event_id", "stage", "transform_id"):
        if not isinstance(receipt.get(key), str) or not receipt[key]:
            errors.append(key + "_missing")
    if not HASH.fullmatch(str(receipt.get("context_id", ""))):
        errors.append("context_invalid")
    stamp = receipt.get("at_ms")
    if isinstance(stamp, bool) or not isinstance(stamp, int) or stamp <= 0:
        errors.append("timestamp_invalid")
    try:
        body = {k: v for k, v in receipt.items() if k != "receipt_hash"}
        if digest(body) != receipt.get("receipt_hash"):
            errors.append("receipt_hash_mismatch")
        if "payload" not in receipt or digest(receipt["payload"]) != receipt.get("output_hash"):
            errors.append("payload_hash_mismatch")
    except (ValueError, UnicodeError) as exc:
        errors.append("canonical_profile:" + str(exc))
    return errors


def reconcile(receipts: Iterable[dict[str, Any]], expected_ids: Iterable[str],
              stages: Iterable[str] = DEFAULT_STAGES,
              max_latency_ms: int | None = None) -> dict[str, Any]:
    """Reconcile unique events against an INDEPENDENT expected-ingress census.

    A test fixture passing here certifies only receipt consistency in that
    fixture. It does not certify production delivery or authenticity of sources.
    No difference of total counters is used as proof of data loss.
    """
    if isinstance(expected_ids, str) or isinstance(stages, str):
        raise ValueError("ID/stage lists required, not a string")
    stages, expected = tuple(stages), set(expected_ids)
    if any(not isinstance(x, str) or not x for x in (*stages, *expected)):
        raise ValueError("non-empty string IDs/stages required")
    if not stages or len(set(stages)) != len(stages):
        raise ValueError("unique ordered stage contract required")
    if max_latency_ms is not None and max_latency_ms < 0:
        raise ValueError("negative SLA")
    grouped: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    failures, duplicates, unexpected = [], 0, set()
    for receipt in receipts:
        errs = verify_receipt(receipt)
        if not isinstance(receipt, dict):
            failures.append({"event_id": "", "stage": "", "errors": errs}); continue
        event_id, stage = receipt.get("event_id", ""), receipt.get("stage", "")
        if errs:
            failures.append({"event_id": event_id, "stage": stage, "errors": errs}); continue
        if event_id not in expected:
            unexpected.add(event_id)
        if stage not in stages:
            failures.append({"event_id": event_id, "stage": stage, "errors": ["stage_not_in_contract"]}); continue
        previous = grouped[event_id].get(stage)
        if previous:
            if previous["receipt_hash"] == receipt["receipt_hash"]:
                duplicates += 1
            else:
                failures.append({"event_id": event_id, "stage": stage, "errors": ["conflicting_duplicate"]})
        else:
            grouped[event_id][stage] = receipt
    rows = []
    for eid in sorted(expected):
        records = grouped.get(eid, {})
        missing = [s for s in stages if s not in records]
        errors = []
        for i, stage in enumerate(stages):
            r = records.get(stage)
            if r is None:
                continue
            if i == 0:
                if r["parent_hash"] is not None or r["input_hash"] is not None:
                    errors.append("ingress_parent_unexpected")
            else:
                parent = records.get(stages[i - 1])
                if parent:
                    if r["parent_hash"] != parent["receipt_hash"] or r["input_hash"] != parent["output_hash"]:
                        errors.append(stage + ":broken_lineage")
                    if r["context_id"] != parent["context_id"]:
                        errors.append(stage + ":context_mismatch")
                    if r["at_ms"] < parent["at_ms"]:
                        errors.append(stage + ":clock_order_unverifiable")
        latency = None
        if not missing:
            latency = records[stages[-1]]["at_ms"] - records[stages[0]]["at_ms"]
            if max_latency_ms is not None and latency > max_latency_ms:
                errors.append("latency_contract_breached")
        invalid = [f for f in failures if f["event_id"] == eid]
        rows.append({"event_id": eid, "missing_stages": missing, "errors": errors,
                     "latency_ms": latency, "complete": not missing and not errors and not invalid})
    complete = sum(1 for r in rows if r["complete"])
    return {"scope": "supplied_receipt_consistency_only", "expected_events": len(expected),
            "complete_events": complete, "exact_duplicate_receipts": duplicates,
            "unexpected_event_ids": sorted(unexpected), "invalid_receipts": failures, "events": rows,
            "observed_completeness_ratio": complete / len(expected) if expected else None,
            "verification": "CONSISTENT" if expected and complete == len(expected) and not failures and not unexpected else "INCOMPLETE_OR_INVALID",
            "source_authenticity_verified": False, "production_delivery_certified": False}


def operator_coverage(required: Iterable[int], registered: Iterable[int],
                      disabled: Iterable[int], results: list[dict[str, Any]],
                      expected_context: str) -> dict[str, Any]:
    """Keep unknown/disabled/not-computed entries in the applicability denominator."""
    required, registered, disabled = sorted(set(required)), set(registered), set(disabled)
    by_id: dict[int, list[dict[str, Any]]] = defaultdict(list)
    for row in results:
        by_id[row["operator_id"]].append(row)
    entries = []
    for oid in required:
        matches = by_id.get(oid, [])
        value = None
        if oid not in registered:
            state = "unregistered"
        elif oid in disabled:
            state = "disabled"
        elif len(matches) > 1:
            state = "conflicting_results"
        elif not matches:
            state = "not_computed"
        elif matches[0].get("context_id") != expected_context:
            state = "context_mismatch"
        elif matches[0].get("state") != "computed" or matches[0].get("value") is None:
            state = "not_computed"
        else:
            value = matches[0]["value"]
            try:
                from .math_reference import decimal
                decimal(value)
                state = "computed"
            except ValueError:
                state = "invalid"
                value = None
        entries.append({"operator_id": oid, "state": state, "value": value})
    computed = sum(r["state"] == "computed" for r in entries)
    return {"required": len(required), "computed": computed,
            "coverage": computed / len(required) if required else None,
            "operators": entries, "undeclared_results": sorted(set(by_id) - set(required))}
