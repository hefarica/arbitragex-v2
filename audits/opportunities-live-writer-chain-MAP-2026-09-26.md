# SERVER-SIDE WRITER CHAIN — `/api/opportunities/live` payload items

**Scope:** read-only audit of `backend/api-server/src/routes/opportunities-live.ts` (1086 lines, read in full),
the PG/Redis producers that feed it, and every competing endpoint in the repo.
**Repo:** `C:\Users\HFRC\Desktop\arbitragex-v2-main (17)`
**Date:** 2026-09-26. Nothing was edited.

**Headline findings**

1. `leg_symbols` is `null` on 41/41 live items because the payload's **only** leg source is
   `opportunities.route_metadata->'token_addresses'` (`opportunities-live.ts:643-659`), and the
   production rows are written by the **api-server bridge archiver**, whose INSERT does not list
   `route_metadata` at all (`opportunities-bridge-archiver.ts:252-259`) → column DEFAULT `'{}'::jsonb`
   → `route_metadata` is coerced to wire `null` (`opportunities-live.ts:701-706`) → `leg_symbols: null`.
2. There is **no** separate producer writing `simulated_target`, `simulated_cost_breakdown`,
   `simulated_at` or `simulated_notes` **as PG columns**: these names are not columns in any table
   (grep of `*.sql` = 0 matches). They are computed **per request in TS** from the Redis
   `arbx:trading_config:<chain>` snapshot.
3. `token_prices_usd` **is** a PG column — but of `trading_config` (a `JSONB` operator-prices map,
   migration 032), NOT of `opportunities`, and it is NOT the source of the wire field. The wire field
   comes from the Redis hash `arbx:token_prices:<chain>` (`opportunities-live.ts:996-1026`).
4. The api-server does **not** serve `/api/opportunities/live`. It mounts only
   `/api/v1/opportunities/live` (`opportunities-live.ts:741`, called at `index.ts:555`). The pubic path
   exists only in the edge worker (`edge/worker/src/index.ts:699`) and edge dev-local
   (`edge/dev-local/src/index.ts:338`). That is the whole mount split.

---

## (a) LIVE ROUTE FIELD → SOURCE TABLE

Route: `GET /api/v1/opportunities/live`. Handler `mountOpportunitiesLive`
(`backend/api-server/src/routes/opportunities-live.ts:735-1083`), registered at
`opportunities-live.ts:741`:

```ts
741:  app.get("/api/v1/opportunities/live", async (req: Request, res: Response) => {
```

Called from `backend/api-server/src/index.ts:555`:

```ts
555: mountOpportunitiesLive(app, pool, redis, logger);
```

### a.1 Envelope

| field | file:line | PG column / computation | null conditions |
|---|---|---|---|
| `count` | `opportunities-live.ts:1029` | `q.rows.length` (grouped rows returned) | never null (0 when empty) |
| `window_total` | `opportunities-live.ts:1034`, SQL `:363` | SQL `(COUNT(*) OVER ())::int` — **counts grouped rows**, i.e. distinct route identities in the window | `0` when no rows (never null, R8) |
| `window` | `opportunities-live.ts:1035` | literal `"latest"` | — |
| `viable_only` | `opportunities-live.ts:1036`, parsed `:760-761` | query param, default `"false"` | — |
| `max_age_seconds` | `opportunities-live.ts:1037`, clamped `:771-774` | query param `max_age_seconds`, default 300, clamped `[10, 86400]` | — |
| `ts` | `opportunities-live.ts:1073` | `new Date().toISOString()` | — |

Request knobs: `limit` clamped `[1,200]` default 50 (`:747`); `viable_only` (`:760-761`);
`max_age_seconds` (`:771-774`); `order` ∈ {`detected_at` (default), `profit_usd`} (`:778-781`).
Fail-honest gates: `503 {error:"db_unavailable"}` when `pool` null (`:742-745`),
`503 {error:"query_failed"}` on PG error (`:1075-1080`).

### a.2 The SQL query, its CTE and its GROUP BY

`LIVE_QUERY` is defined at `opportunities-live.ts:285-388`.

**Exact `route_group_key` expression** — `opportunities-live.ts:288-296`:

```sql
288:    concat_ws('|',
289:      o.chain_id::text,
290:      COALESCE(o.chain_id_out::text, ''),
291:      COALESCE(o.strategy_kind, ''),
292:      o.token_in,
293:      o.token_out,
294:      o.dex_a,
295:      COALESCE(o.dex_b, '')
296:    ) AS route_group_key,
```

`GROUP BY 1` (the expression above) — `opportunities-live.ts:311`.
Aggregates: `MIN(o.detected_at) AS first_seen_at` (`:297`), `MAX(o.detected_at) AS last_seen_at`
(`:298`), `COUNT(*)::int AS confirmations` (`:299`), latest row
`(ARRAY_AGG(o.id ORDER BY o.detected_at DESC, o.id DESC))[1] AS latest_id` (`:303`).
Window + viability filter live **inside** the CTE (`:307-310`):

```sql
307:  WHERE o.detected_at >= NOW() - ($3::int * INTERVAL '1 second')
308:    AND ($2::bool = false
309:         OR (o.status = ANY($4::text[])
310:             AND o.rejection_reason IS NULL))
```

`$4` = `VIABLE_STATUSES = ["detected","validated","simulated","scored"]`
(`opportunities-live.ts:37`; Rust mirror `backend/searcher-rs/src/persistence.rs:32`) — but **only used
when `viable_only=true`**; the default path (`$2=false`) returns rejected rows too.

Outer select `FROM grouped g JOIN opportunities o ON o.id = g.latest_id` (`:369-371`),
`LEFT JOIN tokens ti` on `ti.chain_id = o.chain_id AND ti.address = LOWER(o.token_in)` (`:372-374`),
`LEFT JOIN tokens to_` on `to_.chain_id = COALESCE(o.chain_id_out, o.chain_id) AND to_.address = LOWER(o.token_out)` (`:375-377`),
ordering CASE on `order=profit_usd` over `-(COALESCE(o.net_expected_profit_usd, o.expected_profit_usd))`
(`:383-386`), `LIMIT $1` (`:387`). Bound at `:784-790`.

