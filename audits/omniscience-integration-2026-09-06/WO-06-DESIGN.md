# WO-06 — DISEÑO: Filtro anti-flood estructural en el searcher, ANTES de la emisión

- **WO:** WO-06 · kind: **design** (READ-ONLY: este documento propone diffs; NO se editó código de producción).
- **Agente:** strategy-architect (Gang Omniscience). Fecha: 2026-09-06.
- **Charter:** roadmap `00-PREDATOR-ROADMAP.md` §3 P1-2 (líneas 118-120) — "Filtro anti-flood estructural en el searcher, ANTES de la emisión … criterio DINÁMICO (el flood rota: PEPE 25.8% es nuevo; hardcodear XEN+AGLD nace obsoleto; RULE 00)".
- **Evidencia base:** `04-searcher-pipeline-CROSS.md` §"Evidencia propia" ítems 2 y 7: AGLD 37.2% + XEN 34.0% + PEPE 25.8% = **97.0%** del volumen 24h (57.981 detecciones) en **self-pairs** `token_in==token_out`, amplificados **×28** por la matriz de cartuchos (~769 eventos de mercado × ~28 cartuchos `mev_01_*`); **100% rejected 15 días** (última no-rechazada 2026-08-22 16:52:25Z).
- **Presupuesto HTTP dominio público usado por este agente:** 0/5 (toda la evidencia es lectura de código local + reportes del directorio).
- **Reglas:** RULE 00 (cero tokens hardcodeados en lógica; los 3 addresses viven SOLO en fixtures de test) · §32/§33 (diseño read-only; VPS intocado) · §34.3 (no se toca el terminus; esto es aguas arriba de cualquier terminus) · NO-GIT · lexicon OMEGA.

---

## 0. Diagnóstico: dónde nace, dónde se amplifica y dónde muere hoy el flood

Ruta completa del flood (cada paso con ancla file:line verificada en esta sesión):

1. **Nace en el decoder.** `route_decoder.rs:249-266` construye las legs con `path.windows(2)` (`token_in: w[0], token_out: w[1]`). Un tx spam con `path=[AGLD, AGLD]` produce una leg con `token_in == token_out`. En el path nativo legacy, `patterns.rs:29-50` (`build_dex_arb_candidate`) toma `swap.token_in = path[0]` y `swap.token_out = path.last()` → mismo self-pair. **Ninguno de los dos constructores tiene check de self-pair.**
2. **Se amplifica ×28 en la matriz de cartuchos.** `cartridge_boot.rs::active_evaluate_and_emit` — loop `for (cartridge_id, …) in pertinent` (`cartridge_boot.rs:1085`) evalúa ~28 cartuchos `mev_01_*` por intent; cada positivo construye su propio `Opportunity` (`cartridge_boot.rs:1153-1183`) con `token_in = first_leg.token_in`, `token_out = last_leg.token_out` y `pair_symbol = "0xAGLD/0xAGLD"` (`cartridge_boot.rs:1161`). Además, ANTES del check de oportunidad, cada evaluación emite un outcome de route-discovery (`emit_shadow_outcome`, `cartridge_boot.rs:1096-1104`) — el eco ×28 también alimenta `arbx:route_discovery:outcomes` (el dataset de 42 GB / 86.1M filas, roadmap D-2).
3. **Muere demasiado tarde, con escritura completa.** El rechazo ocurre en el `ConfigAwareEvaluator` del spine (`cartridge_boot.rs:1649-1657` → `TokenNotAllowed:<addr>`) — DESPUÉS del SizeOptimizer (`cartridge_boot.rs:1399-1477`), después de las lecturas Redis de identity/precios (`cartridge_boot.rs:1054-1073`), y el rechazo **se persiste a PG y se publica al stream**: `opportunity_emitter.rs::emit_rejected` (`:349-423`) hace `try_insert_pg_with_route` (`:409`) + `publisher::publish` XADD a `arbx:opps:detected` (`:413`).
4. **Los rows rechazados NUNCA se deduplican — por diseño.** `opportunity_emitter.rs:337`: "Rejected rows are always persisted (no dedup check)". En el path nativo, `scanner.rs:2581-2583`: "Rejected rows … skip this check" — el `OppDedup` (`dedup.rs:96-127`, clave route+5min+$0.10-bucket) solo aplica a filas que pasaron TODOS los gates. El path de cartuchos ni siquiera usa `OppDedup`: su `route_fingerprint` es `cartridge_id:tx_hash:intra_tx_index` (`cartridge_boot.rs:1233-1236`) — único por construcción, dedupea nada.
5. **El knob de liquidez existe pero no está cableado.** `min_pool_liquidity_usd` (default 150.000 USD, env `ARBX_KNOB_MIN_POOL_LIQUIDITY_USD`) está declarado en `canonical_knobs.rs:54/152/251-254` y validado en `:381`, pero un grep de `min_pool_liquidity` sobre `backend/searcher-rs/src` devuelve **solo** canonical_knobs.rs: **cero call-sites de enforcement**. Esto explica el hallazgo del CROSS ("ya existe como knob y NO está frenando esto").
6. **El único gate `same_token_in_out` que existe vive aguas abajo.** `scanner.rs:2846-2847` lo cita como razón del *encoder de simulación* (fase A.3) — es decir, el sistema YA sabe que el self-pair es inválido, pero solo lo verifica al armar el calldata de simulación: después de emitir, persistir y amplificar.
7. **Seam único de emisión.** TODO pasa por `publisher::publish` (`publisher.rs:9-28`, XADD con `MAXLEN ~ 10_000` — de ahí el XLEN 10.001 observado) y por `insert_opportunity_with_route` para PG. El path nativo tiene 4 sitios inline adicionales que persisten+publican sin pasar por `emit_rejected`: `scanner.rs:2017-2037` (no-config), `:2208-2233` (TokenNotAllowed), `:2251-2277` (StrategyDisabled), `:2299+` (gate-blocked).

**Conclusión del diagnóstico:** el self-pair recorre hoy el 100% del pipeline (decoder → matriz ×28 → sizing → spine → PG + Redis stream) para morir en un gate de allowlist cuya fila rechazada se escribe igual. El filtro debe inyectarse ANTES del loop de cartuchos y antes del enriquecimiento Redis del scanner.

