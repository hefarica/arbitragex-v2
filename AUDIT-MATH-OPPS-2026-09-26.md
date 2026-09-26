# AUDITORÍA MATEMÁTICA — cards de /opportunities (ArbitrageX v2)

**Rol:** math-validator + economics-validator (read-only, cero ediciones de código).
**Fecha de evidencia:** 2026-09-26T16:39Z.
**Fuentes reales usadas:**
- `curl http://195.201.235.70:8787/api/opportunities/live` → 32 items (`window_total=32`).
- `curl http://195.201.235.70:8787/api/prices/live?chain_id=1` → 385 precios.
- `curl http://195.201.235.70:8787/api/trading-config` → config viva (`updated_at 2026-09-26T16:38:01Z`, `updated_by admin`).
- Código del repo local (`backend/searcher-rs`, `backend/api-server`, `frontend`).
- **No accesible:** SSH `arbx` (exit 1, sin salida) → no pude leer PG directo ni `redis-cli`. Todo lo de abajo sale del payload HTTP + código.

---

## 0. RESUMEN EJECUTIVO (4 causas raíz, 1 bug de render)

| # | Severidad | Qué | Dónde |
|---|---|---|---|
| **B1** | **CRÍTICA** | El probe es **1e18 raw fijo** para TODO token; con USDC (6 dec) vale **10^12 USDC = $1T**. Se persiste como `amount_in_wei` y el simulador lo usa como notional ⇒ SIM −$504,267,266,530 en una card de USDC. | `dex_engine.rs:242,279` + `opportunity_emitter.rs:827` + `computeSimulatedNet.ts:173-192` |
| **B2** | **CRÍTICA** | **`max_slippage_pct = 0.5` interpretado como fracción (50 %)** en vez de 0.5 %. Cuesta **medio notional** en cada card. Es el 99.99 % del SIM −$1,347.25. | `trading-config.ts:114,172` (columna %) vs `computeSimulatedNet.ts:254` (fracción) |
| **B3** | **ALTA** | **GROSS no es la ganancia del ciclo**: es `max(out_a,out_b) − min(out_a,out_b)` de **dos swaps independientes del mismo probe**, sin encadenar el segundo hop ni cobrar fees. Para el ciclo real el gross sería **negativo**. | `dex_engine.rs:342-351` |
| **B4** | **ALTA** | **"Gross out (AMM)" = capital + gross** en la card de exchange ⇒ muestra la **salida total**, no la ganancia. De ahí el "$2,863.25" y el "$7.24M". | `OpportunityExchangeCard.tsx:257-259,439-440` |
| **B5** | **MEDIA-ALTA** | Precios imposibles en el PriceBus: **AAVE $161,339,420.31**, **SNX $272,885.72**. Envenenan `compute_gross_usd` y el sizing. | Redis `arbx:token_prices:1` (3 writers distintos) |
| **B6** | **MEDIA** | Persistencia del ledger por hop **existe pero es inalcanzable** en las 32 cards: solo se escribe en la rama `Sized`, y las 32 están rechazadas. | `size_optimizer.rs:1093-1100,1117-1131` |

Y una **refutación**: el "no computado" de GROSS/SIM/NET/ROI en 29/32 cards **no** es un bug de render — es el estado honesto (R8) con causa registrada (`v3_quote_unavailable` ×26, `single_pool_no_spread` ×3).

---

## HALLAZGOS NUMÉRICOS

### B1 — CRÍTICA — Probe 1e18 fijo ⇒ 10^12 USDC. `amount_in_wei` está sobrecargado con 3 significados

**Archivo:línea:**
- `backend/searcher-rs/src/engines/dex_engine.rs:242` y `:279` — `let probe_amount = U256::from(10u128).pow(U256::from(18u32));` (idéntico en los dos sitios).
- `backend/searcher-rs/src/engines/dex_engine.rs:342-345` — `v2_amount_out(probe_amount, …)` (el probe **no** se normaliza por decimals).
- `backend/searcher-rs/src/engines/dex_engine.rs:433` → `build_accepted_opportunity(…, intent.amount_in, …)` (`:824`, `:847`: `amount_in_wei: amount_in_wei_str`).
- `backend/searcher-rs/src/opportunity_emitter.rs:818-827` — si hay economics y `amount_in_wei == "0"`, **se estampa el literal `"1000000000000000000"`**.
- `backend/searcher-rs/src/orchestrator.rs:995-1018` — la rama `Sized` copia `gross`/`net` a la `Opportunity` pero **NUNCA** escribe `optimal_amount_in` en `amount_in_wei` (solo lo hacen las ramas de `size_optimizer.rs:1188` y `:1206`).
- `backend/api-server/src/routes/opportunities-live.ts:660` — `amount_in_wei: row.amount_in_wei` (passthrough crudo).
- `backend/api-server/src/simulation/computeSimulatedNet.ts:173-192` — **sí** normaliza por `token_in_decimals` (correcto), pero `:229-233` lo usa como **notional** de todos los costos.

**El cálculo a mano (card USDC, item real del payload):**