**COALESCE order vs `frontend/lib/store/route-key.ts`** — `frontend/lib/store/route-key.ts:21-31`:

```ts
22:  return [
23:    opp.chain_id,
24:    opp.chain_id_out ?? "",
25:    opp.strategy_kind ?? "",
26:    opp.token_in,
27:    opp.token_out,
28:    opp.dex_a,
29:    opp.dex_b ?? "",
30:  ].join("|");
```

**VERDICT: the COALESCE order MATCHES** — same 7 segments, same order
(`chain_id`, `chain_id_out`, `strategy_kind`, `token_in`, `token_out`, `dex_a`, `dex_b`), same
null→`''` treatment. The SQL `concat_ws` skips NULLs (hence the COALESCEs), JS `join` renders
`null/undefined` as `''`. Byte-identical for the current data (all 7 columns are `text`/`integer`;
`chain_id_out` is `integer` in the deployed schema — `audits/schema-drift-2026-09-20/deployed_schema.tsv:385`).
The header comment at `opportunities-live.ts:274-280` and `route-key.ts:8-19` both pin this as a hard
condition. Residual risk (NOT VERIFIED as live): JS `null ?? ""` handles only `null|undefined`; a
`chain_id_out` delivered as another falsy-but-not-null value would diverge — none is possible given
the column's declared type `integer`.

### a.3 The repository / mapping function that turns a PG row into the JSON item

`rowToOpportunity(row, sim, chainBaseTokenSymbol, validations, legSymbols)` —
**`opportunities-live.ts:583-718`**. Called per row at `:1038-1045`. Declared row type
`OpportunityLiveRow` at `:187-260`. Also exported for tests as `__forTesting.rowToOpportunity`
(`:1086`). Helpers: `tokenInfoFromRow` (`:427-495`), `normalizeBlockNumber` (`:513-517`),
`paperStatusFromRow` (`:532-535`), `chainsUsedFromRow` (`:542-548`), `dexesUsedFromRow` (`:554-567`).

### a.4 Field-by-field

