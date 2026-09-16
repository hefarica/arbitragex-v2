import csv
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts" / "data_integrity" / "v3_fee_manifest.py"
spec = importlib.util.spec_from_file_location("v3_fee_manifest", MODULE_PATH)
assert spec and spec.loader
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

CHAIN = "1"
DEX_V3 = "700e5c5e-af97-41c7-bddd-90d5ad08cac7"
FACTORY = "0x" + "aa" * 20
TOKEN0 = "0x" + "bb" * 20
TOKEN1 = "0x" + "cc" * 20
POOL = "0x" + "11" * 20
BLOCK_HASH = "0x" + "dd" * 32


def word_address(address: str) -> str:
    return "0x" + "00" * 12 + address[2:].lower()


def dex_item(dex_id=DEX_V3, *, proto="UNISWAP_V3", pool_count="1", label="UniswapV3"):
    return {
        "id": dex_id, "chain_id": CHAIN, "label": label, "active": True,
        "protocol_type": proto, "factory_count": "1", "pool_count": pool_count,
    }


def pool_item(pool_id="00000000-0000-4000-8000-000000000001", *, address=POOL,
              dex_id=DEX_V3, fee="3000", active=True, symbol0="AAA", symbol1="BBB"):
    return {
        "id": pool_id, "chain_id": CHAIN, "label": address, "active": active,
        "pool_address": address, "fee_tier": fee, "dex_id": dex_id,
        "dex_name": "UniswapV3", "dex_active": True, "protocol_type": "UNISWAP_V3",
        "factory_address": FACTORY, "token0_address": TOKEN0, "token0_symbol": symbol0,
        "token1_address": TOKEN1, "token1_symbol": symbol1,
    }


def page(level, items, *, dex_id=None, next_after=None, source="postgresql-registry",
         schema_version=1, chain_id=CHAIN, limit=100):
    return {
        "schema_version": schema_version, "source": source, "level": level,
        "observed_at": "2026-09-15T00:00:00Z", "execution_verified": False,
        "scope": {"chain_id": chain_id, "dex_id": dex_id, "q": ""},
        "count": len(items), "limit": limit, "items": items,
        "next_after": next_after, "counts_include_inactive": True,
    }


def metadata():
    return {
        "schema_version": 1, "phase": "test", "chain_id": 1,
        "source_deploy_sha": "a" * 40, "source_deploy_id": "run",
        "catalog_source": "postgresql-registry", "catalog_digest": "b" * 64,
        "catalog_stability_reads": 2, "v3_dexes": ["UniswapV3"],
        "block_number": 123, "block_hash": BLOCK_HASH, "rpc_host": "rpc.invalid",
    }


