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
from pathlib import Path
import time
from typing import Any, Callable, Iterable
from urllib import parse, request

FEE_SELECTOR = "0xddca3f43"
FACTORY_SELECTOR = "0xc45a0155"
TOKEN0_SELECTOR = "0x0dfe1681"
TOKEN1_SELECTOR = "0xd21220a7"
SELECTORS = (FEE_SELECTOR, FACTORY_SELECTOR, TOKEN0_SELECTOR, TOKEN1_SELECTOR)
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
    return _json_request(rpc_url, {"jsonrpc": "2.0", "id": 1, "method": method, "params": params})


def rpc_batch(rpc_url: str, calls: list[tuple[str, list[Any]]]) -> list[dict[str, Any]]:
    payload = [{"jsonrpc": "2.0", "id": i + 1, "method": method, "params": params}
               for i, (method, params) in enumerate(calls)]
    response = _json_request(rpc_url, payload)
    if not isinstance(response, list):
        raise RuntimeError("rpc_batch_not_array")
    by_id = {item.get("id"): item for item in response if isinstance(item, dict)}
    if set(by_id) != set(range(1, len(payload) + 1)):
        raise RuntimeError("rpc_batch_missing_or_duplicate_ids")
    return [by_id[i] for i in range(1, len(payload) + 1)]


def decode_u256(value: Any) -> int | None:
    if not isinstance(value, str) or not value.startswith("0x") or len(value) < 3:
        return None
    try:
        return int(value, 16)
    except ValueError:
        return None


def decode_address(value: Any) -> str | None:
    if not isinstance(value, str) or not value.startswith("0x") or len(value) < 42:
        return None
    tail = value[-40:].lower()
    if any(ch not in "0123456789abcdef" for ch in tail):
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