| field | file:line | PG column / computation | null conditions |
|---|---|---|---|
| `id` | `:624` ← SQL `:318` | `opportunities.id` (uuid) | never null |
| `chain_id` | `:625` ← SQL `:319` | `opportunities.chain_id` | never null |
| `chain_base_token_symbol` | `:630` ← `:1042` | **not a column** — `snapshots.get(r.chain_id)?.base_token_symbol` from Redis `arbx:trading_config:<chain_id>` (`simulation/tradingConfigSnapshot.ts:164`, key `:83-85`, read `:218`) | `null` when snapshot absent (Redis miss / malformed JSON) |
| `strategy_kind` | `:631` ← SQL `:320` | `opportunities.strategy_kind` | never null |
| `cartridge_id` | `:632` ← SQL `:344` | `opportunities.cartridge_id` | null on legacy rows (migration 121 era) |
| `dex_a` | `:633` ← SQL `:321` | `opportunities.dex_a` | never null |
| `dex_b` | `:634` ← SQL `:322` | `opportunities.dex_b` | nullable column |
| `pair_symbol` | `:635` ← SQL `:323` | `opportunities.pair_symbol` | nullable column |
| `token_in` | `:636` ← SQL `:324` | `opportunities.token_in` | never null |
| `token_in_info` | `:637` → `tokenInfoFromRow(...,"token_in",...)` `:427-495` | base from LEFT JOIN `tokens` (`token_in_symbol/decimals/logo_url/resolved_via`, SQL `:325-328`) **then post-query overwrite** (`:817-825`); `verified`/`registry_*`/`verified_notes` from `verifyToken(chainId,address,symbol)` (`:445`, `services/tokenRegistry.js`); `validation` from `readTokenValidationsBatch` (`:989`) keyed `${chain}:${addr.toLowerCase()}` (`:596-598`) | **always an object, never null** (`:416-421`); inner `symbol`/`decimals`/`logo_url` null when DB JOIN missed AND on-demand RPC missed; `validation: null` when the validation worker has not run yet (`:470-482`) |
| `token_out` | `:638` ← SQL `:329` | `opportunities.token_out` | never null |
| `token_out_info` | `:639` → same fn | JOIN `tokens to_` (`:330-333`) + overwrite `:826-834`; `chainId = row.chain_id_out ?? row.chain_id` (`:437-439`) | as above |
| `leg_symbols` | **`:643-659`** | `opportunities.route_metadata->'token_addresses'` ∩ the batch-built `legSymbols` map (`:849-897`) | see §(c) — **null on 41/41 live items** |
| `amount_in_wei` | `:660` ← SQL `:334` | `o.amount_in_wei::text` (numeric → string) | never null (bridge rows write literal `0`) |
| `expected_profit_usd` | `:663` ← SQL `:335` | `o.expected_profit_usd::float` — **GROSS** (pre gas/slippage/relay) | nullable column (migration 033 dropped NOT NULL) |
| `net_expected_profit_usd` | `:664` ← SQL `:336` | `o.net_expected_profit_usd::float` (migration 049) — **NET** | null when the Rust spine has not evaluated the row (e.g. bridge rows) |
| `roi_pct` | `:665` ← SQL `:337` | `o.roi_pct::float` | nullable column |
| `risk_score` | `:666` ← SQL `:338` | `o.risk_score::float` | nullable column |
| `rejection_reason` | `:667` ← SQL `:339` | `o.rejection_reason` | null = not rejected (R8 semantics) |
| `paper_status` | `:669` → `paperStatusFromRow` `:532-535` | **DERIVED, not a column**: `row.rejection_reason != null \|\| row.status === "rejected" \|\| row.status === "failed"` → `"paper_rejected"` else `"paper_viable"` | never null |
| `chains_used` | `:670` → `:542-548` | derived `Set([chain_id])` + `chain_id_out` if different | never null |
| `dexes_used` | `:671` → `:554-567` | derived from `dex_a`, `dex_b`, plus `route_metadata.dex_adapters[]` | never null (possibly `[]`) |
| `block_number` | `:672` ← SQL `:340` | `o.block_number` (BIGINT→string at runtime) normalized by `normalizeBlockNumber` `:513-517` | null when NULL or not a safe integer |
| `status` | `:673` ← SQL `:341` | `opportunities.status` (CHECK enum incl. `rejected`) | never null |
| `detected_at` | `:674-676` ← SQL `:342` | `o.detected_at` (TIMESTAMPTZ → Date → `.toISOString()`) | never null |
| `trace_id` | `:677` ← SQL `:343` | `opportunities.trace_id` | never null |
| `first_seen_at` | `:683-685` ← SQL `:314` | `MIN(o.detected_at)` per route group (CTE `:297`) | never null |
| `last_seen_at` | `:686-688` ← SQL `:315` | `MAX(o.detected_at)` per route group (CTE `:298`) | never null |
| `confirmations` | `:689` ← SQL `:316` | `COUNT(*)::int` per route group (CTE `:299`), ≥1 by construction | never null |
| `route_group_key` | `:690` ← SQL `:317` | the `concat_ws` expression above | never null |
| `detector_id` | `:693` ← SQL `:347` | `o.detector_id` (migration 121) | null on legacy / bridge rows |
| `pipeline_latency_ms` | `:694` ← SQL `:348` | `o.pipeline_latency_ms` (BIGINT) via `normalizeBlockNumber` (`:513-517`) | null when NULL or non-safe-integer |
| `chain_id_out` | `:695` ← SQL `:349` | `o.chain_id_out` | null for same-chain routes |
| `bridge` | `:696` ← SQL `:350` | `o.bridge` | null |
| `bridge_fee_usd` | `:697` ← SQL `:351` | `o.bridge_fee_usd::float` | null |
| `route_metadata` | `:701-706` ← SQL `:352` | `o.route_metadata` (JSONB, NOT NULL DEFAULT `'{}'`) — **empty object coerced to `null`** | `null` when absent/`{}`/non-object |
| `simulated_net_profit_usd` | `:606`, emitted `:710` | `sim?.forward?.net_usd` from `forwardSimulate()` (`simulation/computeSimulatedNet.ts:222`, `net_usd` `:65`) | null when snapshot absent (`:939 continue`), `forwardSimulate` returned null, or after the `forward \|\| inverse` gate (`:966-968`) |
| `simulated_amount_in_usd` | `:607`, emitted `:711` | `sim.forward.amount_in_usd` (`computeSimulatedNet.ts:63`) | as above |
| `simulated_roi_pct` | `:608`, emitted `:712` | `sim.forward.roi_pct` (`computeSimulatedNet.ts:66`) | as above |
| `simulated_cost_breakdown` | `:609-610`, emitted `:713` | `sim.forward.cost_breakdown` — **computed TS object, no PG column**; shape `SimulatedCostBreakdown` `computeSimulatedNet.ts:50-60` (gas/lp_fees/slippage/failure_buffer/copied_buffer/capital_cost/ops_overhead/flashloan_fee/relay_fee) | object or `null` |
| `simulated_target` | `:611-612`, emitted `:714` | `sim.inverse` = `inverseSize(simRow, snapshot, target, forward)` (`:961-963`), target from `resolveTarget(snapshot, strategy_kind)` (`:950`; fn `computeSimulatedNet.ts:332`; source `strategy_config` \| `simulation_tab`) — **computed TS object, no PG column** | null when no target configured, or `forwardSimulate` returned null and no ROI floor (Path B unavailable) |
| `simulated_at` | `:616-617`, emitted `:715` | `sim.simulated_at` = `new Date().toISOString()` captured once per request at `:930` | null when neither forward nor inverse produced output (`:616-617`) |
| `simulated_notes` | `:618-621`, emitted `:716` | concat of `sim.forward.notes` + `sim.inverse.notes` (`computeSimulatedNet.ts:69`, `:133`; notes pushed at `:248,263,279,482,514,521,539,582`) | `null` when the concatenated array is empty (`:716`) |
| `token_prices_usd` | `:1050`, `:1063-1071` | Redis hash `arbx:token_prices:<chain_id>` → `priceMaps` (`:996-1026`); keys = upper(SYMBOL) of `token_in_info.symbol ?? registry_symbol`, `token_out_info`, and `leg_symbols` values (`:1051-1062`); value kept only if finite and `> 0` (`:1009`) | `null` when no price map for the chain (`:1050`) or when no symbol of the card has a live price (`:1070`); individual absent keys are simply omitted |

### a.5 `simulated_*` — which table / column, which JOIN

**There is no JOIN or subquery for the `simulated_*` fields, and no PG table/column to name.** All five
are produced in-process:

* inputs: `arbx:trading_config:<chain_id>` (Redis, read once per distinct chain, `:916-927`), the row's
  own `amount_in_wei` / `expected_profit_usd` / `token_in` / `strategy_kind` (`SimulatorRow` built at `:940-948`);
* forward pass `forwardSimulate(simRow, snapshot)` at `:949`;
* target resolution `resolveTarget(snapshot, r.strategy_kind)` at `:950`;
* inverse sizing `inverseSize(...)` at `:961-963`;
* the `SimContext` is stored only when `forward || inverse` (`:966-968`).

The **only** tables touched by this route are `opportunities` (`:304`, `:370`) and `tokens`
(`:81`, `:372`, `:375`, `:876`). No simulation table is read here.

Verified absence (NOT VERIFIED-as-existing): a grep for `simulated_target|simulated_notes|simulated_cost_breakdown|token_prices_usd` over `*.sql` returns **only** two hits, both about
`trading_config.token_prices_usd` (`database/migrations/032_trading_config_simulation_knobs.sql:21,41`)
and a comment (`database/migrations/056_strategy_configs.sql:15`). No migration creates a
`simulated_*` column anywhere.

There **is** a real `simulations` table — but a different one, written by sim-ctl, not read by this
route: `INSERT INTO simulations (opportunity_id, simulator, gas_estimate_wei, gas_price_wei,
slippage_pct, revert_risk_pct, simulated_profit_usd, passed, fail_reason, trace_id, simulated_at)`
at `backend/sim-ctl/src/persistence.rs:30-36`, followed by `UPDATE opportunities SET status=$2,
rejection_reason=COALESCE($3, rejection_reason)` at `backend/sim-ctl/src/persistence.rs:96-102`.
None of those columns is named `simulated_target`/`simulated_cost_breakdown`/`simulated_notes`.

