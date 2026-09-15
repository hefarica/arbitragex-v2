# WO-V3-FEE: on-chain fee units, not an authorization change

Base: `b99c83491f4f35ed4b52497cfb63b8d04b83237a`. Follow-up of #570; no overlap with #571.

## Reproduction: observed, not synthetic production data

Desktop Commander read the public catalog at 2026-09-15T06:58:39.322Z.
All three XEN/WETH V3 pools were active in the PostgreSQL inventory:

| Pool | Catalog fee_tier | On-chain fee() |
|---|---:|---:|
| 0x26f35b980f3b791ac3f7c09ff152815c0dcb5bf3 | 5 | 500 |
| 0x7995430a85156b2d40d5bb701608788cf84019e3 | 30 | 3000 |
| 0x2a9d2ba41aba912316d16742f259412b681898db | 100 | 10000 |

Public API sample: opportunity `3759e7ca-c854-46d8-b07f-152fa0faaeaf`, trace
`fea47842-5c13-4e4d-8657-964ba5c546c0`, detected at 06:58:13.138Z, chain 1,
block 25981220, `v3_quote_unavailable`. The first pair of pools is the
3000/10000 pair in the table. Input: XEN to WETH, amountIn = 1000000000000000000 raw units.

Independent RPC: https://ethereum-rpc.publicnode.com (NOT the VPS provider).
Read methods only: eth_chainId, eth_getBlockByNumber, eth_getCode, eth_call.
Fixed block: 25981220 / 0x18c7124. Hash before and after:
`0x4a870b5ac0f4a8a41c6ffc7c7ae79c007f4b23d0f5b64cd6b42edf8766dd347a`.
QuoterV2: `0x61fFE014bA17989E743c5F6cB21bF9697530B21e`; code = 8273 bytes.

At that same block, direct quoteExactInputSingle calls with fee 5, 30 and
100 reverted (RPC code 3). Fee 3000 and 10000 returned complete 128-byte ABI
responses. Fee 500 still reverted with "Unexpected error"; the corresponding
pool's liquidity() returned zero. That is active in-range liquidity, NOT a
proof that every tick is empty. Neither a successful quote nor this sample
proves a profitable whole cycle, transaction inclusion or realized PnL.

The first six-quote PowerShell batch ended exit 1 AFTER receiving the six RPC
responses, because the final reporting object used bare `false` instead of
`$false`. The follow-up checked liquidity/slot0 and the unchanged block hash
at 07:01:52.2564649Z. Preserve that failed formatting attempt in the history.

## Cause and fix

Reactive discovery and enumeration source adapters normalized V3 pips to bps.
Hydration then wrote the normalized hint to pools.fee_tier and the legacy
Redis/PoolRef fee_bps fields. QuoterV2 and the graph expect raw V3 pips there.
The old Redis path also skipped an existing pool rather than correcting its fee.

Hydration now reads the pool's immutable fee() BEFORE token or pool writes.
The exact, validated uint24 pips are sent to PG, the V3 index, and ImpactIndex.
No source hint, constant 30/500 or magnitude heuristic substitutes for a failed
read. V2 uses its existing bps contract. No token authorization changes.

The ABI parser requires one canonical 32-byte uint24 word and fee < 1000000,
matching the V3 factory bound. A genuinely observed zero remains zero; absent
or malformed data is never zero. Nonstandard fee tiers are preserved exactly,
including fractional-bps tiers such as 150 pips. An observed 100 is NOT multiplied
into 10000. This clarifies the initial claim: rejecting every zero fee would
impose an extra restriction not present in the factory's bound.

Redis correction preserves other pools and unknown metadata, recognizes address
case differences and does not append a duplicate. Malformed cached JSON or a
failed GET is logged without replacing it with an empty list. A Lua CAS compares
the exact prior value before SET so a concurrent hydrator's update is not lost.
CAS conflicts are visible; they are not presented as successful cache updates.

## Verification and scope

- `cargo test --locked --manifest-path tests/v3-fee-units/Cargo.toml --lib`
  compiles the ACTUAL production module. Its 14 unit tests use declared fixtures.
  A dead-code warning for the Lua constant in this minimal harness is expected:
  the full production parent calls it and the Redis tests execute it.
- `ARBX_V3_FEE_DISPOSABLE_REDIS=1 python tests/v3-fee-units/test_redis_cas.py`
  executes eight cases against disposable Redis on 127.0.0.1:36379 ONLY.
  It never reads REDIS_URL or production credentials. Concurrent writers must
  have exactly one winner for the same previous value; only own test keys are deleted.