```
item: a0b869…/c02aaa…  UniswapV2×SushiSwap   rejection_reason=non_positive_profit
token_in = USDC (decimals = 6)   amount_in_wei = 1000000000000000000
price USDC = 0.9999350913168     gross = 48.11088298   net = -0.000011

amountInUsd = (1e18 / 10^6) * 0.9999350913168
            = 1e12 * 0.9999350913168
            = 999,935,091,316.80  USDC      ← coincide EXACTO con simulated_amount_in_usd del payload ✅
            (el operador reportó ~$999,827,448,922 con un precio USDC ligeramente distinto)

slippage_usd = 999,935,091,316.80 * 0.5      = 499,967,545,658.40  ← 0.5 = "0.5 %" leído como fracción
lp_fees_usd  = 999,935,091,316.80 * 0.003    =   2,999,805,273.95
flashloan    = 999,935,091,316.80 * 0.0009   =     899,941,582.19
failure_buf  = 999,935,091,316.80 * 0.0004   =     399,974,036.53
gas          = 250000 * gwei*1e9/1e18 * 2692.6889 =        0.673172225
ops          =                                            0.01
copied       = 48.11088298 * 0.5             =              24.05544149
relay        = max(48.11088298*0.05, 0.5)    =               2.405544149

Σ costos    = 504,267,266,530.10...
net         = 48.11088298 − Σ = −504,267,266,530.0955
```

→ **Coincide dígito a dígito con `simulated_net_profit_usd = -504267266530.0955` del payload.** El desglose real (`simulated_cost_breakdown`) confirma cada término: `slippage_usd=499967545658.4`, `lp_fees_usd=2999805273.9504004`, `flashloan_fee_usd=899941582.18512`, `failure_buffer_usd=399974036.52672005`, `gas_usd=0.6731722250000001`, `ops_overhead_usd=0.01`, `relay_fee_usd=2.4055441490000002`.

**Consecuencia:** el swap descrito es de **1,000,000,000,000 USDC**, no de 1e18 wei de un token de 6 decimales. Todo el ladder de la card es aritméticamente consistente **con un probe imposible**.

**Fix mínimo (2 líneas, no lo aplico — soy auditor):**
```rust
// dex_engine.rs:242 y :279 — normalizar el probe a un notional USD constante
let decimals = canonical_token_decimals(Some(intent_token_in_here));   // ya existe el helper (:705)
let probe_amount = U256::from(10u128).pow(U256::from(decimals));       // 1 unidad entera del token
// o mejor: probe_usd_target / price_usd, redondeado a 10^decimals
```
Complementario: `orchestrator.rs:995-1018` debe hacer `c.opportunity.amount_in_wei = s.optimal_amount_in.to_string()` en la rama `Sized` (hoy solo lo hace el kernel en `size_optimizer.rs:1188,1206`), y `opportunity_emitter.rs:827` no debería fabricar `1e18` sin decimals.

---

### B2 — CRÍTICA — `max_slippage_pct = 0.5` ⇒ 50 % del notional (debería ser 0.5 %)

**Archivo:línea:**
- Live: `/api/trading-config` → `"max_slippage_pct": 0.5`.
- `backend/api-server/src/routes/trading-config.ts:114` — `z.number().min(0).max(50)` y `:172` `max_slippage_pct: z.number().min(0).max(50)`: la columna es **porcentaje** (0–50).
- `backend/api-server/src/simulation/computeSimulatedNet.ts:254` — `const slippage_usd = amountInUsdVal * cfg.max_slippage_pct;` ← la usa como **fracción**.
- Contraste Rust: `backend/math-engine/src/roi_engine.rs:256-258,353-358` — la convención canónica de `max_slippage_pct` es **fracción** (`0.001` = 0.1 %, `0.005` = 0.5 %; test `zero_price_impact_uses_max_slippage_proxy` afirma `slippage_expected_pct == 0.001`). El default Rust también es `0.5` (`shared-rs/src/trading_config.rs:764`), o sea **el desajuste unidad↔columna es estructural**.

**El cálculo a mano (card WETH/DAI — la que el operador vio como SIM −$1,359.39):**

Config viva: `lp_fee_default_pct=0.003`, `max_slippage_pct=0.5`, `failure_risk_buffer_pct=0.0004`, `flashloan_fee_pct=0.0009`, `p_copied_max=0.5`, `capital_cost_rate_annual_pct=0`, `ops_overhead_usd_per_attempt=0.01`, `gas_estimate_units=250000`, `base_token_price_usd=2692.6889`.

```
base rate = flashloan + slippage + failure_buffer + lp_fee + capital_cost
          = 0.0009 + 0.5 + 0.0004 + 0.003 + 0 = 0.5043      ← 50.43 % del notional

item: c02aaa…/dac17f…  UniswapV2×SushiSwap
amount_in_wei = 1e18 ; WETH decimals = 18 ; WETH = 2692.6889 (base_token_price_usd)
amountInUsd  = 2692.6889   ≡  simulated_amount_in_usd = 2692.6889  ✅
gross        = 25.22783426 ≡  expected_profit_usd                    ✅
varCosts     = 2692.6889 * 0.5043            = 1357.9200…
copied       = 25.22783426 * 0.5             =    12.6139…
relay        = max(25.22783426*0.05, 0.5)    =     1.2614
gas          = 0.673172225 + ops 0.01
net          = 25.22783426 − 1357.9200 − 12.6139 − 1.2614 − 0.6732 − 0.01
             = −1347.2522…
```

→ **`simulated_net_profit_usd = -1347.2536590779998` del payload.** El 99.99 % de la pérdida es **el término de slippage** (`2692.6889 × 0.5 = 1346.34`). Con la lectura correcta (0.5 % = 0.005) ese término sería **$13.46** y el net sería ≈ **−$14.6**, no −$1,347.