---

## 1. El predicado no-op: por qué es matemático (y por qué NO es "token_in==token_out de la ruta")

**Definición adoptada — leg-level:** una `RouteIntentLeg` con `token_in == token_out` (comparación de direcciones, case-insensitive) es un no-op matemático: un swap de X→X en una Variedad de Liquidez es la identidad en holdings menos comisión — **Topological Yield estrictamente negativo bajo CUALQUIER estado de reserves**. No existe configuración de mercado, liquidez ni precio bajo la cual sea una oportunidad. Por eso puede suprimirse antes de evaluación, sin simular y sin depender de lista de tokens: es una propiedad estructural de la geometría de la ruta, no de la identidad del token (RULE 00: criterio dinámico — el flood puede rotar AGLD→XEN→PEPE→el que sea; el predicado no cambia).

**Trampa explícita que el diseño evita (y fija con test de regresión):** el predicado NO puede ser `first_leg.token_in == last_leg.token_out` (nivel ruta), porque esa igualdad es la **definición misma de un ciclo cerrado legítimo** (Holonomic Loop Resolution). El test existente `route_intent.rs:334-342` (`multi_leg_intent_valid`: legs [A→B, B→C, C→A]) documenta exactamente la geometría que DEBE pasar. Un filtro ingenuo por `pair_symbol == "X/X"` (que es como el CROSS describe el flood: `pair_symbol = "0x…/0x…"` misma dirección en ambas patas, `cartridge_boot.rs:1161`) habría matado TODO el triangular del sistema. La distinción es: el flood tiene **legs** X→X; el triangular legítimo tiene legs X→Y y **cerramiento a nivel ruta**.

**Caso borde declarado:** una ruta con una leg self-pair EMBEBIDA y otras legs reales (p.ej. [A→B, B→B, B→A]) está estrictamente dominada por su variante podada ([A→B, B→A] — saltar la fee). Diseño conservador: se rechaza el intent completo (`self_pair_noop`), no se poda la leg (podar = reestructurar geometría de ruta, fuera de alcance de WO-06; se documenta como refinamiento futuro). Este caso no aparece en la evidencia 24h (el flood es 1-2 legs todas self-pair), pero el predicado lo cubre por construcción.

**Por qué la supresión NO viola la doctrina de transparencia:** `scanner.rs:2179-2183` exige persistir rechazos de POLÍTICA (`TokenNotAllowed`, `StrategyDisabled`) porque demandan acción del operador (iterar el allowlist). Un `self_pair_noop` **no demanda ninguna acción, jamás** — una fila por evento es costo puro (PG + WAL + XADD + rotación de logs). R8 se preserva por otra vía: counter exacto `arbx_flood_suppressed_total{chain_id, filter}` + `debug!` por ítem + summary agregado por ventana a `info!` (patrón LOGFLOOD-01 / R9 ya usado en `cartridge_boot.rs:1075-1083` y `active_eval_summary` `:1514-1523`). El rechazo es audible y contable; simplemente no genera 28 filas.

---

## 2. Los tres filtros y sus puntos de inyección exactos

### F1 — `self_pair_noop` (no-op matemático, leg-level)

Un solo check por **intent** (no por cartucho) — mata la amplificación ×28 en la fuente. Tres puntos de entrada, tres inyecciones:

| ID | Archivo:línea (ancla) | Punto exacto | Cubre |
|---|---|---|---|
| **I1** | `orchestrator.rs::on_route_intent` — después del contador `DECODED_INTENTS_TOTAL` (`:311-314`), ANTES de `resolve(&intent)` (`:317`) y del spawn de cartuchos (`:383-425`) | Entrada del pipeline v2: engines nativos + dispatch a cartuchos. El contador de decode queda ANTES para que la telemetría de actividad del mempool siga siendo honesta (RULE 00: el operador sigue viendo cuánto decodifica la red) | DexEngine/TriangularEngine/etc. + cartridge path v2 |
| **I2** | `cartridge_boot.rs::active_evaluate_and_emit` — después del semáforo (`:967-978`), ANTES de `runner.list_cartridges()` (`:984`) | Las entradas que BYPASSEAN `on_route_intent`: `spawn_cartridge_eval` llamado desde `route_discovery_worker.rs:1531` y `route_scanner_worker.rs:550` | ciclo cerrado de route-discovery → matriz |
| **I3** | `scanner.rs::process_pending` — inmediatamente después de `build_dex_arb_candidate` (`:1609-1615`), ANTES de la lectura de trading_config (`:1617`) y del enriquecimiento Redis (`:1630-1642`: token-meta ×2, pools V2+V3, reserves) | Path legacy nativo dex_arb; además ahorra 2+ lecturas Redis por tx spam | scanner WS legacy |

En I1/I2 el predicado es `intent.legs.iter().any(|l| l.token_in == l.token_out)`; en I3 es `token_in.eq_ignore_ascii_case(&token_out)` sobre el candidato (el path legacy no tiene legs — `patterns.rs:49-50`).

### F2 — Dedup `(token, ventana)` sobre filas RECHAZADAS (segunda línea dinámica)

**Problema que resuelve:** el flood que NO sea self-pair (p.ej. un par spam A↔B entre dos pools sin spread real — la clase de los 3.415 `non_positive_profit`/24h) o cualquier reincidencia del mismo token. Hoy esas filas se escriben una por evento ×28 porque "rejected rows are always persisted (no dedup check)" (`opportunity_emitter.rs:337`).

**Diseño:** `FloodGate` global de proceso (patrón `counters()`, sin threading de Arc por 5 firmas — ver `counters.rs`), clave `(chain_id, clase_de_razón, par_canónico)`, ventana declarativa `ARBX_KNOB_FLOOD_DEDUP_WINDOW_SECS` (default 60 s). Semántica:

- **Primera ocurrencia en la ventana** → se persiste y publica 1 fila muestra (la evidencia R8 que el operador audita).
- **Ocurrencias siguientes en la ventana** → se suprimen PG+Redis y se cuentan EXACTAS en `arbx_flood_suppressed_total{chain_id, filter="rejected_window_dedup"}`. **El métrico ES el ledger de volumen de rechazo (cero error de muestreo); la fila es una muestra por ventana.** Esto cumple el objetivo declarado del propio docstring de `emit_rejected` (`opportunity_emitter.rs:337-339`: "so the operator can see rejection volume") mejor que N filas idénticas: mismo volumen visible, costo O(1) por ventana.
- `clase_de_razón` = primer token antes de `:` (`TokenNotAllowed`, `NonPositiveProfit`, …) — vocabulario cerrado, cardinalidad de labels acotada. El par NO va a labels (cardinalidad no acotada, el flood rota) — va al `debug!` y al summary por ventana.

**Inyección:**

| ID | Archivo:línea (ancla) | Punto exacto |
|---|---|---|
| **I4** | `opportunity_emitter.rs::emit_rejected` — después del bloque de clasificación de counters (`:356-372`) y del `dry_run` (`:374-393`), ANTES de `score_and_publish`/PG/XADD (`:401-413`) | Cubre cartridge path (`cartridge_boot.rs:1462, 1588, 1603, 1637, 1655, 1664, 1673`) y todos los engines que usan el emitter. Los counters de clasificación (`:356-372`) quedan ANTES del gate: siguen contando TODOS los rechazos — el ledger de razones permanece exacto |
| **I4b** | `scanner.rs` — los 3 sitios inline de rechazo (`:2208` TokenNotAllowed, `:2251` StrategyDisabled, `:2299+` gate-blocked), mismo guard de 3 líneas antes del `insert_opportunity_with_route` + `publisher::publish` | El path nativo no pasa por `emit_rejected` (diagnóstico §0.7) |

`emit_accepted` y la fila no-config (`scanner.rs:2017-2037`, `rejection_reason = None`, clase distinta) quedan FUERA del gate — protección del invariante XLEN para entradas legítimas.

### F3 — Tiering por liquidez de la ruta (cablear el knob existente, declarativo)

**No se inventa un mecanismo nuevo:** `ARBX_KNOB_MIN_POOL_LIQUIDITY_USD` ya existe como knob doctrinal (150.000 USD default, `canonical_knobs.rs:54/152`, validado `:381`) sin un solo call-site. WO-06 lo cablea como prefilter de tier:

- **Se rechaza SOLO con cifra computada** (`liquidity_below_floor`): liquidez USD del pool fuente = `r0/10^d0 · p0 + r1/10^d1 · p1` usando `ReservesEntry` (`reserves.rs:28-39`), decimals vía `reserves::get_token_meta` y precios vía el `price_snapshot` que ya se fetcha una vez por intent (`cartridge_boot.rs:1065-1073`). Cifra < knob → supresión pre-matriz + métrico.
- **R8 fail-honest explícito:** si la liquidez NO es computable (sin reserves cacheadas, sin decimals, o el token no tiene precio en el snapshot) → `None` → **la intent PASA** y se cuenta `arbx_flood_suppressed_total{filter="tier_unknown"}`. El prefilter jamás inventa una cifra ni se vuelve autoridad económica: los gates de downstream (SizeOptimizer/spine) deciden con su taxonomía existente. `None ≠ 0` (mismo invariante que `compute_gross_usd_for_spread`, `scanner.rs:2656-2658`).
- **Alcance declarado del "bottleneck":** en el path de cartuchos solo el pool de la PRIMERA leg tiene reserves cacheadas (`cartridge_boot.rs:1038-1042` lee `intent.legs.first()`); el tier se computa sobre ese pool fuente. El bottleneck de ruta completa (todas las legs) pertenece a route-discovery (que posee el grafo) y queda fuera de WO-06 — hueco declarado, no oculto.

| ID | Archivo:línea (ancla) | Punto exacto |
|---|---|---|
| **I5** | `cartridge_boot.rs::active_evaluate_and_emit` — después de `price_snapshot` (`:1073`), ANTES del loop `for … in pertinent` (`:1085`) | Una computación por intent (no por cartucho); F1 ya corrió antes |
| **I6** (opcional, mismo PR) | `scanner.rs::process_pending` — después de la lectura de pools/reserves (`:1700-1718`), con la misma semántica pass-on-None | El path nativo ya muere hoy en `missing_reserves_pool_b` (11.716/24h) para el mismo spam; incluirlo es simetría, no necesidad |

**Token-risk screen declarativo:** la capa de riesgo declarado por operador (allowlist de tokens del TradingConfig, ya enforcada por el spine como `TokenNotAllowed`) NO se duplica en el prefilter — RULE 00: prohibido que el searcher mantenga SU propia lista de tokens. La única fuente declarativa nueva es el knob de liquidez (número, no lista). La extensión a tiers operator-declarados (`risk_tier` por token en el config JSON) se documenta como extensión futura de schema del TradingConfig, NO se implementa aquí.

---

## 3. Diffs propuestos (marcados `// WO-06 (2026-09-06)`; NO aplicados — este WO es design)

### 3.1 Archivo nuevo `backend/searcher-rs/src/flood_gate.rs`