- The dedicated GitHub workflow runs both sets. Existing Rust CI still builds,
  lints and tests searcher-rs itself; the minimal module harness does not replace it.
- No main merge, production migration, deployment or runtime repair is implied
  by writing this document or by a green isolated test.

## Remaining historical repair: explicit, scoped, reviewed

Existing contaminated PG rows do NOT repair themselves when this code ships.
The enumeration worker may skip already-active inventory. Prepare a read-only
manifest for each (chain_id, pool address, row ID): previous stored fee, actual
factory/token0/token1/fee, source block/hash, and code identity. Include raw fee
100 and other ambiguous values; NEVER issue a blanket fee_tier * 100 update.

After review, a separate controlled repair can compare-and-set the exact old
value to the verified raw fee, record lineage, and rebuild/refresh the affected
Redis and in-process indexes. Check row counts and public quotes afterward.
No such data mutation is performed by this PR, the workflow, or the tests.

XEN remains outside the published allowlist. Its TokenNotAllowed is independent
of this fee correction. A.9, full-route no-cheat simulation (#567), paper accounting
and the multimode execution gateway (#570) remain separate open obligations.


## Review follow-up (same PR, before merge)

Base reviewed: 71bae985e6df02440e779cbd135081230e86ff51. The earlier green
CI did not catch five semantic issues. This follow-up addresses them together:

1. **Redis wire identity.** The actual consumer DTO is now shared:
   `reserves::V3PoolInfo { pool_addr, fee_bps }`. The updater wraps this exact
   type with flattened extra metadata instead of inventing `address`.
   A new test failed against the prior HEAD with `missing field address`
   (Cargo exit 101), before the correction. It now passes along with a
   producer/consumer serialization round trip. The V3 key also uses the
   consumer's canonical sorted-symbol key builder.
2. **Protocol fee units.** Internal V3 fees stay raw pips through calldata,
   RouteIntent, PG, Redis and graph. The V2 contract stays basis points.
   Previously the V3 calldata decoders divided by 100 while graph-produced
   RouteIntents did not, so dividing at the cartridge boundary alone was
   insufficient. V3 decoder assertions were updated against the same encoded
   bytes, not weakened: 500/3000/10000 must remain exact, and 150 may not become1.
   The Rhai boundary publishes explicit fee_pips and true fee_bps. The built-in
   V3 cartridge uses a raw-pips binding. Legacy bps bindings remain supported,
   including exact integral-pip fractional-bps values; overflow, non-finite,
   out-of-range and genuine sub-pip inputs fail instead of rounding/defaulting.
   Runtime and cartridge tests register the SAME math binding, not a test copy.
3. **Concurrent cache hydration.** The production updater rereads, merges and
   retries CAS up to four times. Read/malformed-index/uncertain-write failures
   and exhausted conflicts return errors rather than success. Pure tests call
   the actual updater. Two separate opt-in integration tests call that updater
   and its exact Lua against disposable loopback Redis, including two racing
   pool additions that must BOTH survive. They are not counted as run locally
   when the test Redis service is absent.
4. **Memory index.** add_pool replaces an existing (chain,address) in its
   immutable token pair, removes duplicate copies and preserves other pools,
   chains, ordering and cycle associations. It no longer treats rehydration as
   an additional venue.
5. **Resolution provenance.** Reactive discovery records a resolved pool only
   after successful hydration and memory-index publication. It records once
   per observed pair, preserving a successful sibling when another fails.
   Discovery logs say found_unhydrated until hydration has really finished.

The three hydration-order Python tests are explicitly STATIC call-order guards,
not claims of a database fault-injection or blockchain test. Full Rust CI must
also compile/test the parent searcher and the actual Rhai bridge; the small
fee-module harness does not replace it.

Remaining release/data obligations are unchanged: review and CI for the new
HEAD; protected merge/deploy; a reviewed chain/address/old-value/on-chain-factory,
fee/tokens/block-hash repair manifest for historical rows; reconcile PG/Redis
and the in-process index; then repeat quotes on the version actually served.
No blanket multiplication, token permission change, A.9 approval, signer, LIVE
flag or financial transaction is part of this change. A quote is not a proved
profitable cycle or realized income. #567 and #570 remain separate open work.

One additional read-only source-search command in this review session was
rejected by the tool security layer before execution. It was not retried via
an alternate path or used as runtime evidence. File edits and isolated test
commands that were independently authorized continued normally.
