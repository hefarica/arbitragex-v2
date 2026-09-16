#!/usr/bin/env python3
"""Read-only V3 fee/identity manifest generator.

Reads the PostgreSQL-backed public liquidity catalog and verifies each V3 pool
against immutable on-chain getters at one pinned block. It NEVER writes to
PostgreSQL/Redis and can only generate ROLLBACK-by-default SQL review plans.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import re
from pathlib import Path
import time
import uuid
from typing import Any, Callable, Iterable
from urllib import parse, request

FEE_SELECTOR = "0xddca3f43"
FACTORY_SELECTOR = "0xc45a0155"
TOKEN0_SELECTOR = "0x0dfe1681"
TOKEN1_SELECTOR = "0xd21220a7"
GET_POOL_SELECTOR = "0x1698ee82"  # getPool(address,address,uint24)
SELECTORS = (FEE_SELECTOR, FACTORY_SELECTOR, TOKEN0_SELECTOR, TOKEN1_SELECTOR)
HEX40 = re.compile(r"^[0-9a-f]{40}$")
HEX64 = re.compile(r"^[0-9a-fA-F]{64}$")
HEX_QUANTITY = re.compile(r"^0x[0-9a-fA-F]+$")
CSV_FORMULA_PREFIXES = ("=", "+", "-", "@", "\t", "\r", "\n")
DEFAULT_CATALOG = "https://arbx.ape-tv.net"
USER_AGENT = "ArbitrageX-V3-DataIntegrity/1.0"


def _json_request(url: str, payload: Any | None = None, retries: int = 3) -> Any:
    body = None if payload is None else json.dumps(payload).encode("utf-8")
    headers = {"accept": "application/json", "user-agent": USER_AGENT}
    if body is not None:
        headers["content-type"] = "application/json"
    last: Exception | None = None
    for attempt in range(retries):
        try:
            req = request.Request(url, data=body, headers=headers)
            with request.urlopen(req, timeout=45) as response:
                return json.load(response)
        except Exception as exc:  # network boundary: preserve the final error
            last = exc
            if attempt + 1 < retries:
                time.sleep(0.25 * (2**attempt))
    assert last is not None
    raise last


def rpc_one(rpc_url: str, method: str, params: list[Any]) -> dict[str, Any]:
    response = _json_request(rpc_url, {"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
    if not isinstance(response, dict):
        raise RuntimeError("rpc_response_not_object")
    if response.get("jsonrpc") != "2.0":
        raise RuntimeError("rpc_response_version_invalid")
    response_id = response.get("id")
    if type(response_id) is not int or response_id != 1:
        raise RuntimeError("rpc_response_id_invalid")
    if "error" in response:
        raise RuntimeError(f"rpc_response_error_member_present:{response['error']}")
    if "result" not in response:
        raise RuntimeError("rpc_response_result_missing")
    return response


def rpc_batch(rpc_url: str, calls: list[tuple[str, list[Any]]]) -> list[dict[str, Any]]:
    payload = [{"jsonrpc": "2.0", "id": i + 1, "method": method, "params": params}
               for i, (method, params) in enumerate(calls)]
    response = _json_request(rpc_url, payload)
    if not isinstance(response, list):
        raise RuntimeError("rpc_batch_not_array")
    if len(response) != len(payload) or not all(isinstance(item, dict) for item in response):
        raise RuntimeError("rpc_batch_cardinality_invalid")
    if any(item.get("jsonrpc") != "2.0" for item in response):
        raise RuntimeError("rpc_batch_version_invalid")
    if any("error" in item or "result" not in item for item in response):
        raise RuntimeError("rpc_batch_envelope_invalid")
    ids = [item.get("id") for item in response]
    if not all(type(response_id) is int for response_id in ids):
        raise RuntimeError("rpc_batch_id_type_invalid")
    expected = set(range(1, len(payload) + 1))
    if len(set(ids)) != len(ids) or set(ids) != expected:
        raise RuntimeError("rpc_batch_missing_or_duplicate_ids")
    by_id = {item["id"]: item for item in response}
    return [by_id[i] for i in range(1, len(payload) + 1)]


def decode_u256(value: Any) -> int | None:
    if not isinstance(value, str) or not HEX_QUANTITY.fullmatch(value):
        return None
    try:
        return int(value, 16)
    except ValueError:
        return None


def decode_abi_uint(value: Any) -> int | None:
    if not isinstance(value, str) or len(value) != 66 or not value.startswith("0x"):
        return None
    if not HEX64.fullmatch(value[2:]):
        return None
    return int(value, 16)


def decode_address(value: Any) -> str | None:
    if not isinstance(value, str) or len(value) != 66 or not value.startswith("0x"):
        return None
    if value[2:26] != "0" * 24:
        return None
    tail = value[26:].lower()
    if any(ch not in "0123456789abcdef" for ch in tail) or tail == "0" * 40:
        return None
    return "0x" + tail


def classify(catalog_fee: int | None, onchain_fee: int | None, identity_ok: bool) -> str:
    if onchain_fee is None:
        return "ONCHAIN_ERROR"
    if not identity_ok:
        return "IDENTITY_MISMATCH"
    if catalog_fee is None:
        return "MISSING_FEE"
    return "MATCH" if catalog_fee == onchain_fee else "FEE_MISMATCH"


def _catalog_url(base: str, params: dict[str, str]) -> str:
    return base.rstrip("/") + "/api/v1/pools?" + parse.urlencode(params)


def _canonical_uuid(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    try:
        parsed = uuid.UUID(value)
    except (ValueError, AttributeError):
        return None
    canonical = str(parsed)
    return canonical if value.lower() == canonical else None


def _canonical_evm_address(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    normalized = value.lower()
    if not normalized.startswith("0x") or len(normalized) != 42:
        return None
    if not HEX40.fullmatch(normalized[2:]) or normalized == "0x" + "0" * 40:
        return None
    return normalized


def _validate_catalog_page(
    page: Any, *, level: str, chain_id: int, dex_id: str | None, expected_limit: int,
) -> list[dict[str, Any]]:
    if not isinstance(page, dict):
        raise RuntimeError("catalog_page_not_object")
    if page.get("schema_version") != 1 or page.get("source") != "postgresql-registry":
        raise RuntimeError("catalog_provenance_invalid")
    if page.get("level") != level or page.get("execution_verified") is not False:
        raise RuntimeError("catalog_provenance_invalid")
    scope = page.get("scope")
    if not isinstance(scope, dict) or str(scope.get("chain_id")) != str(chain_id):
        raise RuntimeError("catalog_scope_invalid")
    if scope.get("dex_id") != dex_id or scope.get("q") != "":
        raise RuntimeError("catalog_scope_invalid")
    if page.get("limit") != expected_limit or page.get("counts_include_inactive") is not True:
        raise RuntimeError("catalog_envelope_invalid")
    items = page.get("items")
    if not isinstance(items, list) or page.get("count") != len(items):
        raise RuntimeError("catalog_items_count_invalid")
    next_after = page.get("next_after")
    if next_after is not None and not isinstance(next_after, str):
        raise RuntimeError("catalog_cursor_invalid")
    for item in items:
        if not isinstance(item, dict) or str(item.get("chain_id")) != str(chain_id):
            raise RuntimeError("catalog_row_scope_invalid")
        if _canonical_uuid(item.get("id")) is None:
            raise RuntimeError("catalog_row_id_invalid")
        if level == "pools":
            if item.get("dex_id") != dex_id or item.get("protocol_type") != "UNISWAP_V3":
                raise RuntimeError("catalog_row_scope_invalid")
            if "active" not in item:
                raise RuntimeError("catalog_pool_active_missing")
            active_value = item["active"]
            if active_value is not None and type(active_value) is not bool:
                raise RuntimeError("catalog_pool_active_invalid")
            raw_fee = item.get("fee_tier")
            if raw_fee is not None:
                if not isinstance(raw_fee, str) or not raw_fee or not raw_fee.isascii() or not raw_fee.isdigit():
                    raise RuntimeError("catalog_fee_tier_invalid")
                if len(raw_fee) > 1 and raw_fee.startswith("0"):
                    raise RuntimeError("catalog_fee_tier_invalid")
                fee_value = int(raw_fee)
                if fee_value < 0 or fee_value >= 1_000_000:
                    raise RuntimeError("catalog_fee_tier_invalid")
            for key in ("pool_address", "factory_address", "token0_address", "token1_address"):
                if _canonical_evm_address(item.get(key)) is None:
                    raise RuntimeError(f"catalog_{key}_invalid")
    return items


def catalog_digest(dexes: list[dict[str, Any]], pools: list[dict[str, Any]]) -> str:
    raw = json.dumps({"dexes": dexes, "pools": pools}, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()


def collect_v3_catalog(
    catalog_base: str,
    chain_id: int,
    fetch_json: Callable[[str], Any] = _json_request,
    snapshot_nonce: str | None = None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    # `snapshot` is only a cache-buster. Consistency is proven separately by
    # comparing a full second census after all on-chain verification.
    nonce = snapshot_nonce or f"{time.time_ns()}-{os.getpid()}"
    all_dexes: list[dict[str, Any]] = []
    dex_after: str | None = None
    seen_dex_ids: set[str] = set()
    while True:
        params = {
            "view": "liquidity_catalog", "level": "dexes", "chain_id": str(chain_id),
            "limit": "100", "snapshot": nonce,
        }
        if dex_after:
            params["after"] = dex_after
        dex_payload = fetch_json(_catalog_url(catalog_base, params))
        items = _validate_catalog_page(dex_payload, level="dexes", chain_id=chain_id, dex_id=None, expected_limit=100)
        for item in items:
            dex_id_value = str(item.get("id") or "")
            if dex_id_value in seen_dex_ids:
                raise RuntimeError("catalog_duplicate_dex_id")
            seen_dex_ids.add(dex_id_value)
            all_dexes.append(item)
        next_after = dex_payload.get("next_after")
        if next_after is None:
            break
        if not items or next_after == dex_after or next_after != items[-1].get("id"):
            raise RuntimeError("catalog_dex_cursor_invalid")
        dex_after = str(next_after)

    dexes = [item for item in all_dexes if item.get("protocol_type") == "UNISWAP_V3"]
    if not dexes:
        raise RuntimeError("no_v3_dexes_in_catalog")
    pools: list[dict[str, Any]] = []
    for dex in dexes:
        after: str | None = None
        fetched = 0
        while True:
            params = {
                "view": "liquidity_catalog", "level": "pools", "chain_id": str(chain_id),
                "dex_id": str(dex["id"]), "limit": "100", "snapshot": nonce,
            }
            if after:
                params["after"] = after
            page = fetch_json(_catalog_url(catalog_base, params))
            items = _validate_catalog_page(page, level="pools", chain_id=chain_id, dex_id=str(dex["id"]), expected_limit=100)
            pools.extend(items)
            fetched += len(items)
            next_after = page.get("next_after")
            if next_after is None:
                break
            if not items or next_after == after or next_after != items[-1].get("id"):
                raise RuntimeError("catalog_cursor_invalid")
            after = str(next_after)
        if fetched != int(dex.get("pool_count", -1)):
            raise RuntimeError(f"catalog_count_mismatch:{dex.get('label')}:{fetched}")
    if not pools:
        raise RuntimeError("empty_v3_pool_census")
    addresses = [str(pool.get("pool_address", "")).lower() for pool in pools]
    if not all(_canonical_evm_address(addr) for addr in addresses):
        raise RuntimeError("catalog_pool_address_invalid")
    if len(set(addresses)) != len(addresses):
        raise RuntimeError("catalog_duplicate_pool_address")
    pool_ids = [str(pool.get("id", "")).lower() for pool in pools]
    if len(set(pool_ids)) != len(pool_ids):
        raise RuntimeError("catalog_duplicate_pool_id")
    return dexes, pools


def _rpc_error(items: Iterable[dict[str, Any]]) -> str:
    errors = [item.get("error") for item in items if isinstance(item, dict) and item.get("error")]
    return json.dumps(errors, separators=(",", ":")) if errors else ""


def encode_get_pool_call(token0: str, token1: str, fee: int) -> str:
    def address_word(value: str) -> str:
        raw = value.lower().removeprefix("0x")
        if len(raw) != 40 or any(ch not in "0123456789abcdef" for ch in raw) or raw == "0" * 40:
            raise ValueError("factory_get_pool_address_invalid")
        return "0" * 24 + raw
    if fee < 0 or fee >= 1_000_000:
        raise ValueError("factory_get_pool_fee_invalid")
    return GET_POOL_SELECTOR + address_word(token0) + address_word(token1) + f"{fee:064x}"


def verify_pools(
    pools: list[dict[str, Any]], rpc_url: str, block_hex: str, block_number: int,
    block_hash: str, batch_pools: int,
) -> list[dict[str, Any]]:
    if batch_pools <= 0:
        raise RuntimeError("batch_pools_must_be_positive")
    del block_hex  # calls are deliberately bound to blockHash, never number.
    rows: list[dict[str, Any]] = []
    rpc_host = parse.urlparse(rpc_url).hostname or "unknown"
    block_ref = {"blockHash": block_hash, "requireCanonical": True}
    for start in range(0, len(pools), batch_pools):
        chunk = pools[start:start + batch_pools]
        calls = [("eth_call", [{"to": pool["pool_address"], "data": selector}, block_ref])
                 for pool in chunk for selector in SELECTORS]
        replies = rpc_batch(rpc_url, calls)
        chunk_rows: list[dict[str, Any]] = []
        mapping_calls: list[tuple[str, list[Any]]] = []
        mapping_row_indexes: list[int] = []
        for idx, pool in enumerate(chunk):
            group = replies[idx * 4:(idx + 1) * 4]
            values = [item.get("result") for item in group]
            verification_errors: list[str] = []
            fee = decode_abi_uint(values[0])
            factory = decode_address(values[1])
            token0 = decode_address(values[2])
            token1 = decode_address(values[3])
            if values[0] is None:
                verification_errors.append("fee_result_missing")
            elif fee is None:
                verification_errors.append("fee_abi_invalid")
            if fee is not None and fee >= 1_000_000:
                verification_errors.append("fee_out_of_range")
                fee = None
            for label, raw, decoded in (("factory", values[1], factory), ("token0", values[2], token0), ("token1", values[3], token1)):
                if raw is None:
                    verification_errors.append(f"{label}_result_missing")
                elif decoded is None:
                    verification_errors.append(f"{label}_abi_invalid")
            expected_factory = str(pool.get("factory_address") or "").lower()
            expected_token0 = str(pool.get("token0_address") or "").lower()
            expected_token1 = str(pool.get("token1_address") or "").lower()
            basic_identity_ok = bool(factory and token0 and token1) and (
                factory == expected_factory and token0 == expected_token0 and token1 == expected_token1
            )
            raw_fee = pool.get("fee_tier")
            # Catalog provenance validation guarantees either explicit JSON null
            # or a canonical base-10 decimal string, so conversion is lossless.
            catalog_fee = None if raw_fee is None else int(raw_fee)
            row = {
                "pool_id": pool["id"], "chain_id": str(pool["chain_id"]),
                "dex_name": pool["dex_name"], "protocol_type": pool["protocol_type"],
                "pool_address": str(pool["pool_address"]).lower(), "active": pool.get("active"),
                "catalog_fee": catalog_fee, "onchain_fee": fee,
                "catalog_factory": expected_factory, "onchain_factory": factory,
                "catalog_token0": expected_token0, "onchain_token0": token0,
                "token0_symbol": pool.get("token0_symbol"),
                "catalog_token1": expected_token1, "onchain_token1": token1,
                "token1_symbol": pool.get("token1_symbol"),
                "factory_pool": None, "factory_mapping_ok": False,
                "identity_ok": False, "classification": "ONCHAIN_ERROR",
                "block_number": block_number, "block_hash": block_hash, "rpc_host": rpc_host,
                "rpc_error": _rpc_error(group),
                "verification_error": ",".join(verification_errors),
                "_basic_identity_ok": basic_identity_ok,
            }
            chunk_rows.append(row)
            if fee is not None and factory and token0 and token1:
                mapping_calls.append(("eth_call", [{"to": factory, "data": encode_get_pool_call(token0, token1, fee)}, block_ref]))
                mapping_row_indexes.append(idx)

        if mapping_calls:
            mapping_replies = rpc_batch(rpc_url, mapping_calls)
            for map_idx, reply in enumerate(mapping_replies):
                row = chunk_rows[mapping_row_indexes[map_idx]]
                raw = reply.get("result")
                factory_pool = decode_address(raw)
                errors = [part for part in str(row["verification_error"]).split(",") if part]
                if raw is None:
                    errors.append("factory_get_pool_result_missing")
                elif factory_pool is None:
                    errors.append("factory_get_pool_abi_invalid")
                mapping_ok = factory_pool == row["pool_address"]
                if factory_pool is not None and not mapping_ok:
                    errors.append("factory_mapping_mismatch")
                rpc_extra = _rpc_error([reply])
                if rpc_extra:
                    row["rpc_error"] = ";".join(part for part in (str(row["rpc_error"]), rpc_extra) if part)
                row["factory_pool"] = factory_pool
                row["factory_mapping_ok"] = mapping_ok
                row["verification_error"] = ",".join(errors)

        for row in chunk_rows:
            row["identity_ok"] = bool(row.pop("_basic_identity_ok")) and bool(row["factory_mapping_ok"])
            row["classification"] = classify(row["catalog_fee"], row["onchain_fee"], bool(row["identity_ok"]))
            rows.append(row)
        time.sleep(0.10)
    if len(rows) != len(pools):
        raise RuntimeError("verified_pool_count_mismatch")
    return rows


CSV_FIELDS = [
    "pool_id", "chain_id", "dex_name", "protocol_type", "pool_address", "active",
    "catalog_fee", "onchain_fee", "catalog_factory", "onchain_factory",
    "catalog_token0", "onchain_token0", "token0_symbol", "catalog_token1",
    "onchain_token1", "token1_symbol", "factory_pool", "factory_mapping_ok",
    "identity_ok", "classification", "block_number", "block_hash", "rpc_host",
    "rpc_error", "verification_error",
]


def claim_output_dir(path: Path) -> None:
    # mkdir(exist_ok=False) is the ownership primitive: two concurrent runs
    # cannot both claim the same evidence directory, even when it was empty.
    try:
        path.mkdir(parents=True, exist_ok=False)
    except FileExistsError as exc:
        raise RuntimeError("output_dir_already_claimed") from exc


def positive_int(value: str) -> int:
    try:
        parsed = int(value)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("must be an integer") from exc
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be > 0")
    return parsed


def status_url(base: str, nonce: str) -> str:
    return base.rstrip("/") + "/api/status?" + parse.urlencode({"integrity_nonce": nonce})


def require_expected_deploy_sha(value: Any) -> str:
    if not isinstance(value, str):
        raise RuntimeError("expected_deploy_sha_required")
    normalized = value.strip().lower()
    if not HEX40.fullmatch(normalized):
        raise RuntimeError("expected_deploy_sha_invalid")
    return normalized


def require_stable_catalog(
    before_dexes: list[dict[str, Any]], before_pools: list[dict[str, Any]],
    after_dexes: list[dict[str, Any]], after_pools: list[dict[str, Any]],
) -> str:
    before = catalog_digest(before_dexes, before_pools)
    after = catalog_digest(after_dexes, after_pools)
    if before != after:
        raise RuntimeError("catalog_drift_detected")
    return before


def assert_block_still_canonical(
    rpc_url: str, block_hex: str, expected_number: int, expected_hash: str,
) -> None:
    result = rpc_one(rpc_url, "eth_getBlockByNumber", [block_hex, False]).get("result")
    if not isinstance(result, dict):
        raise RuntimeError("rpc_block_reorg_detected")
    if decode_u256(result.get("number")) != expected_number:
        raise RuntimeError("rpc_block_number_mismatch")
    if str(result.get("hash", "")).lower() != expected_hash.lower():
        raise RuntimeError("rpc_block_reorg_detected")


def _csv_safe_value(value: Any) -> Any:
    if not isinstance(value, str) or not value:
        return value
    stripped = value.lstrip(" \t\r\n")
    if stripped and stripped[0] in CSV_FORMULA_PREFIXES:
        return "\'" + value
    return value


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_FIELDS)
        writer.writeheader()
        for row in rows:
            writer.writerow({key: _csv_safe_value(row.get(key)) for key in CSV_FIELDS})


def build_rollback_sql(rows: list[dict[str, Any]], active_only: bool) -> str:
    candidates = [row for row in rows if row["classification"] in {"MISSING_FEE", "FEE_MISMATCH"}
                  and row["identity_ok"] and row.get("factory_mapping_ok") is True
                  and (not active_only or row["active"] is True)]
    lines = [
        "-- PREPARED ONLY: this plan cannot commit by itself.",
        "-- Re-run manifest immediately before any approved apply.",
        "BEGIN;", "SET LOCAL lock_timeout = '2s';", "SET LOCAL statement_timeout = '30s';",
    ]
    for row in candidates:
        old = "NULL" if row["catalog_fee"] is None else str(row["catalog_fee"])
        active = "TRUE" if row["active"] is True else "FALSE" if row["active"] is False else "NULL"
        lines.append(
            "UPDATE pools AS p SET fee_tier = {new} WHERE p.id = '{pool_id}'::uuid "
            "AND p.chain_id = {chain} AND lower(p.address) = '{address}' "
            "AND p.fee_tier IS NOT DISTINCT FROM {old} AND p.is_active IS NOT DISTINCT FROM {active} "
            "AND EXISTS (SELECT 1 FROM factories f WHERE f.id=p.factory_id AND f.chain_id=p.chain_id "
            "AND lower(f.address)='{factory}') "
            "AND EXISTS (SELECT 1 FROM tokens t0 WHERE t0.id=p.token0_id AND t0.chain_id=p.chain_id "
            "AND lower(t0.address)='{token0}') "
            "AND EXISTS (SELECT 1 FROM tokens t1 WHERE t1.id=p.token1_id AND t1.chain_id=p.chain_id "
            "AND lower(t1.address)='{token1}');".format(
                new=row["onchain_fee"], pool_id=row["pool_id"], chain=row["chain_id"],
                address=row["pool_address"], old=old, active=active,
                factory=row["catalog_factory"], token0=row["catalog_token0"], token1=row["catalog_token1"],
            )
        )
    lines += ["-- Review affected-row counts here.", "ROLLBACK; -- mandatory default"]
    return "\n".join(lines) + "\n"


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_artifacts(output_dir: Path, rows: list[dict[str, Any]], metadata: dict[str, Any]) -> dict[str, Any]:
    output_dir.mkdir(parents=True, exist_ok=True)
    candidates = [row for row in rows if row["classification"] in {"MISSING_FEE", "FEE_MISMATCH"}
                  and row["identity_ok"] and row.get("factory_mapping_ok") is True]
    active = [row for row in candidates if row["active"] is True]
    files = {
        "v3_fee_manifest_full.csv": rows,
        "v3_fee_repair_candidates_all.csv": candidates,
        "v3_fee_repair_candidates_active.csv": active,
    }
    for name, data in files.items():
        write_csv(output_dir / name, data)
    (output_dir / "v3_fee_manifest_raw.json").write_text(
        json.dumps({"metadata": metadata, "rows": rows}, indent=2, sort_keys=True, ensure_ascii=False),
        encoding="utf-8",
    )
    (output_dir / "repair_active_PREPARED_ROLLBACK.sql").write_text(
        build_rollback_sql(rows, active_only=True), encoding="utf-8"
    )
    (output_dir / "repair_all_PREPARED_ROLLBACK.sql").write_text(
        build_rollback_sql(rows, active_only=False), encoding="utf-8"
    )
    from collections import Counter
    summary = dict(metadata)
    summary.update({
        "pools_verified": len(rows),
        "identity_mismatch": sum(not row["identity_ok"] for row in rows),
        "rpc_errors": sum(bool(row["rpc_error"]) for row in rows),
        "verification_errors": sum(bool(row["verification_error"]) for row in rows),
        "onchain_errors": sum(row["classification"] == "ONCHAIN_ERROR" for row in rows),
        "classification_counts": dict(Counter(row["classification"] for row in rows)),
        "active_repair_candidates": len(active), "all_repair_candidates": len(candidates),
        "fee_distribution": {str(k): v for k, v in sorted(Counter(row["onchain_fee"] for row in rows).items(), key=lambda item: str(item[0]))},
        "writes_performed": False, "human_signatures": 0, "required_human_signatures_before_apply": 2,
    })
    (output_dir / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True), encoding="utf-8")
    artifact_names = [*files, "v3_fee_manifest_raw.json", "repair_active_PREPARED_ROLLBACK.sql", "repair_all_PREPARED_ROLLBACK.sql", "summary.json"]
    hashes = {name: _sha256(output_dir / name) for name in artifact_names}
    (output_dir / "MANIFEST.sha256").write_text(
        "".join(f"{digest}  {name}\n" for name, digest in hashes.items()), encoding="utf-8"
    )
    signoff = (
        "# V3 Fee Data Integrity Sign-off — NOT YET SIGNED\n\n"
        f"Chain: {metadata['chain_id']}\nPinned block: {metadata['block_number']}\n"
        f"Block hash: {metadata['block_hash']}\nDeploy SHA: {metadata['source_deploy_sha']}\n\n"
        "Gate: no apply/cache reconciliation until two independent humans review the manifest, "
        "the backup restore evidence, and these hashes.\n\n"
        "Reviewer 1: __________________ Decision: ______ UTC: __________ Attestation: __________________\n"
        "Reviewer 2: __________________ Decision: ______ UTC: __________ Attestation: __________________\n\n"
        "This file intentionally contains no fabricated signature.\n"
    )
    (output_dir / "SIGNOFF_REQUIRED.md").write_text(signoff, encoding="utf-8")
    return summary


def manifest_exit_code(summary: dict[str, Any]) -> int:
    return 2 if (summary.get("identity_mismatch") or summary.get("rpc_errors")
                 or summary.get("verification_errors") or summary.get("onchain_errors")) else 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog-base", default=DEFAULT_CATALOG)
    parser.add_argument("--rpc-url", default=os.environ.get("ETH_RPC"))
    parser.add_argument("--chain-id", type=int, default=1)
    parser.add_argument("--batch-pools", type=positive_int, default=20)
    parser.add_argument("--expected-deploy-sha", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    expected_sha = require_expected_deploy_sha(args.expected_deploy_sha)
    if not args.rpc_url:
        raise SystemExit("--rpc-url or ETH_RPC is required")
    claim_output_dir(args.output_dir)
    run_nonce = f"{time.time_ns()}-{os.getpid()}"

    status = _json_request(status_url(args.catalog_base, run_nonce + "-status-before"))
    if not isinstance(status, dict) or status.get("ok") is not True or not isinstance(status.get("deploy"), dict):
        raise SystemExit("served_status_invalid")
    deploy_sha = str(status["deploy"].get("sha", "")).lower()
    deploy_id = str(status["deploy"].get("id", ""))
    if not HEX40.fullmatch(deploy_sha) or not deploy_id:
        raise SystemExit("served_deploy_identity_invalid")
    if deploy_sha != expected_sha:
        raise SystemExit(f"served_deploy_sha_mismatch:{deploy_sha}")

    chain_response = rpc_one(args.rpc_url, "eth_chainId", [])
    if decode_u256(chain_response.get("result")) != args.chain_id:
        raise SystemExit("rpc_chain_id_mismatch")
    block_hex = rpc_one(args.rpc_url, "eth_blockNumber", []).get("result")
    if not isinstance(block_hex, str):
        raise SystemExit("rpc_block_number_missing")
    block_number = int(block_hex, 16)
    block = rpc_one(args.rpc_url, "eth_getBlockByNumber", [block_hex, False]).get("result")
    if not isinstance(block, dict) or not isinstance(block.get("hash"), str):
        raise SystemExit("rpc_block_hash_missing")
    if decode_u256(block.get("number")) != block_number:
        raise SystemExit("rpc_block_number_mismatch")
    block_hash = str(block["hash"]).lower()

    snapshot_seed = run_nonce
    dexes, pools = collect_v3_catalog(
        args.catalog_base, args.chain_id, snapshot_nonce=snapshot_seed + "-before"
    )
    rows = verify_pools(pools, args.rpc_url, block_hex, block_number, block_hash, args.batch_pools)
    assert_block_still_canonical(args.rpc_url, block_hex, block_number, block_hash)

    # A cache-buster alone is NOT a database snapshot. Re-read the complete
    # catalog after the on-chain pass and require byte-equivalent canonical
    # census content before accepting the evidence.
    dexes_after, pools_after = collect_v3_catalog(
        args.catalog_base, args.chain_id, snapshot_nonce=snapshot_seed + "-after"
    )
    stable_catalog_digest = require_stable_catalog(dexes, pools, dexes_after, pools_after)

    status_after = _json_request(status_url(args.catalog_base, run_nonce + "-status-after"))
    after_deploy = status_after.get("deploy", {}) if isinstance(status_after, dict) else {}
    if (status_after.get("ok") is not True if isinstance(status_after, dict) else True) or        str(after_deploy.get("sha", "")).lower() != deploy_sha or str(after_deploy.get("id", "")) != deploy_id:
        raise RuntimeError("served_deploy_changed_during_manifest")

    metadata = {
        "schema_version": 1, "phase": "WO-DI-01-read-only", "chain_id": args.chain_id,
        "source_deploy_sha": deploy_sha, "source_deploy_id": deploy_id,
        "catalog_source": "postgresql-registry", "catalog_digest": stable_catalog_digest,
        "catalog_stability_reads": 2, "v3_dexes": [dex["label"] for dex in dexes],
        "block_number": block_number, "block_hash": block_hash,
        "rpc_host": parse.urlparse(args.rpc_url).hostname or "unknown",
    }
    summary = write_artifacts(args.output_dir, rows, metadata)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return manifest_exit_code(summary)


if __name__ == "__main__":
    raise SystemExit(main())