Redis `arbx:trading_config:<chain_id>` writer (the true "producer" of every simulated input):
`PUT /admin/trading-config/:chain_id` → PG upsert (`trading-config.ts:577-683`) → Redis mirror
`deps.redis.set(tradingConfigKey(chainId), json)` (`trading-config.ts:687`) → publish
`arbx:trading_config:changes` (`:688`).

### a.6 Producers that WRITE the columns this route reads

| producer | file:line | columns written |
|---|---|---|
| **searcher-rs** (canonical) | `backend/searcher-rs/src/persistence.rs:146-165` (`INSERT INTO opportunities`, single funnel) | `id, chain_id, strategy_kind, dex_a, dex_b, pair_symbol, token_in, token_out, amount_in_wei, expected_profit_usd, net_expected_profit_usd, roi_pct, risk_score, block_number, status, rejection_reason, trace_id, detected_at, route_metadata, cartridge_id, detector_id, pipeline_latency_ms`; `status` derived at `:53-58`, `route_metadata` gate at `:101-129`, `ON CONFLICT (id) DO NOTHING` at `:163` |
| **api-server bridge archiver** (`ARBX_OPPS_BRIDGE_MODE=on`, `opportunities-bridge-archiver.ts:78-80`) | `backend/api-server/src/routes/opportunities-bridge-archiver.ts:252-259` | **only 16 columns**: `id, chain_id, strategy_kind, dex_a('route_discovery'), dex_b(NULL), pair_symbol, token_in, token_out, amount_in_wei(0), expected_profit_usd, roi_pct(NULL), risk_score, block_number(NULL), status, rejection_reason, trace_id, detected_at`. **Does NOT write** `net_expected_profit_usd`, `route_metadata`, `cartridge_id`, `detector_id`, `pipeline_latency_ms`, `chain_id_out`, `bridge`, `bridge_fee_usd` → all NULL / column DEFAULT (`'{}'` for `route_metadata`, migration 099) |
| sim-ctl | `backend/sim-ctl/src/persistence.rs:96-102` | `UPDATE opportunities SET status, rejection_reason, updated_at WHERE id=$1 AND status IN ('validated','scored','detected')` |
| selector-api | `backend/selector-api/src/persistence.ts:28` (`UPDATE opportunities`) | `status`, `rejection_reason` (per header comment `:5`) |
| relays-client | `backend/relays-client/src/persistence.rs:57` (`UPDATE opportunities`) | post-execution status |
| recon | `backend/recon/src/persistence.rs:49` (`UPDATE opportunities`) | reconciliation status (`reconciled`) |
| api-server paper-trade archiver | `backend/api-server/src/routes/paper-trade-archiver.ts:342-346` | writes **`paper_trade_runs`** only (`INSERT INTO paper_trade_runs (opportunity_id, chain_id, strategy_kind, sim_expected_profit_usd, sim_gas_cost_usd, sim_block_number, reason, route_hash, execution_time_ms)`); it **reads** `expected/net_expected_profit_usd` (`:287-310`) but does **not** write `opportunities` |
| **not** a writer | `backend/api-server/src/routes/route-discovery-outcome-sink.ts:168-173` | writes `route_discovery_outcomes`, **never** `opportunities` |

`INSERT`-into-`opportunities` grep across the whole repo returns exactly two non-test writers:
`persistence.rs:148` (Rust) and `opportunities-bridge-archiver.ts:252` (TS). So there is **no second
writer of the same column** in the same request; the only same-request overwrites are inside the route
(see §a.7).

### a.7 Fields overwritten by a SECOND writer inside the same request

Four real in-request double-writes (all in `opportunities-live.ts`), none of them cross-layer:

1. **`token_in_info.symbol/decimals/resolved_via`** — first written by the LEFT JOIN (SQL `:325-328`),
   then overwritten in the row-mutation loop `:817-825`:
   ```ts
   821:            r.token_in_symbol = m.symbol;
   822:            r.token_in_decimals = m.decimals;
   823:            r.token_in_resolved_via = "onchain_partial";
   ```
   Guarded by `if (r.token_in_symbol == null)` (`:818`) — enrich-then-fallback, no clobber of JOIN data.
2. **`token_out_info.*`** — same pattern at `:826-834` (chain resolved as `r.chain_id_out ?? r.chain_id`,
   `:827`).
3. **`leg_symbols`** — built by two writers into the same map: one unnest JOIN into `tokens`
   (`:871-888`) and the on-demand RPC resolver for the misses (`:892-896`). Then `rowToOpportunity`
   reads that map (`:652`) — ordering matters: the map is fully populated (`:869-897`) *before* the
   items are mapped (`:1038-1045`).
4. **`token_prices_usd`** — the item is first built by `rowToOpportunity` without it (`:1039-1045`),
   then the whole map is attached or explicitly nulled:
   ```ts
   1050:                            if (!pm) return { ...item, token_prices_usd: null };
   ```
   and `1068-1071`:
   ```ts
   1068:                            return {
   1069:                              ...item,
   1070:                              token_prices_usd: Object.keys(prices).length > 0 ? prices : null,
   1071:                            };
   ```
   Note the spread order: `{...item, token_prices_usd}` — the enrichment wins, `item` never carries the
   field.

Additional persistent side effect (documented as a cache warm, not an item overwrite): the on-demand
resolver is documented to **write back to `tokens`** so the next request's JOIN hits — route header
comment `opportunities-live.ts:796-801` (*"persists the result back to `tokens` (so the next request
hits the JOIN cache)"*), invoked at `:814` and `:895`. The resolver body
(`backend/api-server/src/routes/tokenResolver.ts`) was **not** read line-by-line in this audit
(NOT VERIFIED at that granularity); its own header cites the persistence behaviour and the
hard-timeout contract.

---

## (b) COMPETING ENDPOINTS

