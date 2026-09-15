import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts" / "data_integrity" / "v3_fee_manifest.py"
spec = importlib.util.spec_from_file_location("v3_fee_manifest", MODULE_PATH)
assert spec and spec.loader
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class V3FeeManifestTests(unittest.TestCase):
    def test_decode_u256_and_address_are_strict(self):
        self.assertEqual(m.decode_u256("0x01f4"), 500)
        self.assertIsNone(m.decode_u256("not-hex"))
        encoded = "0x" + "00" * 12 + "1f98431c8ad98523631ae4a59f267346ea31f984"
        self.assertEqual(m.decode_address(encoded), "0x1f98431c8ad98523631ae4a59f267346ea31f984")
        self.assertIsNone(m.decode_address("0x1234"))

    def test_classification_never_turns_missing_identity_into_match(self):
        self.assertEqual(m.classify(None, 3000, True), "MISSING_FEE")
        self.assertEqual(m.classify(30, 3000, True), "FEE_MISMATCH")
        self.assertEqual(m.classify(3000, 3000, True), "MATCH")
        self.assertEqual(m.classify(3000, 3000, False), "IDENTITY_MISMATCH")
        self.assertEqual(m.classify(3000, None, True), "ONCHAIN_ERROR")

    def test_catalog_paginates_and_checks_declared_count(self):
        responses = {
            "level=dexes": {"items": [
                {"id": "dex-v3", "label": "V3", "protocol_type": "UNISWAP_V3", "pool_count": "2"},
                {"id": "dex-v2", "label": "V2", "protocol_type": "UNISWAP_V2", "pool_count": "9"},
            ]},
            "after=one": {"items": [{"id": "two", "pool_address": "0x" + "22" * 20}], "next_after": None},
            "dex_id=dex-v3": {"items": [{"id": "one", "pool_address": "0x" + "11" * 20}], "next_after": "one"},
        }

        def fake(url):
            for needle, payload in responses.items():
                if needle in url:
                    return payload
            raise AssertionError(url)

        dexes, pools = m.collect_v3_catalog("https://catalog.invalid", 1, fake)
        self.assertEqual([d["id"] for d in dexes], ["dex-v3"])
        self.assertEqual([p["id"] for p in pools], ["one", "two"])

    def test_catalog_fails_if_page_count_disagrees_with_dex_summary(self):
        def fake(url):
            if "level=dexes" in url:
                return {"items": [{"id": "v3", "label": "V3", "protocol_type": "UNISWAP_V3", "pool_count": "2"}]}
            return {"items": [{"id": "only", "pool_address": "0x" + "11" * 20}], "next_after": None}

        with self.assertRaisesRegex(RuntimeError, "catalog_count_mismatch"):
            m.collect_v3_catalog("https://catalog.invalid", 1, fake)

    @staticmethod
    def sample_rows():
        base = {
            "pool_id": "00000000-0000-4000-8000-000000000001", "chain_id": "1",
            "dex_name": "UniswapV3", "protocol_type": "UNISWAP_V3",
            "pool_address": "0x" + "11" * 20, "active": True,
            "catalog_factory": "0x" + "aa" * 20, "onchain_factory": "0x" + "aa" * 20,
            "catalog_token0": "0x" + "bb" * 20, "onchain_token0": "0x" + "bb" * 20,
            "token0_symbol": "AAA", "catalog_token1": "0x" + "cc" * 20,
            "onchain_token1": "0x" + "cc" * 20, "token1_symbol": "BBB",
            "identity_ok": True, "block_number": 123, "block_hash": "0x" + "dd" * 32,
            "rpc_host": "rpc.invalid", "rpc_error": "",
        }
        mismatch = dict(base, catalog_fee=30, onchain_fee=3000, classification="FEE_MISMATCH")
        missing = dict(base, pool_id="00000000-0000-4000-8000-000000000002",
                       pool_address="0x" + "12" * 20, active=False,
                       catalog_fee=None, onchain_fee=500, classification="MISSING_FEE")
        match = dict(base, pool_id="00000000-0000-4000-8000-000000000003",
                     pool_address="0x" + "13" * 20, catalog_fee=100, onchain_fee=100,
                     classification="MATCH")
        return [mismatch, missing, match]

    def test_generated_sql_is_guarded_and_rollback_only(self):
        sql = m.build_rollback_sql(self.sample_rows(), active_only=True)
        self.assertIn("fee_tier IS NOT DISTINCT FROM 30", sql)
        self.assertIn("lower(address)", sql)
        self.assertNotIn("fee_tier * 100", sql)
        self.assertNotIn("00000000-0000-4000-8000-000000000002", sql)
        self.assertTrue(sql.rstrip().endswith("ROLLBACK; -- mandatory default"))

    def test_artifacts_make_signoff_explicit_but_do_not_fake_it(self):
        metadata = {
            "schema_version": 1, "phase": "test", "chain_id": 1,
            "source_deploy_sha": "a" * 40, "source_deploy_id": "run",
            "catalog_source": "postgresql-registry", "v3_dexes": ["UniswapV3"],
            "block_number": 123, "block_hash": "0x" + "dd" * 32,
            "rpc_host": "rpc.invalid",
        }
        with tempfile.TemporaryDirectory() as tmp:
            summary = m.write_artifacts(Path(tmp), self.sample_rows(), metadata)
            self.assertFalse(summary["writes_performed"])
            self.assertEqual(summary["human_signatures"], 0)
            self.assertEqual(summary["required_human_signatures_before_apply"], 2)
            signoff = (Path(tmp) / "SIGNOFF_REQUIRED.md").read_text(encoding="utf-8")
            self.assertIn("NOT YET SIGNED", signoff)
            hashes = (Path(tmp) / "MANIFEST.sha256").read_text(encoding="utf-8")
            self.assertIn("v3_fee_manifest_full.csv", hashes)
            self.assertIn("repair_active_PREPARED_ROLLBACK.sql", hashes)

    def test_all_plan_includes_null_guard_for_missing_fee(self):
        sql = m.build_rollback_sql(self.sample_rows(), active_only=False)
        self.assertIn("fee_tier IS NOT DISTINCT FROM NULL", sql)
        self.assertIn("00000000-0000-4000-8000-000000000002", sql)


if __name__ == "__main__":
    unittest.main()
