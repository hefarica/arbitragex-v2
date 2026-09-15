# V3 Fee Data Integrity — read-only manifest and repair gate

This runbook is the first data-integrity gate after PR #572. It does **not**
authorize a PostgreSQL update, Redis mutation, service restart, A.9 sign-off,
or LIVE mode.

## Canonical schema

Production uses `pools`, not a separate `pools_v3` table:
`pools(id, chain_id, factory_id, address, token0_id, token1_id, fee_tier, is_active)`.
V3 `fee_tier` is consumed as raw fee pips by the Quoter path.

PR #572 prevents new V3 hydration from trusting bps-like source hints: it reads
immutable `fee()` on-chain before publishing PG/Redis/PoolRef. Historical rows
can still be wrong or null and therefore require an independent repair manifest.

## Generate a manifest

Use a read-only RPC. Never place a private RPC credential in a committed file:

```bash
python scripts/data_integrity/v3_fee_manifest.py \
  --rpc-url https://ethereum-rpc.publicnode.com \
  --expected-deploy-sha <FULL_40_HEX_SERVED_SHA> \
  --output-dir ./artifacts/v3-fee-manifest-<UNIQUE_RUN_ID>
```
`--expected-deploy-sha` is mandatory. The output directory must not already
exist; the CLI claims it atomically so concurrent runs cannot interleave files.

The tool validates the served deploy identity with distinct cache-busted status reads,
requires a positive RPC batch size, validates RPC chain and the catalog envelope
(`schema_version`, `source`, `level`, `scope`). It collects the entire V3 census,
pins a block whose returned number must match the requested height, and calls per pool:

- `fee()`
- `factory()`
- `token0()`
- `token1()`
- `factory.getPool(token0, token1, fee)`

ABI uint words must be exactly 64 hexadecimal digits. Every `eth_call` is bound
to that block hash with EIP-1898
`requireCanonical=true`. After on-chain verification, the complete catalog is
read again with a different cache-buster; both canonical catalog digests must
match. The numbered block and served deploy identity are rechecked before any
artifact is accepted.

RPC, ABI, provenance, catalog-drift, identity, factory-mapping, reorg or
`ONCHAIN_ERROR` conditions make the command non-zero. An empty V3 census also
fails closed.

The output contains spreadsheet-safe CSVs, raw non-executable JSON evidence,
ROLLBACK-only SQL review plans, `summary.json`, `MANIFEST.sha256`, and an
unsigned two-reviewer sign-off template.
The generated SQL guards the old fee, row identity, chain, address, factory,
token relationships and active state. It still ends in mandatory `ROLLBACK`.
The generator has no PostgreSQL or Redis mutation client and no APPLY path.

## Absolute repair gate

Do **not** apply a candidate until all of the following exist:

1. two independent human reviews of the same `MANIFEST.sha256`;
2. PostgreSQL backup plus a demonstrated restore check;
3. a fresh manifest against the then-current served SHA and pinned block;
4. the exact old row identity/state still matches the manifest guards;
5. a reviewed PG → `arbx:pool_index_v3:*` → in-process reconciliation plan;
6. post-apply re-verification from PG and immutable on-chain getters.

Never run a global `fee_tier = fee_tier * 100` repair. Raw V3 tiers include
legitimate values such as 100, 500, 2500, 3000 and 10000, while historical
rows may be NULL. Magnitude alone cannot identify the correct value.

Do not invent a Redis key such as `pool:<chain>:<pool>:fee`. The current wire
contract is `arbx:pool_index_v3:<chain>:<sym0>:<sym1>` with canonical
`V3PoolInfo`; reconciliation must use the producer contract landed in #572.

## Evidence captured on 2026-09-15
For served deploy `2cdbce35425ad74340d354afac661bf443bac46e` (deploy run
`35029159263`), the hardened verifier checked Ethereum block `25986250`, hash
`0xbf2e5b25a394f726fb5a347bee16a9736176900d62720891f31e55200e72798d`.
Both complete catalog censuses had digest
`e41b02b48ecbf87ad12055faf2a89a13d71a61ae0899705832800f5e29c6ace4`.

All 318 V3 pools passed catalog provenance, strict ABI decoding,
factory/token identity and canonical factory mapping, with zero RPC,
verification, identity or on-chain errors. The classification remained:
34 MATCH, 80 FEE_MISMATCH and 204 MISSING_FEE, with 90 active repair
candidates.

Mismatch pairs remain 1→100 (8), 5→500 (15), 30→3000 (25), and
100→10000 (32). On-chain tiers also include 2500, proving a universal
multiplier would be unsafe.

This is read-only historical evidence, not authorization to mutate the live
registry. Re-run immediately before any separately approved repair.