```rust
//! WO-06 (2026-09-06) — Structural anti-flood prefilter, BEFORE emission.
//!
//! Three dynamic filters (RULE 00: no token list ever lives here — the flood
//! rotates, AGLD→XEN→PEPE in 15 days; a predicate on route GEOMETRY or on a
//! DECLARED knob never goes stale):
//!   F1 `self_pair_noop` — a leg swapping X→X is identity-minus-fees:
//!      strictly negative Topological Yield under ANY reserves state.
//!   F2 `FloodGate`      — windowed dedup of REJECTED-row emission keyed
//!      (chain, reason-class, canonical pair). First per window is the R8
//!      evidence sample; the rest are counted EXACTLY in the metric.
//!   F3 liquidity tier   — wires the EXISTING ARBX_KNOB_MIN_POOL_LIQUIDITY_USD
//!      (previously declared without a single enforcement call-site).
//!
//! NOT a route-level check: `first.leg.token_in == last.leg.token_out` is the
//! DEFINITION of a legitimate closed cycle (Holonomic Loop Resolution — see
//! route_intent.rs tests `multi_leg_intent_valid`). Only LEG-level identity
//! is a no-op.

use crate::route_intent::{RouteIntent, RouteIntentLeg};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// F1: any leg with token_in == token_out makes the whole intent a
/// mathematical no-op (a route containing such a leg is strictly dominated
/// by its pruned variant — we reject rather than prune; conservative).
pub fn intent_is_self_pair_noop(intent: &RouteIntent) -> bool {
    intent.legs.iter().any(leg_is_self_pair)
}

/// F1 (pure, per-leg): exposed for unit tests and future prune-refinement.
pub fn leg_is_self_pair(leg: &RouteIntentLeg) -> bool {
    leg.token_in == leg.token_out
}

/// F1 on the legacy scanner path (patterns.rs builds candidates from a
/// DecodedSwap, not legs): hex strings, case-insensitive compare.
pub fn swap_is_self_pair(token_in: &str, token_out: &str) -> bool {
    token_in.eq_ignore_ascii_case(token_out)
}

/// F2: windowed admission for rejected-row emission. Process-global
/// (counters() pattern) so every emission site shares one window without
/// threading Arcs through five signatures.
pub struct FloodGate {
    window: Duration,
    seen: Mutex<HashMap<(u64, String, String), Instant>>,
}

impl FloodGate {
    pub fn new(window: Duration) -> Self {
        Self { window, seen: Mutex::new(HashMap::new()) }
    }

    /// `true` = admit (first occurrence inside the window — emit the R8
    /// sample row). `false` = suppress PG+Redis; the caller MUST increment
    /// arbx_flood_suppressed_total{filter="rejected_window_dedup"} so the
    /// count stays exact (metric = ledger, row = sample).
    pub fn admit_rejected(&self, chain_id: u64, reason_class: &str, pair: &str) -> bool {
        let key = (chain_id, reason_class.to_owned(), pair.to_owned());
        let mut seen = self.seen.lock().expect("flood_gate mutex");
        let now = Instant::now();
        match seen.get(&key) {
            Some(t) if now.duration_since(*t) < self.window => false,
            _ => {
                seen.insert(key, now);
                // Bound the map: the flood rotates tokens; a stale entry per
                // rotated pair would leak. Cheap sweep on insert.
                if seen.len() > 4096 {
                    seen.retain(|_, t| now.duration_since(*t) < self.window);
                }
                true
            }
        }
    }
}

/// Process-global instance (window from ARBX_KNOB_FLOOD_DEDUP_WINDOW_SECS,
/// default 60s — canonical_knobs.rs). Lazy so tests can construct their own.
pub fn global_flood_gate() -> &'static FloodGate {
    static GATE: std::sync::OnceLock<FloodGate> = std::sync::OnceLock::new();
    GATE.get_or_init(|| FloodGate::new(Duration::from_secs(knob_window_secs())))
}

fn knob_window_secs() -> u64 {
    crate::canonical_knobs::CanonicalKnobs::from_env().flood_dedup_window_secs
}

/// F3 pure kernel: USD liquidity of one pool from reserves+decimals+prices.
/// None when ANY input is missing (R8: None = "not computed", NEVER 0.0 —
/// the caller passes the intent through and counts tier_unknown).
pub fn pool_liquidity_usd(
    r0: f64, d0: u8, p0: Option<f64>,
    r1: f64, d1: u8, p1: Option<f64>,
) -> Option<f64> {
    let leg0 = p0.filter(|p| p.is_finite() && *p > 0.0)
        .map(|p| r0 / 10f64.powi(d0 as i32) * p);
    let leg1 = p1.filter(|p| p.is_finite() && *p > 0.0)
        .map(|p| r1 / 10f64.powi(d1 as i32) * p);
    match (leg0, leg1) {
        (Some(a), Some(b)) => Some(a + b),
        // One priced side + one unpriced is a LOWER BOUND, not the figure:
        // fail-honest None (declare, never half-invent).
        _ => None,
    }
}
```

### 3.2 `backend/searcher-rs/src/lib.rs`

```diff
 pub mod fe_normalization.rs;
+pub mod flood_gate; // WO-06 (2026-09-06): structural anti-flood prefilter
 pub mod financing;
```

### 3.3 `backend/searcher-rs/src/canonical_knobs.rs` — nuevo knob declarativo

```diff
     pub min_pool_liquidity_usd: f64, // 150_000 (USD, route bottleneck)
+    /// WO-06 (2026-09-06): F2 window for rejected-row emission dedup.
+    pub flood_dedup_window_secs: u64, // 60 (s)
```
(en `default()` `:152` → `flood_dedup_window_secs: 60,`; en `from_env()` `:251+` →
`flood_dedup_window_secs: env_u64("ARBX_KNOB_FLOOD_DEDUP_WINDOW_SECS", d.flood_dedup_window_secs),`;
en `validate()` `:381+` → `if self.flood_dedup_window_secs == 0 { return Err("flood_dedup_window_secs must be > 0".to_string()); }`)

### 3.4 `backend/searcher-rs/src/metrics.rs` — métrico del flood (el delta declarado)

```rust
// ---------------------------------------------------------------------------
// flood_suppressed_total{chain_id, filter}  (WO-06 2026-09-06)
// ---------------------------------------------------------------------------
/// Suppressed-before-emission volume, by structural filter. This counter IS
/// the rejection-volume ledger for suppressed classes (exact count, zero
/// sampling error); PG/Redis keep one sample row per window. `filter` is a
/// closed vocabulary: self_pair_noop | rejected_window_dedup |
//  liquidity_below_floor | tier_unknown.
pub static FLOOD_SUPPRESSED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_flood_suppressed_total",
            "Candidate emissions suppressed by the structural anti-flood prefilter"
        ),
        &["chain_id", "filter"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});
```
(hermano directo de `REJECTED_CONFIG_TOTAL`, `metrics.rs:151-162`, mismo patrón de registro)

### 3.5 I1 — `backend/searcher-rs/src/orchestrator.rs::on_route_intent`