class V3FeeManifestTests(unittest.TestCase):
    def test_decode_u256_and_address_are_strict(self):
        self.assertEqual(m.decode_u256("0x01f4"), 500)
        self.assertIsNone(m.decode_u256("not-hex"))
        self.assertEqual(m.decode_address(word_address(FACTORY)), FACTORY)
        self.assertIsNone(m.decode_address("0x1234"))
        self.assertEqual(m.decode_abi_uint("0x" + "00" * 31 + "01"), 1)
        self.assertIsNone(m.decode_abi_uint("0x01"))
        self.assertIsNone(m.decode_abi_uint("0x" + "0" * 59 + "0_bb8"))
        self.assertIsNone(m.decode_address("0x" + "01" + "00" * 31))
        self.assertIsNone(m.decode_address("0x" + "00" * 32))

    def test_expected_deploy_sha_is_mandatory_and_normalized(self):
        self.assertEqual(m.require_expected_deploy_sha("A" * 40), "a" * 40)
        for bad in (None, "", "unknown", "abc", "g" * 40):
            with self.assertRaises(RuntimeError):
                m.require_expected_deploy_sha(bad)

    def test_batch_size_must_be_positive_and_row_count_is_guarded(self):
        self.assertEqual(m.positive_int("20"), 20)
        for bad in ("0", "-1"):
            with self.assertRaisesRegex(Exception, "must be > 0"):
                m.positive_int(bad)
        with self.assertRaisesRegex(RuntimeError, "batch_pools_must_be_positive"):
            m.verify_pools([pool_item()], "https://rpc.invalid", "0x7b", 123, BLOCK_HASH, 0)

    def test_status_cache_busters_are_distinct(self):
        before = m.status_url("https://catalog.invalid", "run-before")
        after = m.status_url("https://catalog.invalid", "run-after")
        self.assertNotEqual(before, after)
        self.assertIn("integrity_nonce=run-before", before)
        self.assertIn("integrity_nonce=run-after", after)

    def test_rpc_one_validates_jsonrpc_envelope(self):
        original = m._json_request
        try:
            m._json_request = lambda *_a, **_k: {"jsonrpc": "2.0", "id": 1, "result": "0x1"}
            self.assertEqual(m.rpc_one("https://rpc.invalid", "eth_chainId", [])["result"], "0x1")
            invalid = [
                {"jsonrpc": "2.0", "id": True, "result": "0x1"},
                {"jsonrpc": "2.0", "id": 2, "result": "0x1"},
                {"jsonrpc": "1.0", "id": 1, "result": "0x1"},
                {"jsonrpc": "2.0", "id": 1, "error": {"code": -32000}, "result": "0x1"},
                {"jsonrpc": "2.0", "id": 1, "error": None, "result": "0x1"},
                {"jsonrpc": "2.0", "id": 1},
            ]
            for payload in invalid:
                m._json_request = lambda *_a, _payload=payload, **_k: _payload
                with self.assertRaises(RuntimeError):
                    m.rpc_one("https://rpc.invalid", "eth_chainId", [])
        finally:
            m._json_request = original

    def test_rpc_batch_rejects_duplicate_ids_before_collapsing(self):
        original = m._json_request
        m._json_request = lambda *_a, **_k: [
            {"jsonrpc": "2.0", "id": 1, "result": "0x01"}, {"jsonrpc": "2.0", "id": 2, "result": "0x02"},
            {"jsonrpc": "2.0", "id": 2, "result": "0x03"},
        ]
        try:
            with self.assertRaisesRegex(RuntimeError, "rpc_batch_cardinality_invalid"):
                m.rpc_batch("https://rpc.invalid", [("a", []), ("b", [])])
        finally:
            m._json_request = original

    def test_rpc_batch_rejects_non_integer_response_ids(self):
        original = m._json_request
        try:
            for bad_id in (True, 1.0, "1"):
                m._json_request = lambda *_a, _bad_id=bad_id, **_k: [
                    {"jsonrpc": "2.0", "id": _bad_id, "result": "0x01"},
                ]
                with self.assertRaisesRegex(RuntimeError, "rpc_batch_id_type_invalid"):
                    m.rpc_batch("https://rpc.invalid", [("a", [])])
        finally:
            m._json_request = original


    def test_rpc_batch_rejects_error_member_even_when_null_and_missing_result(self):
        original = m._json_request
        try:
            invalid_batches = [
                [{"jsonrpc": "2.0", "id": 1, "error": None, "result": "0x01"}],
                [{"jsonrpc": "2.0", "id": 1}],
            ]
            for payload in invalid_batches:
                m._json_request = lambda *_a, _payload=payload, **_k: _payload
                with self.assertRaisesRegex(RuntimeError, "rpc_batch_envelope_invalid"):
                    m.rpc_batch("https://rpc.invalid", [("a", [])])
        finally:
            m._json_request = original

    def test_catalog_provenance_and_scope_are_enforced(self):
        good = page("dexes", [dex_item()])
        self.assertEqual(len(m._validate_catalog_page(good, level="dexes", chain_id=1,
                                                       dex_id=None, expected_limit=100)), 1)
        bad_source = dict(good, source="cache-fiction")
        with self.assertRaisesRegex(RuntimeError, "catalog_provenance_invalid"):
            m._validate_catalog_page(bad_source, level="dexes", chain_id=1,
                                     dex_id=None, expected_limit=100)
        bad_scope = dict(good, scope={"chain_id": "10", "dex_id": None, "q": ""})
        with self.assertRaisesRegex(RuntimeError, "catalog_scope_invalid"):
            m._validate_catalog_page(bad_scope, level="dexes", chain_id=1,
                                     dex_id=None, expected_limit=100)

    def test_catalog_paginates_dexes_pools_and_cache_busts(self):
        seen = []
        dex_v2 = "11111111-1111-4111-8111-111111111111"
        def fake(url):
            seen.append(url)
            if "level=dexes" in url and "after=" not in url:
                return page("dexes", [dex_item(dex_v2, proto="UNISWAP_V2", pool_count="0", label="V2")],
                            next_after=dex_v2)
            if "level=dexes" in url:
                return page("dexes", [dex_item(pool_count="2")], next_after=None)
            if "after=00000000-0000-4000-8000-000000000001" in url:
                return page("pools", [pool_item("00000000-0000-4000-8000-000000000002",
                                                address="0x" + "22" * 20)], dex_id=DEX_V3)
            return page("pools", [pool_item()], dex_id=DEX_V3,
                        next_after="00000000-0000-4000-8000-000000000001")
        dexes, pools = m.collect_v3_catalog("https://catalog.invalid", 1, fake, snapshot_nonce="nonce123")
        self.assertEqual([d["id"] for d in dexes], [DEX_V3])
        self.assertEqual(len(pools), 2)
        self.assertTrue(all("snapshot=nonce123" in url for url in seen))


    def test_catalog_requires_activity_key(self):
        item = pool_item()
        item.pop("active")
        bad = page("pools", [item], dex_id=DEX_V3)
        with self.assertRaisesRegex(RuntimeError, "catalog_pool_active_missing"):
            m._validate_catalog_page(bad, level="pools", chain_id=1,
                                     dex_id=DEX_V3, expected_limit=100)

    def test_catalog_requires_fee_tier_key(self):
        item = pool_item()
        item.pop("fee_tier")
        bad = page("pools", [item], dex_id=DEX_V3)
        with self.assertRaisesRegex(RuntimeError, "catalog_fee_tier_missing"):
            m._validate_catalog_page(bad, level="pools", chain_id=1,
                                     dex_id=DEX_V3, expected_limit=100)

    def test_catalog_fee_tier_must_be_null_or_canonical_decimal_string(self):
        for bad_fee in (3000, 3000.9, True, False, "", "03", "3000.0", "+3000", "-1", "1_000"):
            bad = page("pools", [pool_item(fee=bad_fee)], dex_id=DEX_V3)
            with self.assertRaisesRegex(RuntimeError, "catalog_fee_tier_invalid"):
                m._validate_catalog_page(bad, level="pools", chain_id=1,
                                         dex_id=DEX_V3, expected_limit=100)
        good_null = page("pools", [pool_item(fee=None)], dex_id=DEX_V3)
        self.assertEqual(len(m._validate_catalog_page(good_null, level="pools", chain_id=1,
                                                      dex_id=DEX_V3, expected_limit=100)), 1)
        good = page("pools", [pool_item(fee="3000")], dex_id=DEX_V3)
        self.assertEqual(len(m._validate_catalog_page(good, level="pools", chain_id=1,
                                                      dex_id=DEX_V3, expected_limit=100)), 1)

    def test_catalog_rejects_non_boolean_pool_activity(self):
        for bad_active in (1, 0, 1.0, 0.0, "true", "false"):
            bad = page("pools", [pool_item(active=bad_active)], dex_id=DEX_V3)
            with self.assertRaisesRegex(RuntimeError, "catalog_pool_active_invalid"):
                m._validate_catalog_page(bad, level="pools", chain_id=1,
                                         dex_id=DEX_V3, expected_limit=100)

    def test_catalog_rejects_noncanonical_uuid_and_duplicate_pool_ids(self):
        bad_uuid = page("pools", [pool_item(pool_id="not-a-uuid")], dex_id=DEX_V3)
        with self.assertRaisesRegex(RuntimeError, "catalog_row_id_invalid"):
            m._validate_catalog_page(bad_uuid, level="pools", chain_id=1,
                                     dex_id=DEX_V3, expected_limit=100)

        duplicate_id = "00000000-0000-4000-8000-000000000001"
        def duplicate(url):
            if "level=dexes" in url:
                return page("dexes", [dex_item(pool_count="2")])
            return page("pools", [
                pool_item(duplicate_id, address=POOL),
                pool_item(duplicate_id, address="0x" + "22" * 20),
            ], dex_id=DEX_V3)
        with self.assertRaisesRegex(RuntimeError, "catalog_duplicate_pool_id"):
            m.collect_v3_catalog("https://catalog.invalid", 1, duplicate, snapshot_nonce="dup")

    def test_catalog_count_mismatch_and_empty_census_fail(self):
        def mismatch(url):
            if "level=dexes" in url:
                return page("dexes", [dex_item(pool_count="2")])
            return page("pools", [pool_item()], dex_id=DEX_V3)
        with self.assertRaisesRegex(RuntimeError, "catalog_count_mismatch"):
            m.collect_v3_catalog("https://catalog.invalid", 1, mismatch, snapshot_nonce="x")
        def empty(url):
            if "level=dexes" in url:
                return page("dexes", [dex_item(pool_count="0")])
            return page("pools", [], dex_id=DEX_V3)
        with self.assertRaisesRegex(RuntimeError, "empty_v3_pool_census"):
            m.collect_v3_catalog("https://catalog.invalid", 1, empty, snapshot_nonce="y")

    def test_catalog_drift_is_detected(self):
        dexes = [dex_item()]
        pools = [pool_item()]
        self.assertEqual(len(m.require_stable_catalog(dexes, pools, dexes, pools)), 64)
        changed = [dict(pool_item(), fee_tier="500")]
        with self.assertRaisesRegex(RuntimeError, "catalog_drift_detected"):
            m.require_stable_catalog(dexes, pools, dexes, changed)

    def test_output_directory_is_atomically_claimed(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "run"
            m.claim_output_dir(target)
            self.assertTrue(target.is_dir())
            with self.assertRaisesRegex(RuntimeError, "output_dir_already_claimed"):
                m.claim_output_dir(target)

    def test_block_identity_revalidation_checks_number_and_hash(self):
        original = m.rpc_one
        try:
            m.rpc_one = lambda *_a, **_k: {"result": {"number": "0x7b", "hash": "0x" + "ee" * 32}}
            with self.assertRaisesRegex(RuntimeError, "rpc_block_reorg_detected"):
                m.assert_block_still_canonical("https://rpc.invalid", "0x7b", 123, BLOCK_HASH)
            m.rpc_one = lambda *_a, **_k: {"result": {"number": "0x7c", "hash": BLOCK_HASH}}
            with self.assertRaisesRegex(RuntimeError, "rpc_block_number_mismatch"):
                m.assert_block_still_canonical("https://rpc.invalid", "0x7b", 123, BLOCK_HASH)
        finally:
            m.rpc_one = original

    def test_get_pool_encoder_and_factory_mapping(self):
        data = m.encode_get_pool_call(TOKEN0, TOKEN1, 3000)
        self.assertTrue(data.startswith(m.GET_POOL_SELECTOR))
        self.assertEqual(len(data), 2 + 8 + 64 * 3)
        captured = []
        def fake_batch(_rpc, calls):
            captured.extend(calls)
            if len(calls) == 4:
                return [
                    {"jsonrpc": "2.0", "id": 1, "result": "0x" + f"{3000:064x}"},
                    {"jsonrpc": "2.0", "id": 2, "result": word_address(FACTORY)},
                    {"jsonrpc": "2.0", "id": 3, "result": word_address(TOKEN0)},
                    {"jsonrpc": "2.0", "id": 4, "result": word_address(TOKEN1)},
                ]
            self.assertEqual(len(calls), 1)
            return [{"jsonrpc": "2.0", "id": 1, "result": word_address(POOL)}]
        original = m.rpc_batch
        m.rpc_batch = fake_batch
        try:
            rows = m.verify_pools([pool_item()], "https://rpc.invalid", "0x7b", 123, BLOCK_HASH, 1)
        finally:
            m.rpc_batch = original
        self.assertEqual(rows[0]["classification"], "MATCH")
        self.assertTrue(rows[0]["factory_mapping_ok"])
        self.assertEqual(rows[0]["factory_pool"], POOL)
        self.assertTrue(all(params[1] == {"blockHash": BLOCK_HASH, "requireCanonical": True}
                            for _method, params in captured))

    def test_factory_mapping_mismatch_fails_identity(self):
        def fake_batch(_rpc, calls):
            if len(calls) == 4:
                return [
                    {"jsonrpc": "2.0", "id": 1, "result": "0x" + f"{3000:064x}"},
                    {"jsonrpc": "2.0", "id": 2, "result": word_address(FACTORY)},
                    {"jsonrpc": "2.0", "id": 3, "result": word_address(TOKEN0)},
                    {"jsonrpc": "2.0", "id": 4, "result": word_address(TOKEN1)},
                ]
            return [{"id": 1, "result": word_address("0x" + "99" * 20)}]
        original = m.rpc_batch
        m.rpc_batch = fake_batch
        try:
            row = m.verify_pools([pool_item()], "https://rpc.invalid", "0x7b", 123, BLOCK_HASH, 1)[0]
        finally:
            m.rpc_batch = original
        self.assertFalse(row["identity_ok"])
        self.assertEqual(row["classification"], "IDENTITY_MISMATCH")
        self.assertIn("factory_mapping_mismatch", row["verification_error"])

    def test_missing_fee_result_is_terminal(self):
        def fake_batch(_rpc, calls):
            self.assertEqual(len(calls), 4)
            return [
                {"id": 1, "result": None}, {"jsonrpc": "2.0", "id": 2, "result": word_address(FACTORY)},
                {"jsonrpc": "2.0", "id": 3, "result": word_address(TOKEN0)}, {"jsonrpc": "2.0", "id": 4, "result": word_address(TOKEN1)},
            ]
        original = m.rpc_batch
        m.rpc_batch = fake_batch
        try:
            row = m.verify_pools([pool_item()], "https://rpc.invalid", "0x7b", 123, BLOCK_HASH, 1)[0]
        finally:
            m.rpc_batch = original
        self.assertEqual(row["classification"], "ONCHAIN_ERROR")
        self.assertIn("fee_result_missing", row["verification_error"])
        with tempfile.TemporaryDirectory() as tmp:
            summary = m.write_artifacts(Path(tmp), [row], metadata())
            self.assertEqual(m.manifest_exit_code(summary), 2)

    @staticmethod
    def sample_rows():
        base = {
            "pool_id": "00000000-0000-4000-8000-000000000001", "chain_id": "1",
            "dex_name": "UniswapV3", "protocol_type": "UNISWAP_V3",
            "pool_address": POOL, "active": True,
            "catalog_factory": FACTORY, "onchain_factory": FACTORY,
            "catalog_token0": TOKEN0, "onchain_token0": TOKEN0, "token0_symbol": "AAA",
            "catalog_token1": TOKEN1, "onchain_token1": TOKEN1, "token1_symbol": "BBB",
            "factory_pool": POOL, "factory_mapping_ok": True, "identity_ok": True,
            "block_number": 123, "block_hash": BLOCK_HASH, "rpc_host": "rpc.invalid",
            "rpc_error": "", "verification_error": "",
        }
        mismatch = dict(base, catalog_fee=30, onchain_fee=3000, classification="FEE_MISMATCH")
        missing = dict(base, pool_id="00000000-0000-4000-8000-000000000002",
                       pool_address="0x" + "12" * 20, factory_pool="0x" + "12" * 20,
                       active=None, catalog_fee=None, onchain_fee=500, classification="MISSING_FEE")
        match = dict(base, pool_id="00000000-0000-4000-8000-000000000003",
                     pool_address="0x" + "13" * 20, factory_pool="0x" + "13" * 20,
                     catalog_fee=100, onchain_fee=100, classification="MATCH")
        return [mismatch, missing, match]

    def test_sql_is_identity_activity_guarded_and_rollback_only(self):
        sql = m.build_rollback_sql(self.sample_rows(), active_only=True)
        self.assertIn("p.fee_tier IS NOT DISTINCT FROM 30", sql)
        self.assertIn("p.is_active IS NOT DISTINCT FROM TRUE", sql)
        self.assertIn("f.id=p.factory_id", sql)
        self.assertIn("t0.id=p.token0_id", sql)
        self.assertIn("t1.id=p.token1_id", sql)
        self.assertNotIn("fee_tier * 100", sql)
        self.assertNotIn("00000000-0000-4000-8000-000000000002", sql)
        self.assertTrue(sql.rstrip().endswith("ROLLBACK; -- mandatory default"))

    def test_all_plan_guards_null_fee_and_inactive_state(self):
        sql = m.build_rollback_sql(self.sample_rows(), active_only=False)
        self.assertIn("p.fee_tier IS NOT DISTINCT FROM NULL", sql)
        self.assertIn("p.is_active IS NOT DISTINCT FROM NULL", sql)
        self.assertIn("00000000-0000-4000-8000-000000000002", sql)

    def test_csv_formula_is_neutralized_but_raw_json_preserves_value(self):
        rows = self.sample_rows()
        rows[0] = dict(rows[0], token0_symbol='=HYPERLINK("x")')
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            m.write_artifacts(root, rows, metadata())
            with (root / "v3_fee_manifest_full.csv").open(newline="", encoding="utf-8") as handle:
                first = next(csv.DictReader(handle))
            self.assertTrue(first["token0_symbol"].startswith("'="))
            raw = json.loads((root / "v3_fee_manifest_raw.json").read_text(encoding="utf-8"))
            self.assertEqual(raw["rows"][0]["token0_symbol"], '=HYPERLINK("x")')

    def test_artifacts_require_two_human_signatures_and_hash_raw_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            summary = m.write_artifacts(root, self.sample_rows(), metadata())
            self.assertFalse(summary["writes_performed"])
            self.assertEqual(summary["human_signatures"], 0)
            self.assertEqual(summary["required_human_signatures_before_apply"], 2)
            self.assertIn("NOT YET SIGNED", (root / "SIGNOFF_REQUIRED.md").read_text(encoding="utf-8"))
            hashes = (root / "MANIFEST.sha256").read_text(encoding="utf-8")
            self.assertIn("v3_fee_manifest_raw.json", hashes)
            self.assertIn("repair_active_PREPARED_ROLLBACK.sql", hashes)


if __name__ == "__main__":
    unittest.main()