| endpoint | file:line | fields served |
|---|---|---|
| **WS `new_opportunity`** (Socket.IO room `opportunities`) | `backend/api-server/src/websocket.ts:515` (`io.to('opportunities').emit('new_opportunity', opp)`); payload = raw PG row via `NOTIFY row_to_json` trigger, `LISTEN opportunities_channel` at `backend/api-server/src/index.ts:1903-1914` | RAW `opportunities` row: `expected/net profit`, `roi_pct`, `block_number`, `route_metadata`, `detector_id`, `amount_in_wei` (stringified, `:502-505`). **Explicitly documented as NOT carrying** `token_in_info`/`token_out_info`, `leg_symbols`, `chain_base_token_symbol`, `paper_status`, `chains_used`, `dexes_used`, `simulated_*` — comment `websocket.ts:506-514` |
| **WS `opportunity:detected` / `opportunity:validated`** (Redis stream bridge) | `backend/api-server/src/websocket.ts:868-1182` (`emitEntry` `:1017-1023`; streams `arbx:hot:detected`/`arbx:hot:simulated` `:973-974`) | raw stream field map `{...data, _stream_id}` — no route-group aggregates |
| `GET /api/opportunities/live` (edge) | `edge/worker/src/index.ts:699` | transparent proxy of the whole payload above; KV cache `arbx:cache:opps` TTL 2s |
| `GET /api/opportunities/live` (edge dev-local) | `edge/dev-local/src/index.ts:338` | transparent proxy |
| `/hot/v1/opportunities/{live,detected,simulated}` | `edge/worker/src/index.ts:793-795` (proxies to api-server `/hot/...`); dev-local real impl `edge/dev-local/src/index.ts:1096-1163` reading Redis `arbx:hot:*` | hot-path Redis stream fields (`XREVRANGE`), not PG |
| `POST /api/v1/opportunities/:id/simulate` | `backend/api-server/src/routes/opportunity-simulate.ts:37-118`, mounted `backend/api-server/src/index.ts:621` | **no opportunity fields** — passthrough of sim-ctl `/simulate` result (`:89-110`); optional `route_metadata` enrichment from searcher-rs `/route` when `route_source=searcher_api` (`:64-84`) |
| `GET /api/v1/analytics/viable-kpis` | `backend/api-server/src/routes/viable-kpis.ts:78-146`, mounted `index.ts:751` | `totals.viable/routed/total/viability_pct`, `by_hops` (from `jsonb_array_length(route_metadata->'dex_adapters')`), `by_kind` (`strategy_kind`) — aggregates over `opportunities` |
| `GET /api/v1/rejections/breakdown` | `backend/api-server/src/routes/rejection-breakdown.ts:152-305`, mounted `index.ts:1666` | `families[].count/share/avg_gross_usd/avg_net_usd/top_raw`, `token_flood[]`, `total_rows`, `rejected_rows` — reads `opportunities.expected_profit_usd`, `net_expected_profit_usd`, `rejection_reason` (`:174-195`) |
| `GET /api/v1/sim/pipeline` | `backend/api-server/src/routes/sim-pipeline.ts:42-116`, mounted `index.ts:787` | per-`strategy_key` score aggregates: `scored`, `accepted`, `avg_posterior_prob`, `avg_kelly_fraction`, `avg_recommended_usd`, `evidence_rows`, `last_scored_at`, `source_context`; `calibrated_strategies` — from `scored_opportunities` + `bayesian_priors` |
| `ScoredOpportunitiesArchiver` (writer, not endpoint) | `backend/api-server/src/routes/scored-opportunities-archiver.ts:173-199` (mounted/started `index.ts:2007-2023`) | writes `scored_opportunities` (stream `arbx:scoring:scored`): `stream_id, opportunity_id, token_pair, posterior_prob, kelly_fraction, recommended_usd, net_profit_usd, bayesian_accepted, prior_log_odds, chain_id, source_context, scoring_mode, evidence_vector, strategy_key, emission_outcome, rejection_reason` — **does NOT touch `opportunities`** |
| `OpportunitiesBridgeArchiver` (writer, not endpoint) | `backend/api-server/src/routes/opportunities-bridge-archiver.ts:251-274` (gated `:78-80`, started `index.ts:2033-2042`) | **writes `opportunities`** from Redis `arbx:route_discovery:outcomes` — the 16-column INSERT listed in §a.6 |
| `GET /api/v1/route-discovery-outcomes[/summary]` | `backend/api-server/src/routes/route-discovery-outcomes-api.ts:90-365`, mounted `index.ts:748` | `route_discovery_outcomes` rows/totals (stream sink `route-discovery-outcome-sink.ts:168-173`) |
| `GET /api/v1/trading-config` + `PUT /admin/trading-config/:chain_id` | `backend/api-server/src/routes/trading-config.ts`, mounted `index.ts:1797-1805`; Redis write `:687-688` | the full `trading_config` row incl. `token_prices_usd`, `simulation_target_profit_usd`, `simulation_target_roi_pct`, `strategy_configs`, cost scalars — the SOURCE of every `simulated_*` and `chain_base_token_symbol` value in §(a) |
| `GET /api/v1/prices/live` | `backend/api-server/src/routes/prices-live.ts:27-64`, mounted `index.ts:557` | `{chain_id, prices, count, ttl_secs, ts}` from the SAME Redis hash `arbx:token_prices:<chain>` (`:39-52`) that feeds `token_prices_usd` |
| `GET /api/v1/executions/recent` | `backend/api-server/src/index.ts:1063-1091` | execution rows joined to opportunities: `e.expected_profit_usd`, `e.actual_profit_usd`, `o.chain_id`, `o.strategy_kind`, `o.pair_symbol` (`:1069-1078`) |
| `GET /api/v1/paper/history` + `/summary` | `backend/api-server/src/routes/paper-history-api.ts:74-146` | `sim_expected_profit_usd`, `sim_net_profit_usd`, `profitable`, `avg_expected_profit_usd`, `avg_net_profit_usd`, p25/median/p75 from `paper_trade_runs` |
| `GET /api/v1/metrics/paper-shadow` | `backend/api-server/src/routes/paper-shadow-metrics.ts:31` | `PNL_EXPR = "COALESCE(actual_profit_usd, sim_expected_profit_usd, 0)"` |
| `GET /api/v1/go-no-go/{ledger,status}` | `backend/api-server/src/routes/go-no-go.ts:561` | same `PNL_EXPR` over `paper_trade_runs` |
| `GET /api/admin/topology/snapshot` (+ `/api/admin/topology/mutations`) | `edge/worker/src/index.ts:1474`, `:1484` | topology vault (graph), not opportunity rows |
| `/api/strategies/runtime-status`, `/api/strategies/catalog`, `/api/detectors/catalog`, `/api/scoring/status` | `edge/worker/src/index.ts:1363`, `:1368`, `:1369`, `:1430` | strategy/detector catalog + runtime status — no per-opportunity economics |