**Verificación independiente con las otras 2 cards** (misma fórmula, reproduce las 3 exactamente):
- DAI/USDC: `amountInUsd = 1e18/1e18 × 0.99982718 = 0.99982718` ≡ payload ✅; `net = 0.00035593 − 0.99982718×0.5043 − 0.00035593×0.5 − 0.5(relay floor) − 0.6832 = −1.6872` ≡ payload `-1.687207106874` ✅.
- USDC: ya demostrado en B1.

**Verificado correcto:** `roi_pct = net / amount_in_usd` (`computeSimulatedNet.ts:293`). Payload: `-1347.2535/2692.6889 = -50.0337 %` = `simulated_roi_pct=-50.033765841943335` ✅ exacto.

**Fix mínimo:** en `computeSimulatedNet.ts:254` usar `cfg.max_slippage_pct / 100`, y en `varCostRateFromCfg` (`:378-386`) igual — **pero verificar antes el resto del repo**, porque `roi_engine.rs` trata el mismo campo como fracción; la corrección debe hacerse en **un solo lado** (probablemente normalizar en `tradingConfigSnapshot.parseSnapshot` y documentar la unidad en la columna). Alternativa inmediata operativa: poner `max_slippage_pct = 0.005` en el admin (pero eso rompería el proxy del lado Rust si algún camino lo lee como fracción… que es exactamente lo que sí hace). **Este es el hallazgo que más urge.**

---

### B3 — ALTA — GROSS es el spread de **dos swaps independientes**, no la ganancia del ciclo (output vs delta)

**Archivo:línea:**
- `backend/searcher-rs/src/engines/dex_engine.rs:342-345`: `out_a = v2_amount_out(probe, r_in_a, r_out_a, fee_a)`, `out_b = v2_amount_out(probe, r_in_b, r_out_b, fee_b)` — **ambos desde `probe`, independientes**.
- `:346-350`: `spread = |out_a − out_b|` (`saturating_sub` por lado).
- `:366-371`: `compute_gross_usd(&gross_spread_units, …, intent.legs.first().map(|l| l.token_out))`.
- `:689-721` `compute_gross_usd`: `spread_f64 = raw/10^decimals(token_out)`; `price = canonical_token_price_usd(token_out, …)`; `Some(spread_f64 * price)`.
- El comentario `:680-684` documenta el bug histórico $69M y el escalado por decimals — **verificado correcto**: `:705` usa `canonical_token_decimals(token_out)` y `:718-719` usa el precio **del token de denominación**, no el de WETH. La refutación de la hipótesis "se mezcla output vs delta por decimals" está en el punto 6.b.

**Por qué sigue siendo "output vs delta" conceptualmente:** el ciclo real es `x →(A) out_a →(B) out_b'` con `out_b' = v2_amount_out(out_a, r_in_b, r_out_b, fee_b)`. El código compara **dos salidas de primer hop**, no el delta del ciclo. Diferencia de primer orden: `gross_mostrado ≈ 2 × gross_real`.

**El cálculo a mano con reserves de ejemplo (WETH/USDT, UniswapV2 vs SushiSwap):**
```
probe x = 1 WETH = 1e18
Pool A UniswapV2 WETH/USDT: r_in = 3,000 WETH ; r_out = 8,070,000 USDT ; fee 30 bps
   out_a = 0.997*1*8,070,000 / (3,000 + 0.997*1) = 8,045,790 / 3,000.997 = 2,681.04 USDT
Pool B SushiSwap  WETH/USDT: r_in = 3,000 WETH ; r_out = 8,100,000 USDT ; fee 30 bps
   out_b = 0.997*1*8,100,000 / 3,000.997                              = 2,691.02 USDT

spread (lo que muestra GROSS) = 2,691.02 − 2,681.04 = 9.98 USDT  ≈ $9.98
pero si el ciclo es 1 WETH →(A) 2,681.04 USDT →(B) WETH:
   out_b' = 0.997*2,681.04*3,000 / (8,100,000 + 0.997*2,681.04)
          = 8,019,769 / 8,102,672.8 = 0.9898 WETH
   delta del ciclo = 0.9898 − 1 = −0.0102 WETH = −$27.5   ← NEGATIVO
```
**Lectura:** el GROSS de la card **no es la ganancia del ciclo**; es la diferencia de dos cotizaciones de un solo hop (imita "cuánto más rinde A que B para el mismo input"), lo que **sobreestima** porque el hop barato nunca se ejecuta dos veces gratis: el ciclo paga **dos** fees (0.3 % cada una = 0.6 %) y **dos** impactos.

**Evidencia empírica de que el signo es falso:** las 3 cards con números tienen `net_expected_profit_usd` **negativo** (−0.000011, −0.000014, −0.000008) y `rejection_reason=non_positive_profit` — el kernel de sizing (`size_optimizer.rs:1083-1100`, que **sí** encadena: `out_b = v2_amount_out(out_a, …)` en `:1110`) encuentra profit ≤ 0 donde el engine había reportado +$48.11 y +$25.23. La discrepancia de signo es la prueba de que `compute_gross_usd` mide otra cosa.

