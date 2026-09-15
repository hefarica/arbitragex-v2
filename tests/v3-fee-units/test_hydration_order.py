"""Static ordering guards, not a database/on-chain integration test.

These pin production call order; runtime behavior is separately tested by Rust
and disposable Redis. No import executes production or reads environment secrets.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / "backend/searcher-rs/src/pool_discovery.rs").read_text(encoding="utf-8")

class HydrationOrderTests(unittest.TestCase):
    def test_fee_observation_precedes_token_and_pool_writes(self):
        body = SOURCE.split("async fn hydrate_and_persist_pool(", 1)[1].split("async fn read_pool_v3_fee(", 1)[0]
        read = body.index("self.read_pool_v3_fee(")
        self.assertLess(read, body.index(".upsert_token_in_db("))
        self.assertLess(read, body.index("self.upsert_pool_in_db("))
        self.assertNotIn("record_observation(", body)

    def test_resolved_observation_follows_successful_index_publication(self):
        body = SOURCE.split("let mut resolved_pool = None;", 1)[1].split("async fn record_observation(", 1)[0]
        success = body.index("if let Ok(pool_ref)")
        hydration = body.index(".hydrate_and_persist_pool(")
        publish = body.index("idx.add_pool(pool_ref)")
        resolved = body.index("resolved_pool = Some(e_pool)")
        record = body.index("self.record_observation(")
        self.assertLess(success, hydration)
        self.assertLess(hydration, publish)
        self.assertLess(publish, resolved)
        self.assertLess(resolved, record)
        self.assertIn("intent.source_event,\n                    resolved_pool,", body)
        self.assertNotIn("Some(e_pool),", body[:hydration])

    def test_v3_index_uses_consumer_key_and_propagates_exhaustion(self):
        body = SOURCE.split("let key = crate::reserves::key_pool_index_v3(", 1)[1]
        body = body.split("Ok(PoolRef", 1)[0]
        self.assertIn("v3_fee::publish_v3_index(", body)
        self.assertIn(".map_err(anyhow::Error::msg)?", body)
        self.assertNotIn("unwrap_or_default()", body)


    def test_v3_bootstrap_writer_uses_shared_cas_not_unconditional_set(self):
        source = (ROOT / "backend/searcher-rs/src/reserves.rs").read_text(encoding="utf-8")
        body = source.split("pub async fn set_pool_index_v3(", 1)[1].split("pub async fn get_pools_for_pair_v3(", 1)[0]
        self.assertNotIn(".set(", body)
        self.assertIn("publish_v3_bootstrap(", body)
        self.assertIn("INDEX_COMPARE_AND_SET", body)

if __name__ == "__main__":
    unittest.main(verbosity=2)