```diff
         // ── Step 1: decoded_intents_total metric ───────────────────────────
         DECODED_INTENTS_TOTAL
             .with_label_values(&[&chain_str, source_str])
             .inc();
+
+        // ── WO-06 (2026-09-06): F1 — self-pair no-op prefilter ─────────────
+        // A leg swapping X→X is identity-minus-fees: strictly negative
+        // Topological Yield under ANY reserves. Killing it HERE — before
+        // impact resolution, engine fan-out and the cartridge matrix —
+        // removes the ×28 amplification at its source. NOT route-level:
+        // first-leg-in == last-leg-out is the definition of a legitimate
+        // closed cycle (Holonomic Loop Resolution). Suppression is audible:
+        // arbx_flood_suppressed_total{filter="self_pair_noop"} + debug +
+        // the per-window summary (R8/R9) — no operator action exists for a
+        // mathematical identity, so no PG row is owed (unlike policy gates).
+        if crate::flood_gate::intent_is_self_pair_noop(&intent) {
+            debug!(
+                event = "flood_gate.self_pair_noop",
+                chain_id,
+                tx_hash = %intent.tx_hash,
+                "intent suppressed: leg-level X→X no-op"
+            );
+            crate::metrics::FLOOD_SUPPRESSED_TOTAL
+                .with_label_values(&[&chain_str, "self_pair_noop"])
+                .inc();
+            return Ok(());
+        }
```

### 3.6 I2 — `backend/searcher-rs/src/cartridge_boot.rs::active_evaluate_and_emit`

```diff
     // Bound global active-eval concurrency; drop (don't queue) when at capacity.
     let _permit = match shadow_semaphore().clone().try_acquire_owned() {
         Ok(p) => p,
         Err(_) => { /* … unchanged … */ }
     };
+
+    // ── WO-06 (2026-09-06): F1 — self-pair no-op dies BEFORE the matrix ────
+    // One check per intent kills the ×28 cartridge amplification for the
+    // AGLD/XEN/PEPE-class flood (04-CROSS §Evidencia 2: 97.0% of 24h volume,
+    // ~769 market events × ~28 mev_01 cartridges). This site is REQUIRED in
+    // addition to on_route_intent because spawn_cartridge_eval
+    // (route_discovery_worker.rs:1531, route_scanner_worker.rs:550) feeds
+    // this function directly, bypassing the orchestrator entry.
+    if crate::flood_gate::intent_is_self_pair_noop(&intent) {
+        debug!(
+            event = "flood_gate.self_pair_noop",
+            chain_id,
+            tx_hash = %intent.tx_hash,
+            "cartridge eval suppressed: leg-level X→X no-op (pre-matrix)"
+        );
+        crate::metrics::FLOOD_SUPPRESSED_TOTAL
+            .with_label_values(&[&chain_id.to_string(), "self_pair_noop"])
+            .inc();
+        return;
+    }
```

### 3.7 I5 — mismo archivo, F3 después del price_snapshot (`:1073`)

```diff
     let price_snapshot: std::collections::HashMap<String, f64> = { /* … unchanged … */ };
+
+    // ── WO-06 (2026-09-06): F3 — liquidity tier on the SOURCE pool ────────
+    // Wires ARBX_KNOB_MIN_POOL_LIQUIDITY_USD (canonical_knobs.rs:54/251;
+    // declared 150K default, previously ZERO enforcement call-sites). R8:
+    // reject ONLY on a computed figure below the floor; incomputable → pass
+    // through + tier_unknown (None ≠ 0 — the spine stays the economic
+    // authority). Scope: source-pool tier (first leg's cached reserves,
+    // :1038-1042); full route-bottleneck tiering belongs to route-discovery.
+    if let Some(entry) = reserves_source.as_ref() {
+        let liquidity = {
+            let mut redis_conn = runner.redis_connection().await;
+            crate::flood_gate::source_pool_liquidity_usd(
+                &mut redis_conn, chain_id, entry, &price_snapshot,
+            )
+            .await
+        };
+        let floor = crate::canonical_knobs::CanonicalKnobs::from_env()
+            .min_pool_liquidity_usd;
+        match liquidity {
+            Some(usd) if usd < floor => {
+                debug!(
+                    event = "flood_gate.liquidity_below_floor",
+                    chain_id, tx_hash = %intent.tx_hash,
+                    liquidity_usd = usd, floor_usd = floor,
+                    "intent suppressed: source-pool liquidity below declared floor"
+                );
+                crate::metrics::FLOOD_SUPPRESSED_TOTAL
+                    .with_label_values(&[&chain_id.to_string(), "liquidity_below_floor"])
+                    .inc();
+                return;
+            }
+            Some(_) => {}
+            None => {
+                crate::metrics::FLOOD_SUPPRESSED_TOTAL
+                    .with_label_values(&[&chain_id.to_string(), "tier_unknown"])
+                    .inc();
+            }
+        }
+    } else {
+        crate::metrics::FLOOD_SUPPRESSED_TOTAL
+            .with_label_values(&[&chain_id.to_string(), "tier_unknown"])
+            .inc();
+    }
```
(`source_pool_liquidity_usd` = wrapper async en `flood_gate.rs` que resuelve decimals de token0/token1 vía `reserves::get_token_meta` y precios por símbolo desde el snapshot, y delega en el kernel puro `pool_liquidity_usd`.)

### 3.8 I3 — `backend/searcher-rs/src/scanner.rs::process_pending`

```diff
     let opportunity = patterns::build_dex_arb_candidate(&ctx, &decoded);
+
+    // ── WO-06 (2026-09-06): F1 — self-pair dies BEFORE enrichment ─────────
+    // X→X is identity-minus-fees; also saves the token-meta/pool/reserves
+    // Redis reads below for spam txs. `decoded_ok` above already counted
+    // the decode (telemetry of what the mempool contains stays honest).
+    if crate::flood_gate::swap_is_self_pair(&opportunity.token_in, &opportunity.token_out) {
+        debug!(
+            event = "flood_gate.self_pair_noop",
+            chain_id = client.chain_id,
+            hash = %hash,
+            "pending tx suppressed: X→X no-op (pre-enrichment)"
+        );
+        crate::metrics::FLOOD_SUPPRESSED_TOTAL
+            .with_label_values(&[&client.chain_id.to_string(), "self_pair_noop"])
+            .inc();
+        return Ok(());
+    }
```