def collect_v3_catalog(
    catalog_base: str,
    chain_id: int,
    fetch_json: Callable[[str], Any] = _json_request,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    dex_payload = fetch_json(_catalog_url(catalog_base, {
        "view": "liquidity_catalog", "level": "dexes", "chain_id": str(chain_id), "limit": "100"
    }))
    dexes = [item for item in dex_payload.get("items", []) if item.get("protocol_type") == "UNISWAP_V3"]
    if not dexes:
        raise RuntimeError("no_v3_dexes_in_catalog")
    pools: list[dict[str, Any]] = []
    for dex in dexes:
        after: str | None = None
        fetched = 0
        while True:
            params = {"view": "liquidity_catalog", "level": "pools", "chain_id": str(chain_id),
                      "dex_id": str(dex["id"]), "limit": "100"}
            if after:
                params["after"] = after
            page = fetch_json(_catalog_url(catalog_base, params))
            items = page.get("items", [])
            if not isinstance(items, list):
                raise RuntimeError("catalog_items_not_array")
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
    addresses = [str(pool.get("pool_address", "")).lower() for pool in pools]
    if not all(addr.startswith("0x") and len(addr) == 42 for addr in addresses):
        raise RuntimeError("catalog_pool_address_invalid")
    if len(set(addresses)) != len(addresses):
        raise RuntimeError("catalog_duplicate_pool_address")
    return dexes, pools


def _rpc_error(items: Iterable[dict[str, Any]]) -> str:
    errors = [item.get("error") for item in items if isinstance(item, dict) and item.get("error")]
    return json.dumps(errors, separators=(",", ":")) if errors else ""


def verify_pools(
    pools: list[dict[str, Any]], rpc_url: str, block_hex: str, block_number: int,
    block_hash: str, batch_pools: int,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    rpc_host = parse.urlparse(rpc_url).hostname or "unknown"
    for start in range(0, len(pools), batch_pools):
        chunk = pools[start:start + batch_pools]
        calls = [("eth_call", [{"to": pool["pool_address"], "data": selector}, block_hex])
                 for pool in chunk for selector in SELECTORS]
        replies = rpc_batch(rpc_url, calls)
        for idx, pool in enumerate(chunk):
            group = replies[idx * 4:(idx + 1) * 4]
            values = [item.get("result") for item in group]
            fee = decode_u256(values[0])
            factory = decode_address(values[1])
            token0 = decode_address(values[2])
            token1 = decode_address(values[3])
            expected_factory = str(pool.get("factory_address") or "").lower()
            expected_token0 = str(pool.get("token0_address") or "").lower()
            expected_token1 = str(pool.get("token1_address") or "").lower()
            identity_ok = bool(factory and token0 and token1) and (
                factory == expected_factory and token0 == expected_token0 and token1 == expected_token1
            )
            raw_fee = pool.get("fee_tier")
            catalog_fee = None if raw_fee in (None, "") else int(raw_fee)
            rows.append({
                "pool_id": pool["id"], "chain_id": str(pool["chain_id"]),
                "dex_name": pool["dex_name"], "protocol_type": pool["protocol_type"],
                "pool_address": str(pool["pool_address"]).lower(), "active": pool.get("active"),
                "catalog_fee": catalog_fee, "onchain_fee": fee,
                "catalog_factory": expected_factory, "onchain_factory": factory,
                "catalog_token0": expected_token0, "onchain_token0": token0,
                "token0_symbol": pool.get("token0_symbol"),
                "catalog_token1": expected_token1, "onchain_token1": token1,
                "token1_symbol": pool.get("token1_symbol"), "identity_ok": identity_ok,
                "classification": classify(catalog_fee, fee, identity_ok),
                "block_number": block_number, "block_hash": block_hash, "rpc_host": rpc_host,
                "rpc_error": _rpc_error(group),
            })
        time.sleep(0.10)
    return rows


CSV_FIELDS = [
    "pool_id", "chain_id", "dex_name", "protocol_type", "pool_address", "active",
    "catalog_fee", "onchain_fee", "catalog_factory", "onchain_factory",
    "catalog_token0", "onchain_token0", "token0_symbol", "catalog_token1",
    "onchain_token1", "token1_symbol", "identity_ok", "classification",
    "block_number", "block_hash", "rpc_host", "rpc_error",
]


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_FIELDS)
        writer.writeheader()
        writer.writerows(rows)


def build_rollback_sql(rows: list[dict[str, Any]], active_only: bool) -> str:
    candidates = [row for row in rows if row["classification"] in {"MISSING_FEE", "FEE_MISMATCH"}
                  and row["identity_ok"] and (not active_only or row["active"] is True)]
    lines = [
        "-- PREPARED ONLY: this plan cannot commit by itself.",
        "-- Re-run manifest immediately before any approved apply.",
        "BEGIN;", "SET LOCAL lock_timeout = '2s';", "SET LOCAL statement_timeout = '30s';",
    ]
    for row in candidates:
        old = "NULL" if row["catalog_fee"] is None else str(row["catalog_fee"])
        lines.append(
            "UPDATE pools SET fee_tier = {new} WHERE id = '{pool_id}'::uuid "
            "AND chain_id = {chain} AND lower(address) = '{address}' "
            "AND fee_tier IS NOT DISTINCT FROM {old};".format(
                new=row["onchain_fee"], pool_id=row["pool_id"], chain=row["chain_id"],
                address=row["pool_address"], old=old,
            )
        )
    lines += ["-- Review affected-row counts here.", "ROLLBACK; -- mandatory default"]
    return "\n".join(lines) + "\n"


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_artifacts(output_dir: Path, rows: list[dict[str, Any]], metadata: dict[str, Any]) -> dict[str, Any]:
    output_dir.mkdir(parents=True, exist_ok=True)
    candidates = [row for row in rows if row["classification"] in {"MISSING_FEE", "FEE_MISMATCH"}
                  and row["identity_ok"]]
    active = [row for row in candidates if row["active"] is True]
    files = {
        "v3_fee_manifest_full.csv": rows,
        "v3_fee_repair_candidates_all.csv": candidates,
        "v3_fee_repair_candidates_active.csv": active,
    }
    for name, data in files.items():
        write_csv(output_dir / name, data)
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
        "classification_counts": dict(Counter(row["classification"] for row in rows)),
        "active_repair_candidates": len(active), "all_repair_candidates": len(candidates),
        "fee_distribution": {str(k): v for k, v in sorted(Counter(row["onchain_fee"] for row in rows).items(), key=lambda item: str(item[0]))},
        "writes_performed": False, "human_signatures": 0, "required_human_signatures_before_apply": 2,
    })
    (output_dir / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True), encoding="utf-8")
    artifact_names = [*files, "repair_active_PREPARED_ROLLBACK.sql", "repair_all_PREPARED_ROLLBACK.sql", "summary.json"]
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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog-base", default=DEFAULT_CATALOG)
    parser.add_argument("--rpc-url", default=os.environ.get("ETH_RPC"))
    parser.add_argument("--chain-id", type=int, default=1)
    parser.add_argument("--batch-pools", type=int, default=20)
    parser.add_argument("--expected-deploy-sha")
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.rpc_url:
        raise SystemExit("--rpc-url or ETH_RPC is required")
    status = _json_request(args.catalog_base.rstrip("/") + "/api/status")
    deploy_sha = str(status.get("deploy", {}).get("sha", ""))
    if args.expected_deploy_sha and deploy_sha != args.expected_deploy_sha:
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
    block_hash = block["hash"]
    dexes, pools = collect_v3_catalog(args.catalog_base, args.chain_id)
    rows = verify_pools(pools, args.rpc_url, block_hex, block_number, block_hash, args.batch_pools)
    metadata = {
        "schema_version": 1, "phase": "WO-DI-01-read-only", "chain_id": args.chain_id,
        "source_deploy_sha": deploy_sha, "source_deploy_id": status.get("deploy", {}).get("id"),
        "catalog_source": "postgresql-registry", "v3_dexes": [dex["label"] for dex in dexes],
        "block_number": block_number, "block_hash": block_hash,
        "rpc_host": parse.urlparse(args.rpc_url).hostname or "unknown",
    }
    summary = write_artifacts(args.output_dir, rows, metadata)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 2 if summary["identity_mismatch"] or summary["rpc_errors"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
