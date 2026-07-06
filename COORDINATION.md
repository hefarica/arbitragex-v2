
## 2026-07-05 ~15:30 — Session "opportunities/omni-loop" (this Claude)
- **Worktree**: `.claude/worktrees/pr4-opportunities` on branch `feat/pr4-opportunity-enrichment` (off `excel-chain-builder` @ `2b242ee` = PR6 base `e9bcadd`).
- **Active workstream**: PR 4 — backend opportunity enrichment (buy/sell price, amount_out, gross/net/roi, fees, route legs).
- **Files I LOCK (do not touch on `feat/pr4-opportunity-enrichment`)**:
  - `database/migrations/100_opportunity_trade_math.sql` (NEW)
  - `backend/shared-rs/src/contracts.rs` (Opportunity struct)
  - `backend/searcher-rs/src/scanner.rs`, `backend/searcher-rs/src/persistence.rs`
  - `backend/api-server/src/routes/opportunities-live.ts`
  - `frontend/lib/store/types.ts` (in the pr4 worktree only)
- **Investigation workflow running**: `wf_b37c8349-1c8` (PR4 design). Implementation follows when it lands.
- **Verified (read-only) the excel-chain-builder RPC sync** from a parallel session: 145 RPC rows, 19 chains, all synced to Chain Builder. Sano — not touching the .xlsm/.bas (that session's file-ownership).
- **Hard gates respected**: no prod migration, no deploy, no push, no secrets, no live flip. Rust unverifiable locally (WDAC + WSL lacks libc-dev) → CI is authority.
- **PR 6 (e9bcadd) intact** on `excel-chain-builder`. PR 7 (hard filter <=0) + PR 3a (duplicity fix) committed there too.

## 2026-07-05 ~18:00 — Sync cycle (this Claude, opportunities/omni-loop)
- PR 4a COMMITTED + PUSHED: feat/pr4-opportunity-enrichment @ 53588ec → github. CI verifies Rust (migration 100 + struct + persistence). Trade-math contract layer (13 nullable cols). Null until PR 4b scanner wiring.
- EntropyEngine math VERIFIED: 15/15 vitest PASS (GL/Hurst/Shannon). Already wired into policy/engine.ts:117-123 (no injection needed).
- EntropyEngine lives on feat/fase-2-token-safety-scorer @ 19b3043 (token-safety session's worktree). I verified read-only; not touching it.
- Dispatching Agent Team (parallel, non-conflicting): PR 4b scanner wiring (pr4 worktree) + token_meta_unavailable blocker (read-only) + PR 5 price-attach design (read-only).
- LIVE FLIP stays operator-GO-gated (Gate 12, paper-first ≥7d, sim-mandatory, KMS, M5, mainnet refusal). Loop drives 100% paper + shadow + live-READINESS; halts at the flip for operator sign-off. Doctrinal, non-negotiable.

## 2026-07-05 ~18:15 — PR 5 design landed (this Claude)
- PR 5 (token real price attach) DESIGN COMPLETE (10-step plan, file:line verified):
  - NEW backend/api-server/src/simulation/livePriceSnapshot.ts (HGETALL arbx:token_prices:<chain>, 5s cache, mirrors tradingConfigSnapshot).
  - opportunities-live.ts: rowToOpportunity cascade (Redis-live → validation.price_usd → null) + 3 new wire fields (token_in/out_real_price_usd, price_source) + route handler threading.
  - NEW backend/api-server/src/routes/token-prices.ts (GET /api/v1/token-prices) + mount at index.ts:502.
  - frontend types.ts: OmniOpportunity +3 fields + mapToOmniOpportunity passthrough.
  - NEW frontend/lib/hooks/useTokenPrice.ts (15s poll).
  - R8: NEVER the $1 stablecoin default (ConfigPriceOracle price_oracle.rs:99-104 is spine-internal); null when uncovered.
- APPLICATION: queued until PR 4b agent lands (avoid concurrent commits in pr4 worktree). Apply in feat/pr4-opportunity-enrichment next.
- Agents still running: PR 4b scanner wiring, token_meta_unavailable blocker, ChatDAPP.txt digest.

## 2026-07-05 ~18:25 — VERIFICATION cycle (this Claude) — "VERIFICA TODO eso del chat"
All ChatDAPP digest claims verified against ground truth (systematic-debugging Phase 1):
- FASE 2 + EntropyEngine vitest: 87/87 PASS (selector-api, ran) ✅
- FASE 4 MakerDaoDssFlashAdapter forge: 7/7 PASS (contracts, ran) ✅
- PR 4b scanner: 5 fields populated (scanner.rs:1440/1690/1691/2285/2290) ✅
- EntropyEngine math: GL `(k-1-α)/k` (l.14) + Hurst `sqrt(n)` (l.21/96/102) + mulberry32(42) (l.49/138/151) ✅
- All cited commits exist (716e2a7/3a54a9d/4e551f2/e87d90f/19b3043/f0fc6f2/e9bcadd/53588ec/a8c4c40/dbf0d57) ✅
- Mig collision RESOLVED: 100_seed_liquidity_primitives (theirs) + 101_opportunity_trade_math (mine, renamed dbf0d57) ✅
DISCREPANCY CAUGHT: digest said "FASE 3 NOT committed" but `select_provider_from_registry` EXISTS + is real (flashloan_engine.rs:311, PG query, fail-honest) → FASE 3 registry ranker IS committed. Digest understated.

## 🔑 KEY OPERATIONAL FINDING — token_meta_unavailable root cause (VERIFIED)
The #1 prod blocker (10.477 dex_arb die/h). Root cause CONFIRMED:
- Redis cache `arbx:tokens:<chain>:<addr>` has ONE writer: `pool_sync_worker.rs:bootstrap_token_cache` (:923-970), called ONCE at boot (:438). NEVER refreshed in the loop.
- token-enricher writes ONLY to PG, never Redis. So tokens enriched after pool_sync_worker boot are invisible → all opps die.
- BUG: `pool_sync_worker.rs:932 AND :1015` read `Option<i32>` for decimals, but mig 098 made it SMALLINT (i16) → inconsistency (could break bootstrap on strict decode).
FIX PATH: (1) short-term: restart pool_sync_worker after enrichment re-bootstraps Redis; (2) durable: periodic re-bootstrap in the loop (~40 LOC) + i32→i16 fix (2 spots) + debug!→warn! for skipped tokens.