### 3.9 I4 — `backend/searcher-rs/src/opportunity_emitter.rs::emit_rejected`

```diff
     pub enum EmitOutcome {
         Published,
         DbError(/* … */),
+        // WO-06 (2026-09-06): suppressed by the windowed flood gate — the
+        // metric carries the exact count; no PG row, no XADD.
+        Suppressed,
     }
```
```diff
         // Dry-run (shadow mode): log + record, no I/O.
         if self.dry_run { /* … unchanged … */ }
+
+        // ── WO-06 (2026-09-06): F2 — windowed rejected-row flood gate ──────
+        // First rejected row per (chain, reason-class, pair) in the window
+        // is the persisted R8 sample; subsequent duplicates are suppressed
+        // from PG+Redis and counted EXACTLY in
+        // arbx_flood_suppressed_total{filter="rejected_window_dedup"}.
+        // The gate classifiers above already ran: the per-reason counters
+        // remain the exact rejection ledger; this gate only bounds writes.
+        let reason_class = rejection_reason.split(':').next().unwrap_or(rejection_reason);
+        let pair = format!("{}/{}", opportunity.token_in, opportunity.token_out);
+        if !crate::flood_gate::global_flood_gate()
+            .admit_rejected(opportunity.chain_id, reason_class, &pair)
+        {
+            debug!(
+                event = "flood_gate.rejected_window_dedup",
+                opp_id = %opportunity.id,
+                reason_class, pair = %pair,
+                "rejected row suppressed (windowed duplicate); count in metric"
+            );
+            crate::metrics::FLOOD_SUPPRESSED_TOTAL
+                .with_label_values(&[
+                    &opportunity.chain_id.to_string(),
+                    "rejected_window_dedup",
+                ])
+                .inc();
+            return Ok(EmitOutcome::Suppressed);
+        }
```
(Nota de compatibilidad: los callers de `emit_rejected` hoy hacen `if let Err(e)` / `.await?` — no matchean variantes, agregar `Suppressed` es aditivo. Verificar en el PR que ningún caller haga `match` exhaustivo.)

### 3.10 I4b — `scanner.rs`, guard idéntico en los 3 sitios inline

```diff
         // (en cada uno de los brazos TokenNotAllowed :2208, StrategyDisabled
         //  :2251 y StrategyConfigGateBlocked :2299, antes del bloque
         //  `if let Some(pool) = db { insert_opportunity_with_route … }`):
+        // WO-06 (2026-09-06): F2 — windowed rejected-row flood gate (same
+        // semantics as emit_rejected; this path bypasses the emitter).
+        let _flood_reason_class = "TokenNotAllowed"; // per-site literal
+        let _flood_pair = format!("{}/{}", opportunity.token_in, opportunity.token_out);
+        if !crate::flood_gate::global_flood_gate()
+            .admit_rejected(client.chain_id, _flood_reason_class, &_flood_pair)
+        {
+            crate::metrics::FLOOD_SUPPRESSED_TOTAL
+                .with_label_values(&[&client.chain_id.to_string(), "rejected_window_dedup"])
+                .inc();
+            return Ok(());
+        }
```

### 3.11 Registro del módulo en tests — `flood_gate.rs #[cfg(test)]`

(Ver §4 — los fixtures AGLD/XEN/PEPE viven SOLO aquí.)

---

## 4. Fixtures AGLD/XEN/PEPE + batería de tests

**Addresses de fixture** (los 3 tokens del flood medido; truncados en `04-CROSS §Evidencia 2` como `0x3235…`, `0x0645…`, `0x6982…`). Son contratos ERC-20 públicos canónicos de mainnet; el PR debe re-verificarlos contra la fuente de verdad antes de mergear (RULE 00 — cero invención):

```sql
-- verificación en el PR (VPS, read-only):
SELECT rejection_reason, count(*) FROM opportunities
 WHERE detected_at > now() - interval '24 hours'
   AND rejection_reason LIKE 'TokenNotAllowed:%'
 GROUP BY 1 ORDER BY 2 DESC LIMIT 10;
-- expected: TokenNotAllowed:0x3235…(AGLD) / 0x0645…(XEN) / 0x6982…(PEPE)
```

```rust
// flood_gate.rs — tests (direcciones reales del flood; SOLO en cfg(test):
// un grep del diff del PR debe mostrar CERO ocurrencias en src/ de producción)
mod fixtures {
    // Adventure Gold — 37.2% del flood 24h (04-CROSS)
    pub const AGLD: &str = "0x32353a6c91143bfd6c7d363b546e62a9a2489a20";
    // XEN crypto — 34.0%
    pub const XEN:  &str = "0x06450dee7fd2fb8e39061434babcfc05599a6fb8";
    // PEPE — 25.8% (el token NUEVO del flood: prueba de que el criterio
    // dinámico es el correcto; un enum XEN/AGLD habría nacido obsoleto)
    pub const PEPE: &str = "0x6982508145454ce325ddbe47a25d4ec3d2311933";
    pub const WETH: &str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
    pub const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
}
```

**Batería (id → aserción):**