**Conclusion for (b):** `/api/v1/opportunities/live` is the ONLY endpoint that emits the full enriched
per-item shape. Everything else either (i) emits the same data differently (WS raw row, edge proxy),
(ii) aggregates it (kpis / breakdown / pipeline / paper metrics), or (iii) writes it (bridge archiver).

---

## (c) `leg_symbols` NULL ROOT CAUSE

### c.1 The exact code path

`opportunities-live.ts:640-659`:

```ts
640:    // F2 (audit §11 RC1): symbols for intermediate route legs (multi-hop).
641:    // Only resolved addresses are included; an absent entry = unresolved →
642:    // the client falls back to its honest shortAddr (R8: no fabrication).
643:    leg_symbols: (() => {
644:      const rm = row.route_metadata;
645:      if (!rm || typeof rm !== "object") return null;
646:      const addrs = (rm as { token_addresses?: unknown }).token_addresses;
647:      if (!Array.isArray(addrs) || addrs.length === 0) return null;
648:      const out: Record<string, string> = {};
649:      let any = false;
650:      for (const a of addrs) {
651:        if (typeof a !== "string") continue;
652:        const s = legSymbols.get(tokenCacheKey(row.chain_id, a));
653:        if (s) {
654:          out[a.toLowerCase()] = s;
655:          any = true;
656:        }
657:      }
658:      return any ? out : null;
659:    })(),
```

**There are exactly three `null` returns and one falsy fallthrough:**

| # | line | condition |
|---|---|---|
| N1 | `:645` | `row.route_metadata` is `null`/not an object. **Already reached before this point** because `rowToOpportunity` coerces an empty JSONB object to `null` at `:701-706` — this branch fires for any row whose `route_metadata` is `{}` |
| N2 | `:647` | `route_metadata.token_addresses` absent, not an array, or an empty array |
| N3 | `:658` | `token_addresses` present and iterable but **every** entry failed the symbol lookup (i.e. `legSymbols` has no entry for any of them) |
| N4 | `:650` | entries are non-strings → skipped (contributes to N3) |

The `legSymbols` map itself is filled at `:849-897`: batch unnest JOIN into `tokens` for all
non-endpoint addresses (`:871-888`, requires `t.symbol IS NOT NULL` at `:880`), then
`resolveTokensOnDemand` for the misses (`:895`). Endpoint addresses are deliberately excluded
(`:863-866`):

```ts
864:          if (a.toLowerCase() === r.token_in.toLowerCase()) continue;
865:          if (a.toLowerCase() === r.token_out.toLowerCase()) continue;
```

**Consequence:** `leg_symbols` can only ever be non-null for a route whose
`route_metadata.token_addresses` contains **≥1 address that is neither `token_in` nor `token_out`**
(i.e. a genuine ≥3-hop cycle with all distinct tokens). A 2-token pair route is *structurally
incapable* of producing `leg_symbols` — every address is an endpoint, `any` stays false, `:658`
returns `null`.

### c.2 Why production is 41/41 null — the writer chain

1. `route_metadata` is the ONLY leg source (`:644-647`). There is no fallback to
   `route_metadata.pool_addresses`, to the `tokens` table, or to the WS path.
2. Column definition: `opportunities.route_metadata JSONB NOT NULL DEFAULT '{}'::jsonb`
   (`database/migrations/099_opportunities_route_metadata.sql:41`; the empty `{}` is the "no topology"
   value — comment `:22-27,51-52`). Empty → `:701-706` maps it to wire `null`:
   ```ts
   701:    route_metadata:
   702:      row.route_metadata &&
   703:      typeof row.route_metadata === "object" &&
   704:      Object.keys(row.route_metadata).length > 0
   705:        ? row.route_metadata
   706:        : null,
   ```
