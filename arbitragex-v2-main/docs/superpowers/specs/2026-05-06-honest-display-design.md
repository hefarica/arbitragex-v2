# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# Design Spec â€” Honest Display for `/opportunities` (Multi-Chain + Cross-Chain Slots)

**Fecha:** 2026-05-06
**Autor:** OMEGA Master Cortex (Claude Opus 4.7)
**Estado:** APROBADO por operador en brainstorming 2026-05-06 (este documento aÃºn pendiente review escrito)
**Sub-proyecto de:** "Honest Opportunities" â€” 6 sub-proyectos secuenciales (Aâ†’Bâ†’Câ†’Dâ†’Eâ†’F, ver Â§1)
**Branch destino:** `main` (incremental, sin worktree)
**No-damage contract:** 100% aditivo. Schema migrations son `ALTER ... DROP NOT NULL` (idempotentes) y `ADD COLUMN ... NULL`. Nueva tabla `tokens` no afecta queries existentes.

---

## 1. Contexto y descomposiciÃ³n

### Por quÃ© existe este sub-proyecto

AuditorÃ­a 2026-05-06 (response live `http://195.201.235.70:5173/opportunities`):

- 50 opportunities reales detectadas por `searcher-rs` desde mempool de Ethereum mainnet (datos verificables: WETH `0xc02aaaâ€¦`, UUIDs, timestamps actuales).
- **Pero la UI muestra**:
  - `pair_symbol` = `"c02aaaâ€¦/dace81â€¦"` (hex truncado, no sÃ­mbolo) â€” porque no hay token registry.
  - `dex_b` = `null` para todas (correcto: el searcher solo observa una pierna; doctrina explÃ­cita en [`patterns.rs:1-14`](backend/searcher-rs/src/patterns.rs#L1-L14)).
  - `expected_profit_usd: 0`, `roi_pct: 0`, `risk_score: 0` para todas (struct Rust hardcodea `0.0` porque el simulador no escribe back; viola R8 fail-honest).
  - Sin logos.

### No-mocks status

AuditorÃ­a confirmÃ³: **el pipeline NO tiene hardcodes ni mocks**. Los datos son reales pero:
1. el frontend muestra hex en vez de sÃ­mbolos (no hay enricher),
2. los campos no calculados se muestran como `0` en vez de `null` (R8 fail-honest violado en el path Rust).

### RelaciÃ³n con specs previas

- **`2026-05-04-real-profit-signal-design.md`** ("Sub-proyecto 1 â€” Real profit signal in detection") trata de hacer que el scanner emita `expected_profit_usd > 0` reales vÃ­a pool data layer + V2 quote engine. Ese spec **se solapa con futuro Sub-Proyecto B** ("Simulator Closes Loop") de la descomposiciÃ³n presente, NO con este Sub-Proyecto A. Los nombres "Sub-proyecto 1" en ambos specs refieren a trayectos distintos.
- **`2026-05-03-sop-evm-integration-design.md`** define la matriz de 10 estrategias y el roadmap REVM real para Sprint 4. Cross-chain (Estrategia #9 EXTREMA) estÃ¡ marcado como "pendiente" â€” Sub-Proyecto D abajo lo aborda.

### DescomposiciÃ³n aprobada por operador (orden secuencial estricto X)

```
Same-chain track
  Sub-Proyecto A  Honest Display          (este spec)              1-2 dÃ­as
  Sub-Proyecto B  Simulator Closes Loop                             1-2 semanas
  Sub-Proyecto C  Single-Chain Cycle Finder                         3-4 semanas

Cross-chain track  (depende de same-chain track maduro)
  Sub-Proyecto D  Cross-Chain Foundation                            4-6 semanas
  Sub-Proyecto E  Cross-Chain Paper Trade                           4 semanas mÃ­nimo
  Sub-Proyecto F  Cross-Chain Live Gated                            ?
```

Este documento cubre **solo Sub-Proyecto A**. B, C, D, E, F obtendrÃ¡n cada uno su propio spec en su momento.

---

## 2. Scope de Sub-Proyecto A

### Cobertura multi-chain

Multi-chain coverage (no cross-chain arbitrage). Cada cadena opera independientemente:

- Ethereum L1 (`chain_id=1`)
- Arbitrum (`chain_id=42161`)
- Optimism (`chain_id=10`)
- Base (`chain_id=8453`)
- Polygon (`chain_id=137`)
- BNB Chain (`chain_id=56`)

El sub-proyecto **prepara** slots cross-chain en schema y UI (Sub-Proyecto D los rellenarÃ¡) pero no implementa cross-chain arbitrage.

### Out of scope (queda para sub-proyectos futuros)

- âŒ CÃ¡lculo de `expected_profit_usd > 0` real (Sub-Proyecto B)
- âŒ Llenar `dex_b` con segundo leg de arbitraje (Sub-Proyecto C)
- âŒ Cross-chain detection / bridges / capital allocation cross-chain (Sub-Proyectos D-E-F)
- âŒ Filtro de safety de tokens (responsabilidad de skill `arbx-token-safety-screen`, otro componente)

---

## 3. Arquitectura

### 3.1 Flujo de datos

```
                         [searcher-rs]
                              â”‚ INSERT opportunities (NULL en profit/roi/risk + cross-chain cols)
                              â”‚ XADD arbx:opps:detected (ya existe)
                              â–¼
                       â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”         â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
                       â”‚      PostgreSQL        â”‚         â”‚       Redis         â”‚
                       â”‚                        â”‚         â”‚ arbx:opps:detected  â”‚
                       â”‚  opportunities         â”‚â—€â”€â”€INSâ”€â”€â”‚   (stream)          â”‚
                       â”‚   + chain_id_out NULL  â”‚         â”‚                     â”‚
                       â”‚   + bridge       NULL  â”‚         â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                       â”‚   + bridge_fee_usd NULLâ”‚                    â”‚ XREAD
                       â”‚                        â”‚                    â–¼
                       â”‚  tokens (NUEVA)        â”‚         â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
                       â”‚   PK (chain_id, addr)  â”‚â—€â”€â”€INSâ”€â”€â”‚ token_enricher_worker  â”‚ â† NUEVO
                       â”‚   symbol               â”‚         â”‚  Â· multicall symbol()  â”‚
                       â”‚   decimals             â”‚         â”‚  Â· multicall decimals()â”‚
                       â”‚   logo_url             â”‚         â”‚  Â· HEAD TrustWallet    â”‚
                       â”‚   resolved_via         â”‚         â”‚  Â· INSERT tokens       â”‚
                       â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜         â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                              â–²
                              â”‚ LEFT JOIN tokens (chain_id, token_in)
                              â”‚ LEFT JOIN tokens (COALESCE(chain_id_out,chain_id), token_out)
                              â”‚
                       â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
                       â”‚  api-server  â”‚  GET /api/v1/opportunities/live
                       â”‚  (modificado)â”‚  â†’ enriched response (token_in_info, token_out_info)
                       â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                              â”‚
                              â–¼
                          [edge â†’ frontend]
                          render con TokenChip + StatusPill + CrossChainSlot oculto
```

### 3.2 Componentes nuevos

| Componente | Tipo | UbicaciÃ³n |
|---|---|---|
| Tabla `tokens` | DB | `database/migrations/034_tokens_table.sql` |
| Worker `token_enricher_worker` | Crate Rust nuevo | `backend/token-enricher/` |
| Componente `<TokenChip>` | React | `frontend/components/TokenChip.tsx` |
| Componente `<DeterministicAvatar>` | React | `frontend/components/DeterministicAvatar.tsx` |
| Componente `<StrategyBadge>` | React | `frontend/components/StrategyBadge.tsx` |
| Componente `<StatusPill>` | React | `frontend/components/StatusPill.tsx` |
| Componente `<CrossChainSlot>` | React | `frontend/components/CrossChainSlot.tsx` |
| Helpers `formatProfitUSD`, `formatPctOrDash`, `formatRiskOrDash` | TS | `frontend/lib/format.ts` |

### 3.3 Componentes modificados

| Componente | Cambio |
|---|---|
| Schema `opportunities` | `DROP NOT NULL` en 3 columnas + agregar 3 columnas cross-chain (todas nullable) |
| Struct `Opportunity` Rust | `expected_profit_usd: f64` â†’ `Option<f64>` |
| `searcher-rs/src/patterns.rs` | `0.0` â†’ `None` para `expected_profit_usd` |
| `searcher-rs/src/persistence.rs` | bind ya soporta Option (sin cambio funcional, sÃ³lo via tipo) |
| `api-server/src/index.ts` (endpoint live) | Query con doble `LEFT JOIN tokens` + transformaciÃ³n a shape anidado |
| `frontend/app/opportunities/OpportunitiesClient.tsx` | Refactor del `<motion.tr>` para usar componentes nuevos |
| `shared-ts/api-contracts.ts` | Agregar tipos `TokenInfo`, `OpportunityListItem` |

### 3.4 Componentes intactos

- `searcher-rs` hot path: NO toca `scanner.rs`, `detector.rs`, `dedup.rs`, `publisher.rs`, `chain_client.rs`, `models.rs`, `amm_math.rs`, `reserves.rs`, `telemetry.rs`, `counters.rs`, `main.rs` (sÃ³lo se toca `patterns.rs` y `persistence.rs`, vÃ­a cambio de tipo en struct compartida)
- `sim-ctl`, `prioritization-spine` (responsabilidad de Sub-Proyecto B)
- `selector-api` (responsabilidad de Sub-Proyecto C)
- Edge worker `edge/dev-local/src/index.ts` y `edge/worker/src/index.ts` (passthrough proxy intacto)

---

## 4. Schema migrations

### 4.1 `033_opportunities_fail_honest_and_cross_chain_slots.sql`

```sql
-- A. Garantizar nullable en 3 columnas (idempotente; las migrations 003+
--    ya las dejaron nullable, este ALTER es defensivo).
ALTER TABLE opportunities
  ALTER COLUMN expected_profit_usd DROP NOT NULL,
  ALTER COLUMN roi_pct             DROP NOT NULL,
  ALTER COLUMN risk_score          DROP NOT NULL;

-- B. Cross-chain slots â€” populated NULL en Sub-Proyecto A; populated por
--    Sub-Proyecto D. El frontend ya tiene la rama de render para esto.
ALTER TABLE opportunities
  ADD COLUMN IF NOT EXISTS chain_id_out      INTEGER       NULL,
  ADD COLUMN IF NOT EXISTS bridge            TEXT          NULL,
  ADD COLUMN IF NOT EXISTS bridge_fee_usd    NUMERIC(20,8) NULL;

-- Constraint defensivo: chain destino debe diferir de chain origen.
ALTER TABLE opportunities
  ADD CONSTRAINT chk_cross_chain_distinct
  CHECK (chain_id_out IS NULL OR chain_id_out <> chain_id);
```

**Reversibilidad**: `DROP COLUMN chain_id_out, bridge, bridge_fee_usd` y `DROP CONSTRAINT chk_cross_chain_distinct`. Idempotente.

### 4.2 `034_tokens_table.sql`

```sql
CREATE TABLE IF NOT EXISTS tokens (
  chain_id      INTEGER     NOT NULL,
  address       TEXT        NOT NULL,             -- lowercase, '0x' prefix, 42 chars
  symbol        TEXT        NULL,                 -- NULL si symbol() fallÃ³ on-chain
  decimals      SMALLINT    NULL,                 -- NULL si decimals() fallÃ³ on-chain
  logo_url      TEXT        NULL,                 -- NULL â†’ frontend cae a avatar deterministico
  resolved_via  TEXT        NOT NULL
    CHECK (resolved_via IN ('onchain_full','onchain_partial','trustwallet_only','failed')),
  resolved_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (chain_id, address),
  CONSTRAINT chk_address_format CHECK (address ~ '^0x[a-f0-9]{40}$')
);

CREATE INDEX IF NOT EXISTS idx_tokens_last_seen ON tokens(last_seen_at DESC);

GRANT SELECT, INSERT, UPDATE ON tokens TO arbx_rw;
GRANT SELECT ON tokens TO arbx_ro;
```

**Decisiones intencionales**:

- `address` siempre lowercase + check format â†’ keys consistentes, filtra direcciones malformadas
- `symbol`/`decimals`/`logo_url` nullable â†’ fail-honest cuando una resoluciÃ³n falla parcialmente
- `resolved_via` enum strict â†’ trazabilidad por token
- PK compuesta `(chain_id, address)` â€” multi-chain por diseÃ±o

### 4.3 Cambios Rust correlacionados

[`backend/shared-rs/src/contracts.rs:29`](backend/shared-rs/src/contracts.rs#L29):
```diff
-    pub expected_profit_usd: f64,
+    pub expected_profit_usd: Option<f64>,
```

[`backend/searcher-rs/src/patterns.rs:51`](backend/searcher-rs/src/patterns.rs#L51):
```diff
-        expected_profit_usd: 0.0, // S2 does not estimate; selector+sim compute this
+        expected_profit_usd: None, // R8 fail-honest: NULL hasta que selector+sim lo calcule
```

`backend/searcher-rs/src/persistence.rs`: bind de `Option<f64>` ya emite NULL automÃ¡ticamente (sqlx). Sin cambio adicional.

---

## 5. `token_enricher_worker` â€” diseÃ±o

### 5.1 UbicaciÃ³n

Crate independiente: `backend/token-enricher/` dentro del workspace Cargo. No vive dentro de `searcher-rs` para mantener la separaciÃ³n entre hot path (latencia ms) y enricher (latencia s).

### 5.2 Disparador hÃ­brido

| Mecanismo | FunciÃ³n |
|---|---|
| `XREADGROUP` consumer-group `enricher` sobre `arbx:opps:detected` | Reactivo en tiempo real (~1-2s tras detecciÃ³n) |
| Reconciliation tick cada 5 minutos | Captura tokens omitidos por restart, errores transitorios, mensajes pre-consumer-group |

Reconciliation query:
```sql
SELECT DISTINCT chain_id, LOWER(token_in) AS address FROM opportunities
  WHERE NOT EXISTS (
    SELECT 1 FROM tokens t
    WHERE t.chain_id = opportunities.chain_id
      AND t.address  = LOWER(opportunities.token_in))
UNION
SELECT DISTINCT
  COALESCE(chain_id_out, chain_id), LOWER(token_out)
  FROM opportunities
  WHERE NOT EXISTS (
    SELECT 1 FROM tokens t
    WHERE t.chain_id = COALESCE(opportunities.chain_id_out, opportunities.chain_id)
      AND t.address  = LOWER(opportunities.token_out))
LIMIT 100;
```

### 5.3 ResoluciÃ³n on-chain (Multicall3)

Multicall3 estÃ¡ deployed en la direcciÃ³n canÃ³nica `0xcA11bde05977b3631167028862bE2a173976CA11` en TODAS las chains soportadas. Una sola RPC call por batch de 50 tokens (100 calls internos: symbol + decimals por token), con `allowFailure: true` por call.

Coste amortizado: ~4ms por token (vs ~50ms si fueran calls individuales).

### 5.4 Logo lookup (Trust Wallet)

URL pattern por chain (mapping a path Trust Wallet):

| chain_id | trustwallet path |
|---|---|
| 1 | `ethereum` |
| 42161 | `arbitrum` |
| 10 | `optimism` |
| 8453 | `base` |
| 137 | `polygon` |
| 56 | `smartchain` |

Pattern:
```
https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/{path}/assets/{address_eip55}/logo.png
```

**CrÃ­tico**: address en EIP-55 checksum case (no lowercase). El worker calcula checksum vÃ­a `alloy-primitives::to_checksum()`. PG sigue almacenando lowercase para keys consistentes; la URL se construye al vuelo.

**VerificaciÃ³n**: HTTP HEAD (no descarga PNG). 200 â†’ guardar URL. 404 â†’ `logo_url = NULL`.

**Rate limit**: GitHub raw 60 req/h sin auth, 5000 con token. Operator suministra `GITHUB_TOKEN_FOR_RAW_API` (Fase 5 no-hardcode); si falla rate limit, exponential backoff y retry en prÃ³ximo tick de reconciliation.

### 5.5 Estado final por token

| Caso | symbol | decimals | logo_url | resolved_via |
|---|---|---|---|---|
| Token estÃ¡ndar listado en TW | "WETH" | 18 | URL vÃ¡lida | `onchain_full` |
| Token raro NO en TW | "FOO" | 6 | NULL | `onchain_full` |
| Token con `symbol()` reverte | NULL | 18 | URL si TW | `onchain_partial` |
| SÃ³lo TW lo conoce | NULL | NULL | URL | `trustwallet_only` |
| Todo falla | NULL | NULL | NULL | `failed` |

En TODOS los casos se hace INSERT (R8: registramos el intento). No re-resolver tokens ya en `tokens` con `resolved_via='onchain_full'`. Re-intentar `failed` despuÃ©s de 7 dÃ­as (TTL para evitar martillar contratos rotos).

### 5.6 ConfiguraciÃ³n (sin hardcode)

`config/operator.toml` o env vars (Fase 5 del no-hardcode doctrine):

```toml
[token_enricher]
enabled = true
reconciliation_interval_seconds = 300
batch_size = 50
failed_retry_days = 7

# RPC URLs leÃ­das vÃ­a shared-rs::config::rpc_url_for(chain_id) â€” NO duplicar literales.
# El operador configura RPC_URL_<CHAIN>_PRIMARY como env vars.

[token_enricher.trustwallet]
# Opcional. Sin token: 60 req/h limit. Con token: 5000 req/h.
github_token_env = "GITHUB_TOKEN_FOR_RAW_API"
```

### 5.7 Observability

Counters Prometheus prefijados `arbx_token_enricher_*`:

```
arbx_token_enricher_resolved_total{chain_id, resolved_via}
arbx_token_enricher_failed_total{chain_id, reason}
arbx_token_enricher_lag_seconds{chain_id}
arbx_token_enricher_stream_consumed_total
arbx_token_enricher_reconciliation_caught_total
```

Logs estructurados con prefix `event=token_enricher.*`.

### 5.8 Failure modes

| Modo | DetecciÃ³n | Respuesta |
|---|---|---|
| RPC chain down | timeout | Skip esa chain, retry en reconciliation |
| Multicall3 revert (improbable) | call fail | Fallback a calls individuales, log critical |
| TW rate limited | 403 + `X-RateLimit-Reset` | Backoff hasta reset, log, recuperar en reconciliation |
| PG down | INSERT fails | Retry 3Ã— con backoff, abandonar batch, recuperar en prÃ³ximo tick |
| Worker crash | systemd / docker restart | Consumer group preserva Ãºltimo ID; reanuda |

### 5.9 Lo que el worker NO hace

- No filtra tokens scam (responsabilidad de `arbx-token-safety-screen`)
- No marca tokens "verified" (frontend infiere de `resolved_via='onchain_full'`)
- No descarga PNGs (browser hace fetch directo)
- No re-resuelve `onchain_full` (datos inmutables)
- No re-intenta `failed` durante 7 dÃ­as

---

## 6. API server â€” query y shape

### 6.1 Query con LEFT JOIN

[`backend/api-server/src/index.ts:356`](backend/api-server/src/index.ts#L356):

```sql
SELECT
  o.id, o.chain_id, o.strategy_kind, o.dex_a, o.dex_b, o.pair_symbol,
  o.token_in,  ti.symbol  AS token_in_symbol,  ti.decimals AS token_in_decimals,
               ti.logo_url AS token_in_logo_url, ti.resolved_via AS token_in_resolved_via,
  o.token_out, to_.symbol AS token_out_symbol, to_.decimals AS token_out_decimals,
               to_.logo_url AS token_out_logo_url, to_.resolved_via AS token_out_resolved_via,
  o.amount_in_wei::text AS amount_in_wei,
  o.expected_profit_usd::float AS expected_profit_usd,
  o.roi_pct::float           AS roi_pct,
  o.risk_score::float        AS risk_score,
  o.block_number, o.status, o.detected_at, o.trace_id,
  o.chain_id_out, o.bridge, o.bridge_fee_usd::float AS bridge_fee_usd
FROM opportunities o
LEFT JOIN tokens ti  ON ti.chain_id  = o.chain_id     AND ti.address  = LOWER(o.token_in)
LEFT JOIN tokens to_ ON to_.chain_id = COALESCE(o.chain_id_out, o.chain_id)
                    AND to_.address  = LOWER(o.token_out)
WHERE o.status IN ('detected','validated','simulated','scored')
ORDER BY o.detected_at DESC
LIMIT $1
```

### 6.2 Response shape (backwards compatible)

`token_in` / `token_out` permanecen como **strings** (addresses). Se agregan **objetos opcionales** `token_in_info` / `token_out_info`:

```json
{
  "id": "...",
  "chain_id": 1,
  "strategy_kind": "dex_arb",
  "dex_a": "uniswap-v2",
  "dex_b": null,
  "pair_symbol": "c02aaaâ€¦/dace81â€¦",
  "token_in": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
  "token_in_info": {
    "symbol": "WETH",
    "decimals": 18,
    "logo_url": "https://raw.githubusercontent.com/trustwallet/assets/.../logo.png",
    "resolved_via": "onchain_full"
  },
  "token_out": "0xdace81e4fe97294a9050a322cbdd48c4c240ec34",
  "token_out_info": null,
  "amount_in_wei": "19700000000000000",
  "expected_profit_usd": null,
  "roi_pct": null,
  "risk_score": null,
  "block_number": null,
  "status": "detected",
  "detected_at": "...",
  "trace_id": "...",
  "chain_id_out": null,
  "bridge": null,
  "bridge_fee_usd": null
}
```

| Campo | Caso | Valor |
|---|---|---|
| `token_*_info` | LEFT JOIN encuentra fila en `tokens` | objeto con campos resueltos |
| `token_*_info` | LEFT JOIN no encuentra fila | `null` |
| `expected_profit_usd` / `roi_pct` / `risk_score` | DB tiene NULL | `null` |
| `chain_id_out` / `bridge` / `bridge_fee_usd` | Same-chain (siempre en SP-A) | `null` |

`pair_symbol` se mantiene como hex truncado (legacy field; frontend ya no lo usa para mostrar par).

### 6.3 Tipos compartidos

`shared-ts/src/api-contracts.ts` (nuevo o extendido):

```ts
export interface TokenInfo {
  symbol: string | null;
  decimals: number | null;
  logo_url: string | null;
  resolved_via: 'onchain_full' | 'onchain_partial' | 'trustwallet_only' | 'failed';
}

export interface OpportunityListItem {
  id: string;
  chain_id: number;
  strategy_kind: 'dex_arb' | 'triangular' | 'backrun' | 'liquidation' | 'flashloan_arb';
  dex_a: string;
  dex_b: string | null;
  pair_symbol: string;
  token_in: string;
  token_in_info: TokenInfo | null;
  token_out: string;
  token_out_info: TokenInfo | null;
  amount_in_wei: string;
  expected_profit_usd: number | null;
  roi_pct: number | null;
  risk_score: number | null;
  block_number: number | null;
  status: 'detected'|'validated'|'simulated'|'scored'|'executing'
        |'executed'|'reconciled'|'rejected'|'failed';
  detected_at: string;
  trace_id: string;
  chain_id_out: number | null;
  bridge: string | null;
  bridge_fee_usd: number | null;
}
```

Validar runtime con Zod (toolkit alignment: Zod ya es dep).

### 6.4 Performance esperado

- 50 opps Ã— 2 LEFT JOIN sobre PK `(chain_id, address)` = 100 index lookups
- EXPLAIN ANALYZE esperado <5ms cache caliente, <15ms cold
- Total handler overhead +5-10ms vs estado actual
- Edge cache CF Worker (TTL 2s) absorbe ~95% de requests
- **ConclusiÃ³n**: cambio no degrada latencia observable

### 6.5 Lo que NO cambia

- Contrato 503 con `error: "db_unavailable"` cuando pool null
- Comentario doctrinal "NEVER synthesize data" en [`index.ts:349-350`](backend/api-server/src/index.ts#L349-L350)
- Filtro `WHERE status IN (...)` (mismos estados)
- Limit cap (1-200, default 50)

---

## 7. Frontend render

### 7.1 Layout final

Se mantienen las 6 columnas actuales: AGE/TIME, ROUTE, NET PROFIT, NET ROI, RISK, ACTION. La columna ROUTE concentra la mayor parte de los enriquecimientos:

```
[StrategyBadge]   [TokenChip token_in] â†’ [TokenChip token_out]
{dex_a} â†’ {dex_b ?? "awaiting cycle finder"}
[StatusPill] [CrossChainSlot (oculto si same-chain)]
```

Cuando Sub-Proyecto D entre, `<CrossChainSlot>` se materializa sin tocar JSX:
```
{dex_a} ({chainName(chain_id)}) â†’ {bridge} â†’ {dex_b} ({chainName(chain_id_out)})
```

### 7.2 Componentes

#### `<TokenChip token_address info chain_id />`

| Caso | Render |
|---|---|
| `info.logo_url` presente | `<img src={logo_url} loading="lazy" onError={fallback}/> + <span>{info.symbol}</span>` |
| `info.symbol` presente, `logo_url` null | `<DeterministicAvatar/> + <span>{info.symbol}</span>` |
| `info` null (enricher pendiente) | `<DeterministicAvatar/> + <span>{shortAddr(address)}</span>` |
| `info.resolved_via === 'failed'` | `<DeterministicAvatar/> + <span title="Token metadata unresolvable">{shortAddr(address)}</span>` |

#### `<DeterministicAvatar seed />`

SVG inline circular con gradient diagonal derivado del hash de los primeros 6 chars del address. Sin dependencias externas, ~200 bytes inlined.

#### `<StrategyBadge kind />`

| `strategy_kind` | Label | Color |
|---|---|---|
| `dex_arb` | DEX-ARB | indigo-500 |
| `triangular` | TRIANGULAR | violet-500 |
| `backrun` | BACKRUN | cyan-500 |
| `liquidation` | LIQUIDATION | rose-500 |
| `flashloan_arb` | FLASH-LOAN | amber-500 |

#### `<StatusPill status rejection_reason />`

9 estados, color-coded, con tooltip:

| Estado | Color | Tooltip |
|---|---|---|
| `detected` | slate-500 | "Captured from mempool â€” pending simulation" |
| `validated` | sky-500 | "Pre-checks passed â€” queued for sim" |
| `simulated` | teal-500 | "Profit/ROI computed â€” pending scoring" |
| `scored` | emerald-500 | "Ready to execute" |
| `executing` | amber-500 (pulse) | "Bundle submitted to relay" |
| `executed` | green-600 | "Confirmed on-chain" |
| `reconciled` | green-700 | "P&L verified" |
| `rejected` | rose-500 | rejection_reason |
| `failed` | red-600 | "Execution reverted or relay rejected" |

#### `<CrossChainSlot opp />`

```tsx
function CrossChainSlot({ opp }) {
  if (opp.chain_id_out === null) return null;
  return <span className="...">{chainName(opp.chain_id)} â†’ <span>{opp.bridge}</span> â†’ {chainName(opp.chain_id_out)}</span>;
}
```

En SP-A: siempre `return null`. SP-D activa sin redeploy de frontend.

### 7.3 Helpers fail-honest

`frontend/lib/format.ts`:

```ts
export function formatProfitUSD(value: number | null) {
  if (value === null) return { display: 'â€”', tone: 'pending' };
  if (value === 0)    return { display: '$0.00', tone: 'zero' };
  if (value > 0)      return { display: `$${value.toFixed(2)}`, tone: 'positive' };
  return                     { display: `-$${Math.abs(value).toFixed(2)}`, tone: 'negative' };
}

export function formatPctOrDash(value: number | null, fractionDigits = 2): string {
  if (value === null) return 'â€”';
  return `${value.toFixed(fractionDigits)}%`;
}

export function formatRiskOrDash(value: number | null): string {
  if (value === null) return 'â€”';
  return `${(value * 100).toFixed(1)}%`;
}
```

Reglas:
- `null` â†’ "â€”"
- `0` cuando `status='simulated'` â†’ "$0.00" (es valor real, sim no encontrÃ³ edge)
- positivo â†’ formato normal

### 7.4 Cambios en `OpportunitiesClient.tsx`

Refactor del bloque `<motion.tr>` para reemplazar `pair_symbol` raw por `<StrategyBadge>` + `<TokenChip>Ã—2` + `<StatusPill>` + `<CrossChainSlot>`. Tooltip "Pendiente Sim." obsoleto â€” reemplazado por "â€”" + tooltip nativo de helper.

### 7.5 R1 Mounted Snapshot â€” confirmaciÃ³n

Componentes nuevos son puros (parseInt sobre seed estable, `.toFixed()`, JSX sin `Date.now()` ni `Math.random()`). Cero violaciÃ³n R1.

### 7.6 R5 â€” auditorÃ­a transitivos

`framer-motion` y `lucide-react` ya en uso, sin cambios. No se introducen deps nuevas.

---

## 8. Testing strategy

Se sigue `superpowers:test-driven-development` (RED-GREEN-REFACTOR). Cada test se escribe antes que el cÃ³digo.

### 8.1 Migrations (Vitest + testcontainers PG)

- Migration 033 idempotente (rerun no falla)
- Migration 034 idempotente
- Constraint `chk_cross_chain_distinct` rechaza `chain_id=chain_id_out`
- Constraint `chk_address_format` rechaza addresses no-lowercase
- Backwards compat: searcher viejo (`expected_profit_usd: f64=0.0`) sigue insertando OK
- LEFT JOIN devuelve null cuando token no en `tokens`

### 8.2 `token_enricher_worker` (Rust)

- `resolve_token_batch` con WETH mainnet â†’ `symbol="WETH", decimals=18` (fork mainnet via Anvil)
- `symbol()` reverte â†’ `resolved_via='onchain_partial'`
- TW 404 â†’ `logo_url=NULL`
- TW rate-limited â†’ backoff y retry
- EIP-55 checksum casing en URL
- Consumer group: dos workers no procesan mismo mensaje
- Reconciliation tick recupera tokens omitidos

### 8.3 API server (Vitest)

- Query con `tokens` ausente â†’ `token_in_info: null`
- Query con `tokens` parcial (`logo_url=null`) â†’ TokenInfo con logo_url null pero symbol presente
- Cross-chain campos siempre null en SP-A (50 rows fixture)
- Response shape valida con Zod
- 503 cuando pool null preservado
- `expected_profit_usd::float` con NULL â†’ `null` en JSON

### 8.4 Frontend (Vitest + Testing Library)

- `TokenChip` 4 casos
- `DeterministicAvatar` mismo seed â†’ mismo SVG
- `formatProfitUSD(null)` â†’ `'â€”'`
- `formatProfitUSD(0)` â†’ `'$0.00'`
- `formatProfitUSD(12.34)` â†’ `'$12.34'`
- `StatusPill` con cada uno de los 9 estados
- `CrossChainSlot` con `chain_id_out=null` â†’ `return null`
- `CrossChainSlot` con `chain_id_out=42161` â†’ muestra bridge + dest chain
- Hydration: render SSR === render CSR (R1)

### 8.5 End-to-end (Playwright)

```ts
test('opportunities page shows enriched tokens or honest fallback', async ({ page }) => {
  await page.goto(`${VPS}/opportunities`);
  await page.waitForSelector('table');
  const detectedRows = await page.locator('[data-status="detected"]').all();
  for (const row of detectedRows) {
    const profit = await row.locator('[data-col="profit"]').textContent();
    expect(profit).toBe('â€”');  // R8 fail-honest
  }
  const logos = await page.locator('img[alt][src*="trustwallet"]').count();
  expect(logos).toBeGreaterThan(0);
});
```

Markup recibe `data-status` y `data-col` attrs para testabilidad sin afectar UX.

### 8.6 Smoke tests post-deploy

`automation/scripts/smoke-honest-display.sh` con 4 checks: shape nuevo, cross-chain campos null, enricher metrics > 0, frontend carga.

---

## 9. Deployment plan (rolling, zero-downtime)

### 9.1 Orden de despliegue

| # | AcciÃ³n | VerificaciÃ³n | Reversibilidad |
|---|---|---|---|
| 1 | `psql -f 033_*.sql` | `\d opportunities` muestra cols nuevas | DROP COLUMN |
| 2 | `psql -f 034_tokens_table.sql` | `\d tokens` muestra schema | DROP TABLE |
| 3 | Verificar searcher viejo sigue insertando | `event=opp.persisted` aparece | N/A |
| 4 | Deploy api-server con LEFT JOIN | `curl` devuelve `token_in_info: null` | git revert image |
| 5 | Deploy frontend con TokenChip | Browser muestra avatares + addresses + "â€”" | git revert image |
| 6 | Deploy searcher con `Option<f64>` | Nuevas filas tienen `expected_profit_usd IS NULL` | git revert image |
| 7 | Deploy `token-enricher` | Logs `event=token_enricher.batch_resolved` | systemctl stop + revert |
| 8 | Esperar reconciliaciÃ³n 5-10 min | `SELECT COUNT(*) FROM tokens` > 50 | (no aplica) |
| 9 | Verificar visualmente `/opportunities` | Logos visibles | (no aplica) |
| 10 | Smoke tests automatizados | `smoke-honest-display.sh` exit 0 | (no aplica) |

### 9.2 Por quÃ© este orden

- **Migrations primero**: schema additive, ningÃºn consumer existente se rompe.
- **API antes que frontend**: shape completo disponible cuando frontend espera `token_in_info`.
- **Searcher despuÃ©s de api-server**: si el searcher emite NULL antes de que api-server lo maneje, el frontend viejo recibe `null` donde espera `0` y crashea.
- **Enricher al final**: depende de los 3 anteriores estables. Trabajo puramente acumulativo.

### 9.3 Skills que aplican durante deploy

- `arbx-pre-edit-audit` (antes del primer Edit a `searcher-rs/src/persistence.rs` y `frontend/.../OpportunitiesClient.tsx`)
- `arbx-rpc-failover-discipline` (en `token-enricher` que usa RPC propio)
- `arbx-no-hardcode-doctrine` (config.toml â€” RPC URLs vÃ­a env, GitHub token vÃ­a env)
- `superpowers:verification-before-completion` (al cerrar SP-A)

### 9.4 MÃ©trica de Ã©xito

Al terminar SP-A, `/opportunities` cumple:

1. âœ… Logos en â‰¥80% de filas con tokens lÃ­quidos mainnet
2. âœ… SÃ­mbolos legibles (`WETH/USDC` no `c02aaaâ€¦/dace81â€¦`)
3. âœ… "â€”" en columnas Profit/ROI/Risk cuando status='detected' (no "$0.00")
4. âœ… Badge `DETECTED` por fila
5. âœ… Badge `strategy_kind` por fila
6. âœ… `<CrossChainSlot>` existe en JSX, no se renderiza (todos NULL)
7. âœ… Hydration sin warnings (R1)
8. âœ… `arbx_token_enricher_resolved_total > 0`
9. âœ… Smoke tests pasan en CI
10. âœ… Bundle size delta < 30KB

---

## 10. Inputs productivos pendientes (Fase 5 no-hardcode)

Antes de implementar, el operador suministra:

1. **RPC URLs por chain habilitada** (env vars `RPC_URL_<CHAIN>_PRIMARY` y `_SECONDARY`):
   - `RPC_URL_ETH_PRIMARY`
   - `RPC_URL_ARB_PRIMARY`
   - `RPC_URL_OPT_PRIMARY`
   - `RPC_URL_BASE_PRIMARY`
   - `RPC_URL_MATIC_PRIMARY`
   - `RPC_URL_BSC_PRIMARY`
   (Si alguna chain no se habilita en SP-A, omitir y el enricher la salta silently.)
2. **GitHub token para raw API** (opcional pero recomendado): env var `GITHUB_TOKEN_FOR_RAW_API`. PAT sin scopes; eleva rate limit Trust Wallet.

Sin estos valores, el deploy falla cleanly al boot del enricher con error explÃ­cito (no fallback a localhost ni a hardcode).

---

## 11. Riesgos y mitigaciones

| Riesgo | Probabilidad | MitigaciÃ³n |
|---|---|---|
| Trust Wallet rate-limit bloquea logos | Baja con `GITHUB_TOKEN_FOR_RAW_API` (5000 req/h); Media-alta sin token (60 req/h) | Operador suministra token (Fase 5 Â§10); reconciliation tick recupera fallos transitorios |
| RPC chain falla durante batch | Alta | Skip esa chain, retry en prÃ³ximo tick |
| Schema migration breaks existing query | Baja (additive) | Migration es 100% additive; rollback es DROP COLUMN |
| Frontend bundle aumenta significativamente | Baja | Avatar inline (sin lib externa); meta esperada <30KB delta |
| `expected_profit_usd::float` con NULL devuelve 0 (regresiÃ³n) | Baja | Test E2E verifica `null` literal en JSON |
| Hydration warning por componentes nuevos | Media-baja | Componentes son puros; tests SSR=CSR snapshot |
| `pair_symbol` legacy field rompe consumer no-frontend | Baja | Mantenido sin cambios; documentado como legacy |

---

## 12. Reversibilidad total

**Cualquier paso puede revertirse**:

- Migrations: `DROP COLUMN`, `DROP TABLE`, `DROP CONSTRAINT` â€” todos comandos seguros.
- CÃ³digo: `git revert` + redeploy de cada container.
- Tabla `tokens` puede vaciarse con `TRUNCATE` y reconstruirse desde reconciliation tick.
- Worker `token-enricher` puede detenerse permanentemente sin afectar otros componentes.

**No hay punto de no-retorno**.

---

## 13. PrÃ³ximo paso

Tras aprobaciÃ³n del operador a este spec:

1. Invocar `superpowers:writing-plans` para producir implementation plan detallado con TDD por componente.
2. Plan crearÃ¡ tareas de 2-5 minutos cada una con paths exactos, cÃ³digo completo, pasos de verificaciÃ³n.
3. EjecuciÃ³n vÃ­a `superpowers:executing-plans` o `superpowers:subagent-driven-development`.

