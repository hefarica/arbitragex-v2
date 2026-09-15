# V3 Fee Data Integrity — read-only manifest and repair gate

This runbook is the first gate after PR #572. It does **not** authorize a
PostgreSQL update, Redis mutation, service restart, A.9 sign-off, or LIVE mode.

## Canonical schema

The production registry uses `pools`, not a separate `pools_v3` table:
`pools(id, chain_id, factory_id, address, token0_id, token1_id, fee_tier, is_active)`.
`fee_tier` for V3 is consumed as raw fee pips by the Quoter path.

PR #572 prevents new V3 hydration from trusting bps-like source hints: it reads
immutable `fee()` on-chain before writing PG/Redis/PoolRef. Historical rows can
still be wrong or null and therefore require an independent repair manifest.

## Generate a manifest

Use a read-only RPC. Never place a private RPC credential in a committed file:

```bash
python scripts/data_integrity/v3_fee_manifest.py \
  --rpc-url https://ethereum-rpc.publicnode.com \
  --expected-deploy-sha <FULL_SERVED_SHA> \
  --output-dir ./artifacts/v3-fee-manifest
```
The tool first confirms the served deploy SHA and RPC chain, pins one block,
then discovers every `UNISWAP_V3` DEX/pool through the PostgreSQL-backed
liquidity catalog. At the same block it calls, per pool:

- `fee()`
- `factory()`
- `token0()`
- `token1()`

A fee difference is not accepted as repair evidence when factory/token identity
does not match. RPC failures and identity mismatches make the command non-zero.

The output contains:

- full verification CSV;
- all and active-only repair candidate CSVs;
- SQL review plans ending in mandatory `ROLLBACK`;
- summary JSON;
- SHA-256 manifest;
- unsigned two-reviewer sign-off template.

The generator has no database or Redis write capability. This is deliberate.

## Absolute repair gate

Do **not** apply a candidate until all of the following exist:

1. two independent human reviews of the same manifest hash;
2. PostgreSQL backup with a demonstrated restore check;
3. a fresh re-run against the then-current DB rows and an on-chain pinned block;
4. the exact old PG value still matches the manifest (optimistic concurrency guard);
5. a reviewed PG → Redis `arbx:pool_index_v3:*` → in-process index reconciliation plan;
6. post-apply re-verification from both PG and on-chain getters.

Never run a global `fee_tier = fee_tier * 100` repair. Raw on-chain tiers include
legitimate values such as 100 and non-3000 tiers, while historical rows may be
NULL. Magnitude alone cannot identify the correct value.

Do not invent a Redis key such as `pool:<chain>:<pool>:fee`. The current wire
contract is `arbx:pool_index_v3:<chain>:<sym0>:<sym1>` with canonical
`V3PoolInfo`. Reconciliation must use the producer contract landed in #572.

## Evidence captured on 2026-09-15

For served deploy `a575a4e4e6b9a1cd876752f32b755ccc6f081b45`, a read-only
verification of Ethereum chain 1 observed 318 V3 pools. All 318 matched their
catalog factory/token0/token1 identity and all getter calls succeeded.

Observed classifications were 34 MATCH, 80 FEE_MISMATCH and 204 MISSING_FEE.
There were 90 active repair candidates. Mismatch pairs were 1→100 (8),
5→500 (15), 30→3000 (25), and 100→10000 (32). On-chain tiers also included
2500, proving a universal multiplier would be unsafe.

This observation is historical evidence, not authorization to mutate the live
registry. Re-run immediately before any approved repair.