| ID | Test | Aserta |
|---|---|---|
| T1 | `self_pair_single_leg_agld_suppressed` | intent 1-leg [AGLD→AGLD] (geometry del path=[AGLD,AGLD] vía `route_decoder.rs:249-266`) → `intent_is_self_pair_noop == true` |
| T2 | `self_pair_multi_leg_xen_suppressed` | intent 2-leg [XEN→XEN, XEN→XEN] → `true` |
| T3 | `self_pair_scanner_predicate_pepe` | `swap_is_self_pair(PEPE, PEPE)` → `true`; y case-mix `0x3235…` vs `0x3235…` (checksum vs lower) → `true` |
| **T4** | **`triangular_closed_cycle_is_not_noop` (REGRESIÓN — el test que fija la trampa de §1)** | intent [WETH→USDC, USDC→PEPE, PEPE→WETH] → `false`. Ancla de geometría legítima: el test existente `route_intent.rs:334-342` ya clava [A→B,B→C,C→A] como válida — este test garantiza que el filtro NUEVO no la mata. `pair_symbol` sería "WETH/WETH" (first-in == last-out) y DEBE pasar: nivel ruta ≠ no-op |
| T5 | `two_leg_cycle_passes` | [WETH→USDC, USDC→WETH] → `false` (el degenerate 2-hop legítimo — swap y vuelta — NO es no-op matemático: tiene spread real computable; el spine lo evalúa) |
| T6 | `embedded_self_pair_rejected_conservatively` | [A→B, B→B, B→A] → `true` (dominada por la variante podada; decisión conservadora documentada §1) |
| T7 | `flood_gate_window_dedup` | `FloodGate::new(60s)`: 1ª llamada (chain 1, "TokenNotAllowed", "AGLD/AGLD") → admit; 2ª y 28ª → suppress; ventana expirada (gate de window=0s en test propio) → admit de nuevo |
| T8 | `flood_gate_distinct_pairs_never_collide` | (1,"TokenNotAllowed","AGLD/AGLD") admit y (1,"TokenNotAllowed","XEN/XEN") admit simultáneos; distinta chain_id tampoco colisiona |
| T9 | `liquidity_tier_kernel` | `pool_liquidity_usd(100e18, 18, Some(2000.0), 200e6, 6, Some(1.0)) == Some(2200.0)` → 2200 < 150_000 → `liquidity_below_floor`; con `Some(2.0e18)`-escala → pasa; price `None` → `None` → tier_unknown/passthrough (R8: None ≠ 0) |
| T10 | `legit_emission_count_invariant` | harness con emitter `dry_run=false` + redis mock-free… **NO**: sin Redis real, T10 se divide en (a) unit: `admit_rejected` admite TODO input legítimo (T8 ya lo cubre) y (b) la verificación XLEN de deploy (§5). Declarado honestamente así — no se fabrica un integration test que no puede correr local (RULE 00) |

**Sobre T5 — decisión declarada:** [A→A] vía 2 pools distintos (buy en pool1, sell en pool2) NO es self-pair leg-level (legs A→B? no — legs serían A→B? …) — precisión: una ruta 2-pools legítima es [X→Y (pool1), Y→X (pool2)] con legs X→Y e Y→X — ambas distintas de identidad → pasa F1 por construcción. El cross-pool del MISMO par es exactamente el dex_arb canónico. F1 no lo toca.

---

## 5. Invariante XLEN y el delta del flood como MÉTRICA

**Invariante (entradas legítimas):** para toda intent no-self-pair, con par dedup-fresco en su ventana y tier computado-por-encima-del-piso (o incomputable), el conjunto de emisiones post-parche es IDÉNTICO al pre-parche → **`XLEN arbx:opps:detected` delta = 0** (§33.3). Los tres filtros son probadamente no-ops sobre entradas legítimas:

- F1: no-op salvo legs X→X (T4/T5 prueban que ciclos cerrados y cross-pool pasan).
- F2: solo toca filas con `rejection_reason` DENTRO de una ventana duplicada; la primera ocurrencia se emite idéntica (T7/T8).
- F3: solo rechaza con cifra computada < knob; incomputable pasa (T9).

**Verificación del invariante (runbook del PR, §33.3):**
1. Pre-deploy: `redis-cli XLEN arbx:opps:detected` + tasa por minuto de un par LEGÍTIMO de control (p.ej. rows `dex_arb` con `rejection_reason IS DISTINCT FROM 'TokenNotAllowed:%'`) en ventana de 10 min.
2. Post-deploy: misma medición → tasa igual (± ruido de mercado). Delta XLEN esperado para tráfico legítimo: 0.
3. El volumen suprimido aparece SOLO como `increase(arbx_flood_suppressed_total[10m])` — el delta del flood **declarado como métrica, no como pérdida**.

**Por qué cero pérdida de oportunidades es demostrable y no una promesa:** el 100% del volumen suprimido hoy termina en filas con `rejection_reason NOT NULL` (100% rejected 15 días; `max(detected_at) WHERE rejection_reason IS NULL` = 2026-08-22 16:52:25Z — 04-CROSS §Evidencia 2). No existe una sola oportunidad (fila aceptada) dentro del conjunto suprimido: F1 solo toca geometrías de Topological Yield estrictamente negativo; F2 solo toca duplicados de rechazo dentro de 60 s; F3 solo toca liquidez computada bajo el piso declarado por el operador.

**Aritmética del delta esperado (24h, de la evidencia, no proyección):**
- `opportunities` PG: −≈56.2K filas/día (97.0% de 57.981) de las cuales AGLD 21.588 · XEN 19.236 · PEPE ≈14.96K.
- `arbx:opps:detected` XADD: misma proporción del flujo (entries-added observado 518.489 en ~5 h — el share rechazado domina).
- `arbx:route_discovery:outcomes`: el eco ×28 por evaluación de cartucho (`emit_shadow_outcome` incondicional por cartucho, `cartridge_boot.rs:1096-1104`) muere en I2 para self-pairs → ≈769 eventos × 28 cartuchos/día menos outcomes AGLD-clase (más los de XEN/PEPE). **La fracción exacta del influjo de 30M filas/día de RDO que es eco del flood NO fue computada read-only** (requiere JOIN temporal RDO×opportunities en el VPS) — se declara el hueco (R8) y se estima solo el orden: la parte del flood es la palanca individual más grande disponible sobre la tasa de inserción que alimenta el ENOSPC 09-12/13 (roadmap D-2).

---

## 6. Efecto cascada (documentado)