3. The rows carrying `dex_a = 'route_discovery'` are written by the **api-server bridge archiver**,
   whose INSERT column list is only:
   ```ts
   252:        `INSERT INTO opportunities
   253:           (id, chain_id, strategy_kind, dex_a, dex_b, pair_symbol, token_in, token_out,
   254:            amount_in_wei, expected_profit_usd, roi_pct, risk_score, block_number,
   255:            status, rejection_reason, trace_id, detected_at)
   ```
   (`opportunities-bridge-archiver.ts:252-255`). `route_metadata` is **not in the list** → column
   DEFAULT `'{}'` → N1 fires for every such row. The provenance of `dex_a='route_discovery'` as this
   archiver's marker is confirmed independently in the Rust side's own metric doc:
   `backend/searcher-rs/src/metrics.rs:476-479` — *"The api-server route-discovery bridge archiver also
   INSERTs into `opportunities` (dex_a='route_discovery')"*.
4. Even for `route_metadata` that *does* get written, the Rust persistence gate structurally refuses
   the 3-hop triangular topology and downgrades it to `'{}'`:
   `backend/searcher-rs/src/persistence.rs:109-123`:
   ```rust
   109:            let structurally_ok =
   110:                rm.token_addresses.len() == hops + 1 && rm.pool_addresses.len() <= hops;
   ...
   123:                serde_json::json!({})
   ```
   And the triangular engine builds exactly the topology that fails it —
   `backend/searcher-rs/src/engines/triangular_engine.rs:628-629`:
   ```rust
   628:        token_addresses: vec![token_a_addr.clone(), token_c_addr.clone()],
   629:        dex_adapters: vec!["uniswap-v2".to_string(); 3],
   ```
   For a closed A→B→A cycle `token_a_addr == token_c_addr`, so `token_addresses = [A, A]` (len 2)
   while `dex_adapters.len() == 3` ⇒ `structurally_ok = (2 == 4) && ...` = false ⇒ persisted as `{}`.
   Independently corroborated by the production observation recorded in the code comment at
   `backend/searcher-rs/src/orchestrator.rs:1209-1210`: *"`route_metadata.decimals.map` was empty in
   32/32 production rows"*.
5. Residual (not excluded by code alone): N3 would also fire if a genuine multi-hop topology existed
   but no leg symbol resolved — requires `tokens.symbol IS NULL` **and** the on-demand RPC resolver
   failing for every leg (`:892-896`).

**Verdict:** `leg_symbols === null` is the *expected*, honest output for the current
production topology: the live rows have no topology at all (bridge-archiver INSERT omits
`route_metadata`), and the Rust path that *does* try to persist a triangular topology is structurally
gated to `'{}'` by `persistence.rs:110`. To make `leg_symbols` non-null, either the bridge archiver must
write `route_metadata.token_addresses` with the intermediate hop, or the triangular builder must emit
the full `[A,B,C,A]` path so `token_addresses.len() == hops + 1`.

`__forTesting.rowToOpportunity` (`:1086`) is the pure mapper that makes this testable without PG. The
route's own test fixture even *omits* `leg_symbols` from its populated-topology assertion
(`backend/api-server/src/routes/opportunities-live.test.ts:217-227` — it inserts
`token_addresses: [...addresses, addresses[0]]` and only asserts `route_metadata`), so **no test pins
the `leg_symbols` contract** — that is the regression gap.

---

## (d) ROUTE MOUNT SPLIT (`:8080` vs edge `:8787`)

### d.1 api-server serves ONLY `/api/v1/opportunities/live`

* Registration: `backend/api-server/src/routes/opportunities-live.ts:741`
  `app.get("/api/v1/opportunities/live", ...)`
* Mount call: `backend/api-server/src/index.ts:555` `mountOpportunitiesLive(app, pool, redis, logger);`
  (inside the "Sprint 7: public v1 read endpoints" block, comment `index.ts:546-549`)
* Port: `backend/api-server/src/index.ts:1788` `const PORT = Number(process.env["API_PORT"] ?? 8080);`
  and `:2046` `httpServer.listen(PORT, ...)`; compose sets `API_PORT: "8080"` (comment `:1782-1787`)
* A repo-wide grep for `app.get("/api/opportunities` in the api-server returns **zero** matches; the
  only `/api/opportunities/live` string in `backend/` is a comment
  (`opportunities-bridge-archiver.ts:9`).

Therefore `curl localhost:8080/api/opportunities/live` inside the api-server container hits no route →
Express's default 404 `Cannot GET /api/opportunities/live`. **This is not a failure of the route; it is
the correct response for a path the api-server does not own.** A correct in-container probe is
`curl localhost:8080/api/v1/opportunities/live`.

### d.2 The public path is owned by the edge worker

`edge/worker/src/index.ts:699`:

```ts
699: app.get("/api/opportunities/live", (c) => proxy(c, "/api/v1/opportunities/live", "arbx:cache:opps", 2));
```

(header `:9` documents the KV cache: *"KV-backed cache for `/status` and `/api/opportunities/live` (S1: 2 s TTL)"*).
The proxy rewrites `/api/opportunities/live` → api-server `/api/v1/opportunities/live`
(`proxy()` at `edge/worker/src/index.ts:673-679`). Identical wiring in dev:
`edge/dev-local/src/index.ts:338`
`app.get("/api/opportunities/live", (req, res) => proxy("/api/v1/opportunities/live", req, res));`.

### d.3 The split, stated exactly

| layer | path it serves | evidence |
|---|---|---|
| api-server (:8080) | `/api/v1/opportunities/live` **only** | `opportunities-live.ts:741`, `index.ts:555`, `index.ts:1788` |
| edge worker (:8787 / `NEXT_PUBLIC_EDGE_URL`) | `/api/opportunities/live` → proxies to the api-server v1 path, KV-cached 2s | `edge/worker/src/index.ts:699` |
| edge dev-local | same | `edge/dev-local/src/index.ts:338` |
| frontend | fetches the edge path: `${getPublicEdgeBaseUrl()}/api/opportunities/live?...` | `frontend/lib/store/useOmniOpportunities.ts:129`; also `frontend/lib/api-client.ts:304` |

This matches the project's own doctrine (RULE 02 in `CLAUDE.md`: REST → Edge Worker). Docs that cite
the edge path's api-server source are consistent with the split:
`docs/architecture/frontend-wiring-map.json:126` lists the api-server endpoint as
`GET /api/v1/opportunities/live`.

No stub shadows the real handler: `mountStubs(app, ...)` runs at `backend/api-server/src/index.ts:1752`
— *after* `mountOpportunitiesLive` (`:555`) — and its opportunity surface is only
`POST /api/v1/opportunities/:id/simulate` → 501 (`backend/api-server/src/routes/stubs.ts:101-105`),
which is itself shadowed by the real handler mounted at `index.ts:621`. A repo-wide grep for
`app.get("/api/opportunities` inside `backend/` returns zero matches, so no early catch-all can be
explaining the observed 404 either.

---

## (e) UNKNOWNS / NOT VERIFIED

Every item below is something this audit could **not** establish from the repository, or a claim that
must be checked against the live VPS before being acted on. No name was invented.

**Schema / columns**

1. **`simulated_target`, `simulated_cost_breakdown`, `simulated_at`, `simulated_notes` — NOT VERIFIED as
   PG columns, and the evidence says they do not exist as such.** No `*.sql` in the repo declares them
   (grep hit count 0). They are TS-computed per request. If a live VPS column with one of these names
   exists, it was created outside this repo's migration tree.
2. **`opportunities.token_prices_usd` — NOT VERIFIED as existing; evidence says it does not.** The only
   `token_prices_usd` column found is `trading_config.token_prices_usd JSONB NOT NULL DEFAULT '{}'`
   (`database/migrations/032_trading_config_simulation_knobs.sql:21`, deployed-schema row
   `audits/schema-drift-2026-09-20/deployed_schema.tsv:1082`). The wire field comes from Redis.
3. The deployed `opportunities` column list used here is taken from the audit artifact
   `audits/schema-drift-2026-09-20/deployed_schema.tsv:367-392` (26 columns, snapshot 2026-09-20) — a
   *snapshot*, not a live read. `simulated_*`/`token_prices_usd` are absent from it. I did not query the
   live DB (no read-only DB access in this session).
4. `opportunities.updated_at` (present in the snapshot, `:384`) is **never** surfaced by this route.

**Runtime facts that code cannot settle**

5. **Which producer wrote the 41 live items is inferred, not directly observed.** All three signals in
   the code (`dex_a = 'route_discovery'` literal, `amount_in_wei = 0`, `net/roi/block = NULL`) are
   unique to the bridge archiver (`opportunities-bridge-archiver.ts:252-259`), and `metrics.rs:476-479`
   names that archiver as the `dex_a='route_discovery'` writer. But the CB-01 census recorded the gate as
   **off** (`audits/control-board-2026-09-07/CB-01-CENSO.md:79`: `opps_bridge_archiver | B |
   ARBX_OPPS_BRIDGE_MODE | off`). Either it was enabled after 2026-09-07 or a row source I could not see
   exists. Confirm with:
   `SELECT dex_a, count(*) FROM opportunities WHERE detected_at > now() - interval '1 hour' GROUP BY 1;`
   and `docker exec <api-server> printenv ARBX_OPPS_BRIDGE_MODE`.
6. **The empirical proof of the `leg_symbols` cause is a query, not a code read.** Run:
   `SELECT route_metadata::text, token_in, token_out, dex_a FROM opportunities
    WHERE detected_at > now() - interval '1 hour' ORDER BY detected_at DESC LIMIT 41;`
   Expected: 41 × `{}` (or a populated topology whose `token_addresses` has exactly the two endpoints).
   If instead you see non-empty `token_addresses` with ≥3 distinct addresses, then the cause is branch N3
   (symbol lookup miss) and the fix is in `tokens` coverage, not in the writer.
7. **Is `ARBX_OPPS_BRIDGE_MODE=on` in production?** The default is `off`
   (`opportunities-bridge-archiver.ts:78-80`); no compose/`.env` file in the repo sets it (grep for
   `ARBX_OPPS_BRIDGE_MODE` in `*.yml` = 0 matches).
8. **Does `redis` reach the route as non-null in production?** Two independent branches degrade when it
   is null: `simulated_*` all stay null (`:939 continue`) and `chain_base_token_symbol` is null
   (`:1042` opt-chain). If `simulated_*` is null on 100% of items, this is the first thing to check —
   `arbx:trading_config:1` presence (`docker exec redis redis-cli GET arbx:trading_config:1`).
9. **`arbx:token_prices:<chain>` coverage**: the wire `token_prices_usd` is `null` for the whole item
   whenever no card symbol has a live price (`:1070`). Requires
   `docker exec redis redis-cli HGETALL arbx:token_prices:1` to distinguish "producer silent" from
   "symbol mismatch".
10. **Type parity of `route_group_key`** is asserted only for the shapes present in the schema
    (`chain_id_out integer`). No runtime test compares the SQL key with `routeGroupKeyOf` on live rows —
    `frontend/lib/store/__tests__/route-key.test.ts` pins the JS side only.
11. **`leg_symbols` has no test.** `backend/api-server/src/routes/opportunities-live.test.ts:217-227`
    builds a populated topology but asserts only `route_metadata`; the contract of `leg_symbols`
    (including its three null branches) is unpinned by any test I found.
12. **Cross-chain leg lookup uses `r.chain_id` for every leg address** (`opportunities-live.ts:652`,
    `:860`, `:866`) even though the token path may contain `chain_id_out` hops. Whether a real cross-chain
    topology can carry `chain_id_out` leg addresses is NOT VERIFIED — no cross-chain row with populated
    `route_metadata` was found in the repo artifacts.
13. **`hot/v1/opportunities/*` on the canonical edge worker**: `edge/worker/src/index.ts:793-795` proxies
    those paths straight through to `API_SERVER_URL` (`:673`), but no `/hot/v1/*` route could be located
    in `backend/api-server/src`. Only `edge/dev-local` implements them locally (`:1096-1163`). Whether the
    canonical deployment serves them is NOT VERIFIED.

**Files read in full for this report**

`backend/api-server/src/routes/opportunities-live.ts` (1086 lines),
`backend/api-server/src/routes/opportunity-simulate.ts` (119),
`backend/api-server/src/routes/opportunities-bridge-archiver.ts` (285),
`backend/api-server/src/routes/scored-opportunities-archiver.ts` (211),
`backend/api-server/src/routes/rejection-breakdown.ts` (308),
`backend/api-server/src/routes/viable-kpis.ts` (149),
`backend/api-server/src/routes/sim-pipeline.ts` (117),
`backend/api-server/src/routes/prices-live.ts` (65),
`backend/api-server/src/simulation/tradingConfigSnapshot.ts` (317),
`frontend/lib/store/route-key.ts` (31).
Read in cited ranges: `backend/api-server/src/index.ts` (:490-740, :740-810, :960-1040, :1040-1120,
:1775-1815), `backend/api-server/src/websocket.ts` (:498-537), `backend/searcher-rs/src/persistence.rs`
(:1-300), `backend/searcher-rs/src/orchestrator.rs` (:1150-1279),
`backend/searcher-rs/src/engines/triangular_engine.rs` (:580-659),
`backend/searcher-rs/src/metrics.rs` (:468-497), `backend/sim-ctl/src/persistence.rs` (:20-149),
`backend/shared-rs/src/candidates.rs` (:160-209),
`backend/api-server/src/routes/trading-config.ts` (:330-399, :560-739),
`backend/api-server/src/routes/route-discovery-outcome-sink.ts` (:150-209),
`edge/worker/src/index.ts` (:673-732), `backend/api-server/src/simulation/computeSimulatedNet.ts` (:1-120).
