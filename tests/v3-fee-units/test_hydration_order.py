"""Static ordering guards, not a database/on-chain integration test.

These pin production call order; runtime behavior is separately tested by Rust
and disposable Redis. No import executes production or reads environment secrets.

--------------------------------------------------------------------------------
FEE-UNITS-01 (2026-10-07) — WHY ONE GUARD WAS CHANGED, AND WHY THAT IS NOT A
WEAKENING
--------------------------------------------------------------------------------
`test_resolved_address_is_captured_before_publication_and_survives_failed_hydration`
stores POOL-RESOLVE-01 (PR #844). Two of its assertions pinned the DEFECT that
#844 fixes, and its split anchor stopped existing:

  · REMOVED `assertLess(publish, resolved)` — it required publication to happen
    BEFORE the address was captured. That order IS the bug: recording only on a
    SUCCESSFUL hydration discarded an address the factory had ALREADY answered.
    Measured cost of that order: 6.173 of the 6.203 pending observed pairs had
    `resolved_pool_addr IS NULL`, so the retry sweep had no address to hydrate.
    A guard that pins a bug is not a guard; it is the bug with a test attached.
  · REMOVED `assertNotIn("Some(e_pool),", body[:hydration])` — the same pinned
    order stated negatively: it forbade the capture from appearing before
    hydration, i.e. it forbade the fix itself.
  · RE-ANCHORED the split: the old anchor `let mut resolved_pool = None;` was
    removed by #844, so `SOURCE.split(...)[1]` raised IndexError and this test
    ERRORED instead of FAILING (a red that does not say what is wrong). Anchors
    now target `resolved_addr`.

WHAT THE GUARD ASSERTS NOW (the real intent, nothing weaker):
  1. hydration still precedes publication            (kept, unchanged);
  2. the captured address still precedes the write   (kept, re-anchored);
  3. capture precedes publication                    (the correction);
  4. the capture is UNCONDITIONAL: it sits BEFORE the `if let Ok(pool_ref)`
     success arm, so a failed hydration cannot skip it;
  5. the write is MONOTONIC: `is_resolved` only moves false→true and
     `resolved_pool_addr` is COALESCE'd, so a later failed observation can never
     null an address already captured — and the old unconditional `= $7` that
     erased known addresses is now explicitly forbidden.
The guard still fails if any of those five properties regresses.
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

    def test_resolved_address_is_captured_before_publication_and_survives_failed_hydration(self):
        # anchor re-targeted: `let mut resolved_pool = None;` was removed by #844.
        # The pre-check makes a future rename FAIL with a message instead of
        # ERRORING with `IndexError: list index out of range` (the shape this
        # guard had on #844's tree, which said nothing about what was wrong).
        anchor = "let mut resolved_addr: Option<Address> = None;"
        self.assertIn(anchor, SOURCE, f"anchor gone: {anchor!r} — update this guard deliberately")
        body = SOURCE.split(anchor, 1)[1]
        body = body.split("async fn record_observation(", 1)[0]

        success_arm = body.index("if let Ok(pool_ref)")
        hydration = body.index(".hydrate_and_persist_pool(")
        publish = body.index("idx.add_pool(pool_ref)")
        capture = body.index("resolved_addr = Some(e_pool)")
        record = body.index("self.record_observation(")

        # 1. KEPT: hydration happens before the successful publication
        self.assertLess(hydration, publish)
        # 2. KEPT (re-anchored): the captured address precedes the write
        self.assertLess(capture, record)
        # 3. THE CORRECTION: the address is captured BEFORE publication
        self.assertLess(capture, publish)
        # 4. the capture is UNCONDITIONAL — it sits before the success arm, so a
        #    failed hydration cannot skip it
        self.assertLess(capture, success_arm)
        # the write passes the CAPTURED address (regex: survives re-indentation)
        self.assertRegex(body, r"intent\.source_event,\s*\n\s*resolved_addr,")

        # 5. the write is MONOTONIC — a later failed observation can neither null
        #    an already-captured address nor un-set the resolution flag
        rec = SOURCE.split("async fn record_observation(", 1)[1].split("async fn ", 1)[0]
        self.assertIn(
            "is_resolved = observed_unindexed_pairs.is_resolved OR EXCLUDED.is_resolved",
            rec,
        )
        self.assertIn(
            "resolved_pool_addr = COALESCE(observed_unindexed_pairs.resolved_pool_addr,",
            rec,
        )
        # the unconditional overwrite that erased known addresses is forbidden —
        # asserted on the SQL LITERAL ONLY. Scanning `rec` whole would match the
        # explanatory comment printed above the statement, i.e. it would measure
        # prose instead of code and report a false regression (this guard did
        # exactly that on its first draft, and the red was the guard's fault).
        sql = rec.split('r#"', 1)[1].split('"#', 1)[0]
        self.assertNotIn("resolved_pool_addr = $7", sql)
        self.assertNotIn("is_resolved = $6", sql)

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