| Superficie | Efecto | Mecanismo | Fuente |
|---|---|---|---|
| **purge/RDO (D-2)** | Menos inserciones: opportunities −97%/24h + outcomes de cartucho −(self-pair share). El cap de purge 20M/día vs inserción 30M/día es hoy deficitario (+10M/día ≈ +5 GB/día → ENOSPC ≈09-12/13); este filtro es el único P1 que ataca la INSERCIÓN en vez del cap | I2 mata `emit_shadow_outcome` por cartucho antes del loop; I1/I3 matan las filas rechazadas | 00-PREDATOR-ROADMAP.md D-2 (§1.3), 07-CROSS §2.4 |
| **WAL/ENOSPC** | Menos INSERT PG → menos WAL por batch (precedente: 53.8M DELETEs = 16 GB WAL sin reciclar → crash-loop 09-04). Reducir la generación es más seguro que acelerar el purge | menos filas → menos WAL por ciclo | memoria pg-wal-burst-delete-checkpoint-pacing; P0-3 |
| **Ruido de logs (R9)** | `cartridge.active_eval_enter` 25/s a INFO (200 líneas = 5.6 s de reloj) muere en I2 para spam; rotación de 50 MB se desahoga | I2 retorna antes del `info!` de entrada (`cartridge_boot.rs:959-964`) | 04-CROSS §Evidencia 7 |
| **Flip revm (P1-3)** | El drain-guard y la cobertura del catálogo operarían sobre ~40× menos ruido; `strategy_not_simulatable_in_s4` 37.3K/24h se reduce en el share spam | menos entradas al embudo de simulación | 00-PREDATOR-ROADMAP P1-2 "Riesgo si no"; 05-CROSS |
| **Redis trim-antes-de-consumo (D-10)** | lag>length hoy inerte (todo rejected) se vuelve CRÍTICO post-flip; menos XADD de flood alarga el margen entre frente de trim y consumidores | menos XADD → menor velocidad de rotación del stream MAXLEN 10K | publisher.rs:7; 07-CROSS §2.3 |
| **Grupos huérfanos (D-9)** | Menos volumen por consumir; no los cura (eso es P1-10) — efecto paliativo, declarado | — | 03-CROSS C4 |

---

## 7. Gate del PR (checklist para el implementador)

1. **PR con ID** (P-∅ §37): `ARBX-FLOOD-STRUCT-01 (WO-06)` — un PR = un ID; nada ajeno al filtro entra al diff (cero reformateo).
2. **Unit tests** §4 (T1-T9) verdes + `cargo check` + `cargo clippy` + `cargo fmt` (workspace searcher-rs; Windows AppControl: usar el árbol principal con `target/` caliente — §36 nota worktrees).
3. **RULE 00 grep del diff:** `git diff | grep -iE "0x3235|0x0645|0x6982|agld|xen\b|pepe"` → hits SOLO en `#[cfg(test)]`. Un solo hit en src/ de producción = rechazo del PR.
4. **Invariante XLEN** (§5 runbook): delta = 0 para tráfico legítimo en ventana de 10 min post-deploy; supresión visible SOLO en `arbx_flood_suppressed_total`.
5. **Promtool** (si se agrega regla): con el fix #545 el unit-test de reglas carga reglas reales — cualquier alerta nueva sobre el métrico usa window 24 h (lección `SimulationFailureRateHigh`, P1-6).
6. **Modo-invariante §34.1:** el prefilter corre IDÉNTICO en PAPER_SHADOW/TESTNET/LIVE_MAINNET — es matemática de geometría de ruta, no semántica de modo. La pregunta canónica: "¿funcionaría con capital real en LIVE_MAINNET?" → sí: suprime exactamente lo mismo, porque un X→X nunca fue ejecutable en ningún modo.
7. **§34.3 intocado:** el diseño no toca `relays-client`, `live_exec_policy`, default-deny ni `MainnetRefused`. Es aguas arriba del terminus.
8. **Rollback:** revert del PR = comportamiento actual (el flood vuelve); no hay migración de schema, no hay cambio de stream key, no hay estado persistido nuevo (el FloodGate es in-memory por proceso).

---

## 8. Cumplimiento de reglas (declarado)

- **RULE 00:** cero tokens hardcodeados en lógica — los predicados son geometría de ruta (F1), ventana+razón (F2) y knob numérico (F3). Los 3 addresses viven solo en fixtures de test y se re-verifican contra PG en el PR (§4). Cero datos fabricados: la fracción no computada del eco en RDO se declara hueco (§5).
- **R8 fail-honest:** `tier_unknown` pasa-through contado; `None ≠ 0` en el kernel de liquidez; `EmitOutcome::Suppressed` distingue supresión de publicación.
- **R9:** supresión per-item a `debug!` + counter exacto; sin nuevos eventos INFO por ítem.
- **§32/§33:** este WO es diseño read-only. Cero requests HTTP al dominio público (0/5). VPS intocado. Nada de executor/wallets/capital/firma/broadcast.
- **NO-GIT:** cero commits/pushs/PRs generados por este agente; los diffs de §3 son propUESTAS para el PR del implementador.
- **Lexicon OMEGA:** Variedad de Liquidez (pool), Topological Yield (rendimiento), TLS/Holonomic Loop Resolution citados en su traducción física.

## 9. Verificación local ejecutada por este agente (evidencia de que el diseño calza)

- `git`-free: lectura de `backend/searcher-rs/src/{scanner,cartridge_boot,orchestrator,dedup,route_intent,route_decoder,publisher,opportunity_emitter,signal_tier,canonical_knobs,reserves,metrics,patterns}.rs` en HEAD actual (branch `a6-cbprom-01`); el CROSS certificó que `backend/searcher-rs` está sin delta vs `origin/main` 9ac06d2d (04-CROSS tabla local: "git diff --stat origin/main..HEAD -- backend/searcher-rs = vacío") → las anclas de línea son válidas para el código desplegado.
- Grep de verificación: `same_token_in_out` (1 hit, solo comentario del encoder — §0.6) · `min_pool_liquidity` (solo canonical_knobs.rs — §0.5) · `flood` (solo log-flood/comentarios, no existe un gate previo — no se duplica trabajo) · `OppDedup` (wired solo en scanner path via chain_supervisor.rs:295-322; cartridge path sin dedup — §0.4).

---

*Fin del diseño WO-06. Los diffs de §3 NO están aplicados; el entregable es este documento. Fail-honest en todo: los dos huecos declarados son (1) fracción exacta del eco en RDO (requiere query en VPS) y (2) T10-integration (requiere Redis real; cubierto por runbook §5 en su lugar).*