**Fix mínimo:** en `dex_engine.rs:342-351`, encadenar:
```rust
let out_a = amm_math::v2_amount_out(probe_amount, r_in_a, r_out_a, fee_a);
let out_b = amm_math::v2_amount_out(out_a,      r_in_b, r_out_b, fee_b);   // ← out_a, no probe
let delta  = out_b.saturating_sub(probe_amount);                            // ← delta del ciclo
```
(y elegir la dirección rentable de las dos, no el valor absoluto). Nota: el comentario `:377-381` invoca "honest equilibrium market reading" para justificar no pre-rechazar; eso es correcto para el **spread**, pero el campo se llama `expected_profit_usd` y la card lo rotula GROSS/**ganancia**.

---

### B4 — ALTA — "Gross out (AMM)" = capital + gross (muestra la salida, no la ganancia)

**Archivo:línea:**
- `frontend/components/opportunities/exchange/OpportunityExchangeCard.tsx:257-259`:
  ```ts
  // Gross out (AMM): capital + gross when both exist, else "—".
  const grossOutUsd = capitalInUsd != null && grossUsd != null ? capitalInUsd + grossUsd : null;
  ```
  con `capitalInUsd = opp.simulated_amount_in_usd ?? tgt.suggested_amount_in_usd` (`:245-247`) y `grossUsd = opp.expected_profit_usd` (`:249`).
- Render: `:439-440` → `<span>Gross out (AMM)</span><span>{usdAmount(grossOutUsd)}</span>`.
- **Esta es la card desplegada en `/opportunities`**: `usdAmount` renderiza `"—"` cuando es null (por eso las 29 cards sin `expected_profit_usd` muestran "no computado"), e imprime "$2,692.69" en el mismo bloque (`:410`, `:414` "Monto a invertir / Monto a prestar") — exactamente el layout que describe el operador (labels en español).

**Qué campo del payload lo alimenta:** `items[].simulated_amount_in_usd` (capital) + `items[].expected_profit_usd` (gross).

**Por qué puede ser $7.24M:** es `capital + gross`. La card USDC tiene `simulated_amount_in_usd = 999,935,091,316.80`; con **cualquier** gross positivo ≥ $0.005 el resultado es ≥ $1e12. Para que salga **$7.24M** exactamente hace falta `expected_profit_usd` en el rango de **$4.5M–$7.2M con capital ≈ $2.7k** (p.ej. `capitalInUsd=2692.69`, `gross=$7,241,857`), o bien que en el momento de la captura del operador la card de USDC tuviera `capital` de otro orden. **No pude reproducir el $7.24M literal** (ver "NO CONFIRMADO"); lo que **sí** queda demostrado es que el rótulo es el equivocado: ese campo es **salida total**, no ganancia, y con el probe de B1 puede alcanzar cualquier magnitud.

**La otra card (`OpportunityTradeCard.tsx`, la del repo)** tiene el bug inverso y más de forma que de fondo:
- `:183` `const grossUsd = opp.expected_profit_usd ?? null;` → el ledger "Gross out (AMM spread)" (`:601-608`) pinta el **gross real** (bien), pero
- `:181` `const capitalInUsd = opp.simulated_amount_in_usd ?? null;` → el renglón "Flash loan in (TLS)" (`:573-577`) hereda el notional envenenado de B1.

**Fix mínimo:** renombrar/separar en `OpportunityExchangeCard.tsx:257-259`:
`grossOutUsd` → `endValueUsd` (etiqueta "Salida total (AMM)", `capital + gross`) **y** añadir un renglón propio `Ganancia bruta (AMM spread)` con `grossUsd` solo. Así el operador ve las dos magnitudes sin confundirlas.

---

### B5 — MEDIA-ALTA — Precios imposibles en el PriceBus (AAVE 10^9×, SNX 10^6×)

**Evidencia (payload real `/api/prices/live?chain_id=1`, 385 entradas):**
```
AAVE   161,339,420.31     (real ≈ $161–300)      → ×10^9 aprox (161.33942 × 1e9 = 161,339,420.31)
SNX        272,885.72     (real ≈ $0.27)         → ×10^6 exacto (0.27288572 × 1e6)
WBTC        84,165.36     (real ≈ $100–120k)     → ×0.75, no concluyente
WETH         2,692.778863726  ✅   USDC 0.9999350913168 ✅   DAI 0.99982718 ✅
LINK 14.31 ✅  UNI 9.65 ✅  MKR 1770.054 ✅  CRV 0.3587 ✅  PEPE 0.00000445 ✅
```
269 de 385 precios están en rango plausible; **2 son imposibles**. Los mismos valores aparecen en `/api/trading-config.token_prices_usd` (`AAVE:161339420.31`), es decir el config sirve el mapa de Redis.

**Impacto en la matemática de la card:**
- `compute_gross_usd` (`dex_engine.rs:718-719`) multiplica el spread del `token_out` por ese precio ⇒ cualquier card con AAVE/SNX de `token_out` reporta gross ×10^6–10^9.
- `size_optimizer.rs:2034` (`resolve_token_price`) y `clamp_to_cap_wei`: con `AAVE=$161M`, `cap_usd=$1000` produce `x_lo/x_hi` de **6.2e-6 AAVE** = 6.2e12 wei ⇒ el optimizador busca el óptimo en una región económicamente vacía.

**Dónde nace (3 writers al MISMO hash `arbx:token_prices:<chain>`, last-writer-wins sin arbitraje de autoridad):**
1. `backend/searcher-rs/src/workers/price_worker.rs:1294-1314` (`persist_prices`, HSET + EXPIRE + PUBLISH). Chainlink: `:1147-1166` `raw / 10f64.powi(decimals)` con `decimals` de la tabla `price_oracles` — **un `decimals` mal sembrado en esa tabla produce exactamente este bug** y no hay validación de rango.
2. `backend/token-enricher/src/dexscreener.rs:482-540` (HSET propio) — escribe `priceUsd` del **par**, keyed por símbolo.
3. `backend/token-enricher/src/geckoterminal_tier.rs:561-605` — igual, `price_usd` del **pool**.
El doc lo admite: `geckoterminal_tier.rs:21` *"Both are independent, env-gated WRITERS into `arbx:token_prices` (last-writer-…)"*.

**NO CONFIRMADO (qué falta):** no pude leer `price_oracles.decimals` ni el `TokenMeta` de AAVE/SNX (SSH bloqueado). Para cerrar el root cause hace falta **una** de estas dos cosas: `SELECT token_address, oracle_address, decimals FROM price_oracles WHERE kind='chainlink'` filtrado por AAVE/SNX, o el log de arranque del `price_worker` con `chainlink_priced` para ver qué tier escribió AAVE por última vez.

**Fix mínimo:** validar en `price_worker.rs:1166-1169` un rango de plausibilidad por símbolo conocido (banda ±90 % contra el anchor del bus / contra el precio previo del hash) y **rechazar la escritura** en vez de persistir (R8). En paralelo, añadir el símbolo a la lista de "no escribible por el tier de pool" del enricher.

---

### B6 — MEDIA — El ledger por hop existe en el código pero es **inalcanzable** en las 32 cards

Detalle completo en la sección **PER-LEG LEDGER**.

---

## PER-LEG LEDGER (el pedido clave del operador)

### (a) ¿El dato existe y se persiste? — **SÍ existe, NO se persiste en producción**

| Capa | Estado | file:line |
|---|---|---|
| Cómputo del ledger | **SÍ** — kernel 2-leg lo emite | `size_optimizer.rs:1130-1131` (`leg_amounts_in = [amount_in, out_a]`, `leg_amounts_out = [out_a, out_b]`), también `:1441-1442`; kernel triangular `:860-874` |
| Transporte | **SÍ** | `SizedCandidate.leg_amounts_in/out` — `size_optimizer.rs:281-282`; hilo en `orchestrator.rs:1014-1018` y `cartridge_boot.rs:2346-2348` |
| Adjuntado al route_metadata | **SÍ** | `shared-rs/src/candidates.rs:203-229` `attach_leg_ledger` (all-or-nothing, exige `len == dex_adapters.len()` y `token_addresses.len() == hops+1`), llamado en `orchestrator.rs:1186-1187` y `cartridge_boot.rs:2471` |
| Serialización | **SÍ** | `candidates.rs:148-169` — `leg_amounts_in`, `leg_amounts_out`, `leg_zero_for_one`, `skip_serializing_if = None` |
| **¿Persistido en prod?** | **NO — 0/32 items** | payload: `route_metadata = {"decimals":{"map":{}},"dex_adapters":[…],"pool_addresses":[…],"token_addresses":[…]}`, sin campos de ledger |

**Por qué no se persiste (causa exacta):** el ledger solo se construye en la rama **después** de los gates de profit:

```
size_optimizer.rs:1093   if profit_wei <= 0      → Rejected(NonPositiveProfit, Some(usd))   ← SIN ledger
size_optimizer.rs:1117   if profit_at_clamped<=0 → Rejected(NonPositiveProfit, Some(usd))   ← SIN ledger
size_optimizer.rs:1127-1131 → leg_amounts_in/out  ← SOLO aquí
orchestrator.rs:1020-1060  rama Rejected → (c, None, None)   ← leg_ledger = None explícito
orchestrator.rs:1059                  (c, None, None)
```
Las 32 cards tienen `rejection_reason` poblado (`v3_quote_unavailable` ×26, `single_pool_no_spread` ×3, `non_positive_profit` ×3) ⇒ **todas** caen en la rama `Rejected` ⇒ `leg_ledger=None` ⇒ `attach_leg_ledger` nunca corre.
Nota adicional: aun con profit positivo, la rama `Sized` con net ≤ 0 **sí** adjuntaría el ledger (`size_optimizer.rs:1185-1200` devuelve `Sized` con `net_negative:true` y `leg_amounts_in: Some(...)`), pero su comentario homólogo en el camino Kelly lo anula (`:710-715`, `:729-732`).

### (b) ¿Llega al payload del api-server? — **SÍ, passthrough completo, sin filtrar**
- `backend/api-server/src/routes/opportunities-live.ts:701-705`: `route_metadata: … Object.keys(row.route_metadata).length > 0 ? row.route_metadata : null` — pasa el JSONB **verbatim**.
- `:643-659` (`leg_symbols`) y `:854-856` leen `token_addresses` del mismo objeto.
- **No hay ningún mapeo que descarte `leg_amounts_in`** (grep de `leg_amounts_in` en `backend/api-server/src` → **0 coincidencias**). El dato llega si el searcher lo escribe.

### (c) ¿Qué falta para que la card lo muestre por hop?

**Backend (2 bloqueadores reales):**
1. **Emitir el ledger también en la rama `Rejected`** — hoy `orchestrator.rs:1059` devuelve `(c, None, None)`; el kernel ya tiene `out_a/out_b` calculados en `size_optimizer.rs:1093-1100` (camino `profit_wei <= 0`) y puede devolverlos en el `Rejected`. Sin esto, **ninguna** card rechazada (que son el 100 % de la producción actual) tendrá ledger por hop.
2. **`route_metadata.decimals.map` está VACÍO en 32/32 items** (`{"map":{}}`, comprobado). Sin decimals por token, el cliente **no puede** convertir los wei por hop a USD. `attach_leg_ledger` no rellena `decimals` (`candidates.rs:203-229`).

**Frontend (2 bloqueadores):**
3. **El único consumidor es el panel de detalle, no la card**: `deriveLegLedger` está implementado y completo (`frontend/lib/store/types.ts:721-749`, con `cycle_delta_wei` en la pata de cierre `:746-749` y validación `isUnsignedWei` `:556-559`), pero solo lo importa `frontend/components/opportunities/OpportunityDetailTabs.tsx:43,243`. **Ni `OpportunityExchangeCard.tsx` ni `OpportunityTradeCard.tsx` lo importan** (grep `deriveLegLedger` en `*.tsx` → 1 archivo). El ladder de la card pinta los hops con `value={null}` (trade card `:578-587`) o directamente no pinta importe por hop.
4. **La card no recibe los decimals de la ruta** (el `RouteMetadataWire` los expone en `types.ts:127-138`, pero el render de la card usa `token_in_info/token_out_info` — que sí traen `decimals` — solo para los extremos, no para los intermedios).

**Cadena mínima para "cada hop muestra cuánto gana":** `size_optimizer` emite ledger en `Rejected` → `attach_leg_ledger` + `decimals` poblado → `orchestrator` lo adjunta → `api-server` ya lo pasa → card llama `deriveLegLedger(opp)` y pinta `amount_out_wei[i] − amount_in_wei[i]` convertido con `decimals[token]`.

---

## HOPS 2-7 — estado por motor + constantes

| Motor / capa | Límite real | file:line |
|---|---|---|
| Discovery multi-hop (ciclos 2–7) | **2..=7** (`hop_mask: 0b11_1111`) | `route_discovery/multi_hop_search.rs:113-127`, clamp `:157-158` (`max_hops.min(7)`), `:167` |
| Knobs canónicos | **`max_hops: 7`**, validación `2..=7` | `canonical_knobs.rs:52,173,275,388-391`, env `ARBX_KNOB_MAX_HOPS`; test `:822` `assert_eq!(k.max_hops, 7)` |
| Route scanner worker | **`DEFAULT_MAX_HOPS = 7`** | `workers/route_scanner_worker.rs:77-78,256` (`clamp(2,7)`) |
| Route-discovery worker | `clamp(2, 7)` desde `finder.max_depth` | `route_discovery/route_discovery_worker.rs:399-400`; `discovery_workload.rs:266` |
| `agent_graph` (DFS agente) | `2..=7` | `agent_graph.rs:39,57,92` |
| Hop masks por estrategia | hi = `min(max_hops, 7)` | `strategy_hop_mask.rs:313-328` |
| **Config de despliegue (la que manda en prod)** | **`max_depth: 3`** ⇒ solo ciclos de 2-3 | **`backend/searcher-rs/config/strategies/route_applicability.yaml:25`** (con `min_liquidity_hint: 1.0`, `max_pools_per_pair: 8`) |
| `triangular_worker` | **3 hops hard-coded** (3 ciclos V2 + `V3_CYCLES`, 3 hops cada uno) | `workers/triangular_worker.rs:174-176` (`MVP_CYCLES`), `:203` (`V3_CYCLES`), `:999` (`hop_outs.len() != 3`), `:1424-1428`, `:1650`, `:1788` (`MAX_PHASES = 3`); ledger = `None` en `:612-613` |
| `spanning_tree_engine` | N hops por topología; **no emite ledger** | `engines/spanning_tree_engine.rs:499,532` (usa `cycle.optimal_amount_in_wei`, sin `leg_amounts_*`) |
| `size_optimizer` (sizing real) | **kernel genérico en N = 2..7** (`size_triangular_with_reason` itera TODOS los legs y emite el ledger por hop; el dispatch lo alcanza para cualquier `legs.len() != 2`) — ⚠️ **corregido, ver §HOPS-CORRECCIÓN** | `size_optimizer.rs:519-528` (dispatch), `:765-783` (guard 2..=7), `:860-874` (ledger N-leg); test `triangular_4leg_sizes_all_legs_no_truncation` |
| `dex_engine` (esto es lo que produce TODAS las cards de prod) | **2 pools = 2 hops**, siempre | `engines/dex_engine.rs:265-268` (`for i, for j in (i+1)`), `:885` (`legs: vec![leg_a, leg_b]`) |
| `cartridge_boot.rs` (un punto) | `max_hops: 4` hard-codeado ⇒ **5-7 hops inrepresentables dentro del runtime de cartuchos** | `cartridge_boot.rs:1429` (original) — **corregido en HOPS-CARD-05** (`canonical_max_hops()`) |

**Veredicto HOPS 2-7 (original, PARCIALMENTE ERRADO — leer la corrección de abajo):** el **código soporta 2–7** (detección: `multi_hop_search`, knobs, scanner). La **producción no**: `route_applicability.yaml:25 max_depth: 3` limita el discovery a 3, y el **sizing solo existe para 2 y 3 hops** ⇒ un ciclo de 4–7 detectado no puede pasar por `SizeOptimizer` (no hay kernel) y por tanto **nunca** produce una card con números por hop. Y los 32 items del endpoint son **todos de 2 hops** (`dex_engine`). Constantes citadas arriba.

### HOPS-CORRECCIÓN (2026-09-26, verificación posterior contra el código y contra producción)

Tres afirmaciones del veredicto original eran **falsas**; se corrigen con evidencia reproducible:

1. **"No hay kernel de sizing para 4-7 hops" — FALSO.** `size_triangular_with_reason` es genérico en N: valida `legs.len()` en `2..=7` (`size_optimizer.rs:778-783`), construye `hop_reserves` para **todos** los legs (`:786-827`) y emite el ledger por hop cuando `leg_outputs.len() == legs.len()` (`:860-874`). El dispatch lo alcanza para cualquier `legs.len() != 2` (`:519-528`, comentario WO-13). Evidencia de test: `triangular_4leg_sizes_all_legs_no_truncation` (**4 legs**) verde; `evaluate_cycle_detailed` + `cycle_profit_with_ledger` (`workers/triangular_worker.rs:277-296`, `:887-888`) tampoco asumen 3 hops.
2. **"`route_applicability.yaml:25 max_depth: 3` es la config que manda en prod" — FALSO.** La precedencia real es `ARBX_KNOB_MAX_HOPS` > `ARBX_ROUTE_DISCOVERY_MAX_DEPTH` > yaml (`route_discovery_worker.rs:126-132`), y **producción tiene `ARBX_ROUTE_DISCOVERY_MAX_DEPTH=7`** (verificado en `/opt/arbitragex-v2/.env`). Medición en vivo: `routes_found=500` por tick (el cap `max_routes_per_tick`), **cero** ticks con `work_limited`/`capped` en 20 min, `edges_built=378`. Es decir: el DFS profundo **ya corre a profundidad 7** en producción sin starvation.
3. **"el sizing no existe, por eso no hay cards" — FALSO por la razón correcta.** El motivo medido de que **las 571.517 filas de las últimas 3 h sean todas de 2 hops** (query directa a PG, `jsonb_array_length(route_metadata->'dex_adapters')`) es que **el único emisor es `dex_engine`**, que enumera **pares de pools** (2 hops por construcción). Los candidatos de route-discovery (hasta 7 hops, 500/tick) **no se emiten**: en 30 min de logs, `route_intent.emitted = 0` (`dispatch_enabled`/`plan_dispatch` no los despacha), y `spawn_cartridge_eval` (`orchestrator.rs:267-298`) — el puente que SÍ usa el `SizeOptimizer` genérico y el emitter — solo se invoca para intents despachados.

**Gaps reales de HOPS (corregidos):**
- (a) **Puente discovery → emisión**: convertir los `RouteCandidate` de 3-7 hops (`route_discovery/types.rs:206-222`, que ya traen `tokens`/`pools`/`directions`/`fee_tiers`) en `StrategyCandidate` con `route_plan` y despacharlos al pipeline de sizing/emisión. Es el trabajo que hace aparecer cards de 3-7 hops con números por hop.
- (b) **`cartridge_boot.rs:1429 max_hops: 4`**: techo duro del DFS dentro del runtime de cartuchos ⇒ 5-7 hops inrepresentables allí. **Arreglado en HOPS-CARD-05** (`canonical_max_hops()`, knob canónico, clamp 2..=7).
- (c) **Defaults de despliegue inconsistentes**: el yaml decía 3 y el fallback embebido de `DiscoverySettings` decía 5, contra el `Max_Hops` canónico = 7. **Arreglado en HOPS-CARD-05** (una sola fuente + gate `shipped_discovery_depth_reaches_the_canonical_max_hops`).
- (d) `triangular_worker` escanea 3 ciclos V2 + `V3_CYCLES` (3 hops c/u) y su test usa `MAX_PHASES = 3`: es el barrido propio de ese worker, no el camino genérico (`multi_hop_search`). Sigue pendiente para N genérico.


---

## 6. ¿SON ARBITRAJES CORRECTOS? (2 casos reales reconstruidos)

### 6.a — USDC/WETH UniswapV2×SushiSwap — **el signo es plausible, la magnitud es imposible**
```
pools: 0x397ff1542f962076d0bfe58ea045ffa2d347aca0 (UniV2 USDC/WETH, ~$47M TVL real)
       0x1445f32d1a74872ba41f3d8cf4022e9996120b31 (PancakeSwap V3 — ¡ojo, no es SushiSwap!)
gross mostrado   = $48.11088298  (spread de 2 single-swaps con input = 1e12 USDC)
net canónico     = −$0.000011    reason=non_positive_profit
```
**Refutación de la hipótesis "es un arb real de $48":** un input de 1e12 USDC frente a $47M de liquidez es un swap de **21,000× la reserva**: `v2_amount_out(1e12 USDC, r_in=23,600 WETH, r_out=47e9 USDC, 30bps)` devuelve ≈ `0.997·47e9·1e12/(23,600+0.997·1e12)` ≈ `4.70e10 USDC` — el pool se satura y el "spread" de $48 es ruido numérico de un punto de operación absurdo. **Signo plausible (los dos pools difieren), magnitud imposible.** Coherente con `net_expected_profit_usd = −0.000011` del kernel, que **sí** encadena.

### 6.b — WETH/DAI UniswapV2×SushiSwap — **negativo y correcto en orden de magnitud**
```
pools: 0x397ff1542f962076d0bfe58ea045ffa2d347aca0 (UniV2) ; SushiSwap WETH/DAI
input = 1 WETH ($2,692.6889)  gross mostrado = $25.22783426  net = −$0.000008  reason=non_positive_profit
prueba de encadenamiento (lo que el kernel SÍ hace, size_optimizer.rs:1109-1110):
   out_a      = v2_amount_out(x, rin_a, rout_a, 30)
   out_b      = v2_amount_out(out_a, rin_b, rout_b, 30)
   profit     = out_b − x  →  ≤ 0  (de ahí el sello de −8e-6 USD)
dos fees del 0.3 % (0.6 % round-trip) + gas ≈ 0.673 USD:
   para x = 1 WETH el ciclo necesita un desvío > 0.6 % entre pools para ser positivo.
   0.6 % de $2,692 = $16.2 ⇒ un gross real de $25 (0.94 %) sería rentable… pero el kernel
   devuelve ≤ 0 ⇒ las reservas implican un desvío < 0.6 % o el gross del engine
   NO es el delta del ciclo (verificado en B3: el engine no encadena).
```
**Verificado correcto:** la conversión de unidades de `compute_gross_usd` (`dex_engine.rs:705-720`) — usa `canonical_token_decimals(token_out)` y el precio del token de denominación (no el de WETH). La hipótesis del operador "se mezcla output vs delta por decimals" es **FALSA en la conversión a USD** (esa parte está bien desde el fix `$69M` documentado en `:679-684`) y **VERDADERA en la semántica** (B3: el gross es el spread de dos hops independientes, no el delta del ciclo).

**Lo que falta para reconstruir al 100 %:** las reservas reales de los pools y sus `fee_bps` (`arbx:pool_index:1:*` / `reserves` en Redis, o `pool_snapshots`/`pools` en PG). Sin SSH no pude leerlas: **el dato que falta es `(r0, r1, fee_bps)` de los 2 pools por card**. Con eso el cálculo se cierra al wei.

---

## NO CONFIRMADO (y qué dato haría falta)

1. **El valor exacto "$7.24M" de Gross out**: no lo reproduje (payload actual: si el gross es positivo y el capital es $2.7k, `capital+gross` sería ~$2.7k; si la card era la de USDC, sería ~$1e12). Falta: la captura/payload del momento exacto en que el operador lo vio (el endpoint rota en minutos) o el `trace_id` de esa card para leer el row histórico en PG.
2. **El "$2,863.25" de GROSS**: aritméticamente compatible con `capital($2,692.69 a precio actual) + gross`, pero el gross exacto mostrado (≈$170) no está en el payload actual (hoy esa card muestra gross=$25.23). Falta lo mismo: el snapshot de la card.
3. **Root cause exacto de AAVE/SNX**: no pude leer `price_oracles.decimals` ni el hash de Redis en el VPS (SSH `arbx` falla con exit 1 y sin salida desde este host). Falta: `SELECT token_address, oracle_address, decimals FROM price_oracles WHERE kind='chainlink'` o el log `price_worker.chainlink_priced`.
4. **Reservas reales** de los pools de los 3 items con economía (para cerrar 6.a/6.b al wei).
5. **`v3_quote_unavailable` ×26**: confirmé que es la causa registrada del "no computado", pero **no** verifiqué si el projector V3 falla por RPC/circuit-breaker o por catálogo de fees (`empty_v3_fee_catalog`). Requiere logs del searcher (`orchestrator.rs:390-402` y `state_projector`).
6. **`relay_fee_usd` / `capital_cost_usd = 0`**: verificados en el desglose; `capital_cost_rate_annual_pct=0` es decisión del operador, no bug.

---

## Lo que está BIEN CALCULADO (refutaciones explícitas)

- **Escalado por decimals en `compute_gross_usd`** (`dex_engine.rs:702-720`): correcto — divide por `10^decimals(token_out)` y multiplica por el precio del **token de denominación**. La nota del operador "el escalado por decimals mezcla output vs delta" **no aplica a esta función**.
- **`amountInUsd` en el simulador** (`computeSimulatedNet.ts:173-192`): correcto (BigInt + `10^decimals`), reproduce exacto los 3 valores del payload. El problema es el **dato** que recibe (B1), no la conversión.
- **La aritmética del simulador** (`computeSimulatedNet.ts:281-291`): reproduce **dígito a dígito** las 3 cards (`-504267266530.0955`, `-1.687207106874`, `-1347.2536590779998`) y los 3 `roi_pct` (`-50.4299…`, `-168.749…`, `-50.0337…`). No hay error de cómputo: hay **dos entradas envenenadas** (probe B1, slippage B2).
- **`v2_amount_out`** (`amm_math.rs:96`) y su encadenamiento en el kernel (`size_optimizer.rs:1109-1110`): correcto — `out_b = v2_amount_out(out_a, …)`.
- **El estado "no computado"** en 29/32 cards **no es un bug**: R8/RULE 00 funcionando, con razón explícita por fila (`v3_quote_unavailable` 26, `single_pool_no_spread` 3). Ninguna card inventa números.
- **`deriveLegLedger`** (`frontend/lib/store/types.ts:721-759`): implementación correcta y honesta (all-or-nothing, BigInt-exact, `cycle_delta_wei` solo en la pata de cierre). El problema es que nada la alimenta en producción y que solo el panel de detalle la usa.
- **`attach_leg_ledger`** (`shared-rs/src/candidates.rs:203-229`): correcto — rechaza ledger desalineado en vez de fabricar enlaces.
- **Los hops de las 32 cards son 2** (todos `dex_engine`, `legs: vec![leg_a, leg_b]`), coherente con el payload (3 `token_addresses`).

---

## Orden de arreglo sugerido (por impacto económico)

1. **B2** (`max_slippage_pct`/100) — 1 línea; sin esto **ninguna** ruta puede ser net-positive (el piso es −50 %).
2. **B1** (probe por decimals + `amount_in_wei` desde `optimal_amount_in` en la rama `Sized`) — elimina el $1T y el −$504B.
3. **B3** (encadenar el gross del engine) — hace que GROSS signifique lo que dice.
4. **B4** (separar "salida total" de "ganancia bruta" en la card).
5. **B5** (gate de plausibilidad de precios en los 3 writers de `arbx:token_prices`).
6. **B6** (ledger en la rama `Rejected` + `decimals` en `route_metadata` + render por hop en la card) — el pedido del operador.
