# 🏛️ LAS RUTAS — Doctrina de la Joya de la Corona

> **Descubrimiento · Especialización · Configuración Óptima Dinámica**
>
> Investigación de clase mundial (5 investigadores paralelos: papers arXiv/VLDB/USENIX, docs de protocolos verificados on-chain, repos de producción, práctica searcher) + mapa interno del repo. **100% route-céntrico**: todo instrumento financiero aparece únicamente como dimensión que determina qué rutas existen para el sistema.
>
> Fecha: 2026-08-18 · Fuente de implementación parcial: PR #411 (DeferNeverDrop, en CI)

---

## 0. Por qué las rutas son la joya de la corona

Todo el negocio del sistema se reduce a UNA pregunta: **¿qué ciclos cerrados del grafo de liquidez son rentables netos, y cómo los encontramos antes que nadie?** Sin descubrimiento exhaustivo, no hay oportunidades; sin especialización correcta (sizing + financiamiento), las descubiertas son mentiras cosméticas; sin configuración dinámica, el operador no puede adaptar el descubrimiento al mercado que cambia. Los números del mundo lo confirman: de 2.4M de oportunidades escaneadas, ~0.006% son rentables post-gas (bot de producción, 3 meses) — el filo está en **encontrar las pocas y no perder ninguna**.

---

## 1. EL DESCUBRIMIENTO — estado del arte mundial vs. nuestra posición

### 1.1 La formulación canónica

```
Tokens = vértices · Pools = aristas dirigidas (multigrafo: 1 arista por pool)
peso(u→v) = −log(r(u→v))   donde r = tipo de cambio ajustado por fee
CICLO RENTABLE ⇔ Σ pesos < 0 ⇔ Π tasas > 1
```

Universal en TODA la literatura (DeFi-ARB IEEE S&P 2021 → RICH VLDB 2025). El pipeline completo itera **por bloque** (12s Ethereum): actualizar reserves → detectar/enumerar → sizificar → simular → someter.

### 1.2 Los algoritmos y SUS NÚMEROS duros

| Algoritmo | Complejidad | Realidad de producción |
|---|---|---|
| **Bellman-Ford-Moore** (DeFi-ARB 2021) | O(V·E) | El baseline de producción; **TLE >3,600s** más allá de ~45-70K nodos (RICH Tabla 3) |
| **RICH** (VLDB 2025, Tokka Labs — 40+ venues) | Color-coding+DP: O(2^k·V·E) por coloración | **47.5s en grafo de 360K nodos vs 1,892s del mejor baseline (32.7×)**; k≤5 default; error 0.02-3.9% vs óptimo exacto |
| **Johnson** (ciclos elementales, 1975) | O((n+e)(c+1)) output-sensitive | Enumeración EXACTA pero c exponencial en grafos densos — base de `networkx.simple_cycles` |
| **DFS acotado** | O(d^maxHops), d≈4 | **El default del practicante** (ccyanxyz, whack-a-mole, evm-amm-search) |
| **Enumeración exhaustiva** (evm-amm-search, Rust) | — | **35-39ms warm en basket de 320 pools (2-3 hops)**; refresh incremental 4.37ms vs 8ms full |

**Hecho teórico crítico (RICH Teo. 2.2):** el problema del ciclo MÁS negativo con k hops acotados es **NP-completo** — por eso todos los sistemas de producción son heurísticos o probabilísticos. La enumeración exhaustiva solo es viable sobre **grafo podado** (§1.4).

### 1.3 La realidad de los hops (datos, no opiniones)

- **85% de los arbitrajes cíclicos ejecutados históricamente son de 3 hops** (Wang et al., 292,606 arbs medidos)
- **Ciclos >5 hops = <2% del mercado** (GoldPhish/Wang vía RICH)
- Costo de enumeración O(d^k): los tiers 6-7 dominan el runtime y contribuyen ~nada
- **Implicación para nuestro 2..7:** exhaustivo hasta 5 como core; **6-7 como knob medible** (yield por tier en shadow antes de gastar ahí) — exactamente lo que el panel flotante configurará

### 1.4 La poda del grafo — la doctrina documentada

```
Ethereum mainnet crudo:      ~402-448K tokens · 428-475K pools (3 DEXes)
Tras poda efectiva:          ~11,000 tokens · 25,000 pools   (PRIME 2026)
Arbitraje-activo histórico:  2,890 pools · 1,143 tokens llevaron TODAS las ops >0.1 ETH
                             (17,189 ciclos explotados; solo 265 >10 veces)
Topología: 98.34% de pools tocan WETH/USDC/USDT/DAI/WBTC; Gini TVL 0.996;
           betweenness(WETH)=0.9995 — el grafo ES un hub con WETH al centro
```

Poda canónica en 4 capas: (1) **core hub** (WETH + stables + top-K por TVL — el k-core 11 tiene solo 18 tokens/125 pools y carga TODA la conectividad); (2) **floor de liquidez**; (3) eliminación de hojas (96.65% de tokens solo conectan con WETH); (4) **safety screening** (drop FoT/rebase). **Cada decisión de poda se loguea** — cobertura auditable (R8).

### 1.5 Refresco: eventos, no rebuilds por bloque

Doctrina de dos relojes: **hot-path event-driven** (WebSocket `Sync`/`Swap`/`Slot0` → cache en memoria, sub-segundo — whack-a-mole DexStream, evm-amm-state) + **reconciliación por bloque** (RICH "block iteration"). Mejora cuantificada: re-cotizar SOLO las rutas que tocan pools cambiados = **4.37ms vs 8ms full** — requiere un **índice ruta→aristas** (la pieza que nos falta, §5 F4).

### 1.6 NUESTRA POSICIÓN (honesto)

| Capacidad | Mundo | Nosotros | Estado |
|---|---|---|---|
| Enumeración exhaustiva sin pérdida | Solo en baskets podados | **DeferNeverDrop** (cursor+defer, prueba H1 matemática) | ✅ PR #411 — **ÚNICO con garantía exhaustiva probada** |
| Detección negativa rápida | BFM/RICH | `multi_hop_search` (pasaje Bellman-Ford) | ✅ existe (observación) |
| Multigrafo paralelo | petgraph, 1 arista/pool | Nuestro DFS ya enumera pools paralelos + rotación por ladder | ✅ |
| Grafo podado por tiers | PRIME core-hub | Pool enum (PR #410) + safety screen | 🟡 parcial — falta tiering hub |
| Índice incremental ruta→pool | evm-amm-state 4.37ms | — | ❌ F4 del plan |
| Poda auditable | "log every pruning decision" | rejected_event con razón | ✅ (telemetría existe) |

**Síntesis:** tenemos la pieza más difícil y rara (enumeración exhaustiva *probada*). El plan nos lleva a paridad de producción en poda, incrementalidad y ranking.

---

## 2. LAS DOS CAPAS — descubrimiento puro ≠ evaluación (doctrina de primera clase)

**La separación que el mundo suele mezclar y que nosotros hacemos estructural.** Dos capas con contratos distintos, telemetría distinta y accountable distinta:

```
┌────────────────────────────────────────────────────────────────────┐
│ CAPA 1 · DESCUBRIMIENTO (puro — SIN juicios económicos)            │
│ Enumera TODOS los ciclos cerrados 2..N hops sobre el grafo podado. │
│ Salida: candidatas con TOPOLOGÍA (tokens+ pools). Nada más.        │
│ Contrato: exhaustivo sin pérdida (DeferNeverDrop, prueba H1).      │
│ Métrica: `candidatas_aparecidas(hop_tier)`                         │
├────────────────────────────────────────────────────────────────────┤
│ CAPA 2 · EVALUACIÓN (funil medible con atribución total)           │
│ Cada candidata atraviesa gates ORDENADOS; cada gate cuenta su      │
│ mortandad. NADA muere en silencio — cada muerte tiene razón,       │
│ hop-tier y modo de financiamiento.                                  │
│                                                                    │
│  candidatas(hop)                                                    │
│    → [G1 · liquidez] reserves/slot0 frescos + floor         ──✗ n₁ │
│    → [G2 · financiamiento] viable bajo qué modos (por modo) ──✗ n₂ │
│    → [G3 · gas] min_amount_in > gas/spread                  ──✗ n₃ │
│    → [G4 · capital] (si financing off): size ≤ inventory     ──✗ n₄ │
│    → [G5 · sizing] Δ* y EV neto por modo (waterfall §2.4)    ──✗ n₅ │
│    = viables(hop, modo) + tamaño + ledger completo                 │
└────────────────────────────────────────────────────────────────────┘
```

**El ledger del funil alimenta LA DAPP** — sus paneles, su ranking, sus decisiones de config: por cada hop-tier, cuántas rutas APARECEN y cuántas DESAPARECEN al imponer cada condición — p.ej. "a 5 hops aparecieron 1,240; la liquidez mató 612; el fee del provider mató 284; el gas mató 310; sin financiamiento solo 3 caben en capital propio; con flash swap sobreviven 29; con Balancer 0 bps sobreviven 31". La tabla por tier × gate × modo ES la información de decisión de la dapp (dónde gasta compute el descubrimiento y qué financing maximiza rutas vivas). *(Insumos externos — como el Excel canónico del operador — entran como material de referencia y calibración, no como consumidor de esta telemetría.)*

**Reglas de la separación:**
1. La Capa 1 NUNCA consulta precios, fees ni capital — mezclarla con economía es la fuente clásica de "no sé cuántas rutas hay, solo cuántas pasaron".
2. La Capa 2 es la ÚNICA que mata rutas, y cada muerte emite `(hop_tier, gate, razón, modo)` a telemetría — extensión natural del `rejection_reason` existente, ahora con dimensión tier.
3. Los knobs del panel actúan en SU capa: `max_hops`/`routes_per_tick`/`tiering` en Capa 1; toggles de financiamiento/ceilings/capital en Capa 2. Un toggle de financing **jamás** reduce las candidatas descubiertas — solo el funil lo hace visible.
4. El orden de gates es por costo creciente (liquidez es gratis consultando cache; financiamiento es aritmética; sizing es el caro al final) — early-exit honesto.

### 2.1 La definición operativa moderna

```
RUTA = (secuencia de tokens, elección de pool por pierna, modo de financiamiento)
     = (token-path, pool-choice vector, financing-mode)
```

Tres dimensiones ortogonales. Nuestro DFS ya enumera las dos primeras; la tercera es **la dimensión que activa el panel**.

### 2.2 La dimensión financiera — verificado on-chain HOY (2026-08-18)

El financiamiento determina **si una ruta existe para el sistema** (y su tamaño óptimo). Fees verificados por RPC:

| Modo | Mecánica | Fee hoy | Profundidad | CUÁNDO HACE EXISTIR LA RUTA |
|---|---|---|---|---|
| **Capital propio** | inventory WETH del executor | 0 bps | = inventory | Rutas ≤ capital propio (típicamente pequeñas) |
| **Flash loan Balancer V2/V3** | vault.flashLoan, todo el balance del vault, multi-token | **0 bps** | vault balance | Rutas de cualquier tamaño donde el vault tenga el token |
| **Morpho Blue** | pull-back exacto (fee imposible por código) | **0 bps** | idle per-market | Ídem |
| **Maker dss-flash** | mint DAI hasta debt ceiling | **0 bps** | **$500M DAI** | TODA ruta quoteada en DAI, cualquier tamaño |
| **Flash loan Aave V3** | flashLoanSimple (V4 NO tiene flash — seguir V3) | **5 bps** (gobernable — leer `FLASHLOAN_PREMIUM_TOTAL` on-chain, JAMÁS hardcodear) | $1B+/activo | El fallback universal: más cobertura token×chain |
| **Flash swap UniV2** | swap() con callback; préstamo FUSIONADO con swap | 30.09 bps mismo-token / **≈0 marginal** si el par ya es pierna de la ruta | reserves del par | Rutas cuyo ciclo ya cruza un par V2 — financing gratis |
| ~~dYdX~~ | v3 sunset 2024-10-28 | — | — | **EXCLUIR de toda tabla** (docs viejos lo listan) |

**La regla de oro que emerge del mundo:** el costo financiero es un **término POR-RUTA computado a detección** — `financing_cost_bps(provider, token, size)` — no una constante. Una ruta triangular WETH→X→WETH con flash swap en su propia pierna V2 cuesta ~0; la misma con flash loan neutro cuesta 5-30 bps y puede dejar de existir (caer bajo el break-even).

### 2.3 El sizing por topología — las matemáticas que especializan cada ruta

| Topología de ruta | Método | Fórmula / enfoque |
|---|---|---|
| **2-pool mismo par** (OrthogonalEquilibrium) | **Raíz cuadrática CERRADA, O(1)** | `k=(1−f)x_b+(1−f)²x_a; r*=(−b+√(b²−4ac))/2a` (Cyfrin); zero-fee: `p*=((√k₁+√k₂)/(x₁+x₂))²` |
| **Pool vs precio de referencia** | Fórmula de banda | `Δα*=(R_α−√(k/(γ·m_p)))+` con banda de no-trade `γ·m_p ≤ m_u ≤ γ⁻¹·m_p` (Angeris 1911.03380) — tamaño cero DENTRO de la banda es honesto, no fallo |
| **Ciclos ≥3** (HolonomicLoopResolution) | **SIN forma cerrada** (Zhang 2406.16600) | Programa convexo en control-plane O Marginal-Price root-finding (1 variable por token, ms-scale, hasta 200× más rápido que Clarabel — Bancor 2502.08258) |
| **Multi-ruta con splits** | Convexo (Angeris 2204.05238) | El óptimo verdadero; requiere subgrafo candidato → **enumerate-then-optimize** (el consenso de producción, que es exactamente nuestra forma) |

**El waterfall de costos que gobierna todo** (por qué financing cambia el SET de rutas):

```
EV_neto(ruta) = P(inclusión) × [gross(Δ) − f·Δ − Σ_fees_dex(γⁿ) − gas − tip]

gas   = FIJO por intento (~180-220k + 50-100k callback flash) → NO mueve el Δ óptimo,
        solo la viabilidad (umbral mínimo)
f·Δ   = fee flash = LINEAL en principal → BAJA el argmax del tamaño óptimo
        (He-Yang-Zhou: modelar la pierna de compra como (1+f)·p — NUNCA optimizar sin
        fee y restar después)
γⁿ    = fees DEX compuestos por hop ((0.997)² ≈ 0.994 round-trip V2)
tip   = 50-90% del neto bajo competencia (γ adaptativo, Flashbots guidance)

Ley √ universal:  Δ* ∝ √(discrepancia) × √(liquidez)
```

### 2.4 El ranking EV pre-ejecución (el orden del mundo)

1. **Screen por spread** desde tasas marginales cacheadas — piso: `min_amount_in > (gas_price×gas)/spread`
2. **Sizing** por topología (§2.3)
3. **EV neto = sized_profit − gas − financing_fee** — con staleness como feature de primera clase (blocks desde el último Sync por pierna; >99% de oportunidades muere al bloque siguiente)
4. **Simulación exacta** antes de someter (REVM — ya la tenemos en sim-ctl)
5. **Calibración retrospectiva**: replay estilo `flashbots/hindsight` (revm fork por bloque, profits = cota inferior honesta) — el template exacto para puntuar la cobertura histórica de nuestro enumerador

---

## 3. LA CONFIGURACIÓN ÓPTIMA — dinámica, operada, viva

### 3.1 Los knobs (superficie completa)

| Knob | Tipo | Efecto sobre el SET de rutas |
|---|---|---|
| `routes_per_tick` (500/600/1000) | budget de emisión | Solo RITMO (DeferNeverDrop: jamás cobertura) — más budget = ladder completo más rápido |
| `max_hops` (2..7, floor 7 en shadow) | profundidad | El tier 6-7 es medible-y-decidible (§1.3): yield por tier en shadow |
| `financing.flash_loan` | toggle | Activa/desactiva el 90%+ de las rutas grandes (sin TLS, solo rutas ≤ capital propio) |
| `financing.flash_swap` | toggle | Rutas cuyo financing óptimo es la propia pierna V2 (~0 bps marginal) |
| `financing.flash_lending` (mint) | toggle | Rutas DAI-quote de cualquier tamaño (0 bps, $500M) |
| `financing.atomic` | toggle | Patrones atómicos multi-venue (futuro L2/Solana) |
| `provider_ladder` | allowlist ordenada + `fee_ceiling_bps` por provider | Balancer(0)→Aave(5bps, leer on-chain)→UniV3-flash(tier) — la ceiling es kill-switch anti-gobernanza (el fee de Aave YA movió 0.30→0.09→0.05) |
| `graph.tiering` | core-K / floor liquidez | Tamaño del grafo podado (§1.4) — tradeoff cobertura vs latencia |

### 3.2 El canal runtime (patrón canónico del repo, ya probado)

```
Panel flotante → PUT /admin/route-discovery-config/:chain (admin-gated + audit)
  → PG source-of-truth (upsert) → Redis SET arbx:route_discovery_config:<chain>
  → PUBLISH arbx:route_discovery_config:changes
  → Worker Rust: ConfigWatcher (cache 1s, hot-reload sub-segundo) ← MISMO patrón que
    trading_config (PG→Redis mirror→pub/sub→Rust) — rehydrate al boot incluido
```

### 3.3 El panel flotante (diseño)

- **Trigger**: botón discreto (SlidersHorizontal icon) en el status strip de `/routes/discovery`
- **Cuerpo**: Sheet lateral derecho (patrón `strategy-settings-sheet.tsx`), `liquid-glass` + tokens del tema (`--card` translúcido, `--border`, radius `--radius`), sombra suave — discreto, elegante, nativo del diseño
- **Secciones**: Rutas (budget select 500/600/1000 · hops select con floor) · Financiamiento (4 toggles con badge del fee actual por provider) · Avanzado (fee ceilings por provider)
- **Feedback**: toast on-save, badge de config activa en el strip, todo optimista con rollback honesto

### 3.4 Semántica de los toggles (implementación)

El toggle de financing **NO es cosmético**: gatea en el pipeline por etapas —
1. **Discovery**: sin cambios (enumeramos TODO siempre — DeferNeverDrop)
2. **Dispatch/evaluación**: rutas cuya viabilidad requiere el modo desactivado → rechazo honesto `financing_disabled:<mode>` (visible, auditable — R8)
3. **Sizing**: el costo financiero entra al waterfall ANTES del argmax (§2.3)
4. **Provider selection**: ladder size-aware por ruta (`cost = fee_bps × size` s.t. `liquidity(provider, token) ≥ size` — reproduce el failover Aave↔Balancer observado en producción: Flashbots elige Balancer por fee; Aave gana cuando el vault no cubre el tamaño)

---

## 4. PLAN DE IMPLEMENTACIÓN — 5 fases, cada una su PR + gates (§37)

### F1 · Canal de config + worker vivo + panel flotante + FUNIL de dos capas *(la petición original, enriquecida)*
- Redis `arbx:route_discovery_config:<chain>` + PG + pub/sub + ConfigWatcher Rust (patrón trading_config)
- Worker consume `routes_per_tick` y `max_hops` VIVOS por tick (respeta floors: shadow ≥7, budget ≥1 — jamás rompe DeferNeverDrop)
- **Separación estructural de las dos capas (§2)**: discovery emite solo topología; la evaluación pasa a funil de gates G1→G5 con ledger `(hop_tier, gate, razón, modo)` en cada tick de telemetría
- **Endpoint del funil** `GET /api/route-discovery/funnel` → tabla tier × gate × modo que alimenta los paneles de la dapp (aparecidas vs sobrevivientes por condición, en vivo)
- Panel flotante Sheet glass con los knobs §3.1/§3.3 (sección Rutas = Capa 1; sección Financiamiento = Capa 2 — visualmente separadas como las capas mismas)
- Toggles financing v1: gates de Capa 2 con razón honesta `financing_disabled:<mode>`
- **Gate**: contract test del canal + test de floors + test del funil (conteos tier×gate cuadran contra oráculo sintético) + e2e Playwright del panel

### F2 · Dimensión financiera por ruta (candidatos derivados)
- Tras sizificar una ruta, instanciar variantes de financing como candidatos baratos (el sizing difiere SOLO en el término multiplicativo (1+f) — computar una vez, derivar el resto)
- Ladder size-aware por ruta + lectura ON-CHAIN de fees (Aave `FLASHLOAN_PREMIUM_TOTAL`, Balancer `getFlashLoanFeePercentage`, cache TTL corto + observación al cruzar ceiling)
- Registrar qué modo gana por ruta = datos de calibración (los priors Kelly futuros necesitan EXACTAMENTE esto)
- **Gate**: test del ladder con fees mockeados on-chain-shaped + auditoría no-hardcode

### F3 · Ranking EV con staleness
- Score por ruta: `P(inclusión) × sized_profit − gas − f·Δ`, descuento por freshness de reserves por pierna y por número de piernas
- Validación retrospectiva estilo hindsight (revm replay, cota inferior)
- **Gate**: backtest sobre histórico shadow ≥ correlación positiva rank vs realized

### F4 · Índice incremental ruta→pools (los 4.37ms del mundo)
- Mantener mapa ruta→aristas; en cada evento Sync/Swap/Slot0, re-cotizar SOLO rutas afectadas
- Dos relojes: event-driven hot + reconciliación por bloque
- **Gate**: benchmark de latencia de re-quote ≤ 2× el full-tick

### F5 · Tiering del grafo + medición de yield por hop (sobre el funil)
- Core hub (WETH+stables+top-K) + periferia con floor — poda LOGUEADA
- El funil de dos capas YA produce el yield por tier con atribución total (aparecidas vs muertas por gate); F5 lo convierte en decisión de la dapp: dónde gasta compute el descubrimiento, qué financing maximiza rutas vivas
- **Gate**: panel de yield por tier × gate en la dapp; revisión operador tras 1 semana de shadow

### Expectativas honestas (para calibrar el éxito)
- Margen neto del arbitraje atómico: **~0.62%** del volumen ($21M/$3.39B, 1 mes, EigenPhi) · 50-90% del bruto se puja al propositario · ~0.006% de oportunidades escaneadas son rentables post-gas · solo ~10% de bloques satisfacen viabilidad positiva en pares top
- Por eso shadow mide EV **pre y post-bribe** — el ranking honesto para cuando llegue live

---

## 5. FUENTES PRIMARIAS

**Discovery:** RICH (VLDB 18(11):4081, 2025) + código · Wang et al. arXiv:2105.02784 · Zhang arXiv:2406.16573 + 2504.15809 · Cherkassky-Goldberg 1999 · ABF ISCAS 2001 · Johnson 1975 · análisis red Uniswap arXiv:2503.07834 · GoldPhish USENIX'23
**Optimización:** Angeris arXiv:2204.05238 + 1911.03380 + 2302.04938 · Marginal-Price Bancor arXiv:2502.08258 · Hermes ICDCS'25 · PRIME arXiv:2603.08337 · Multi-Path arXiv:2607.22540 · competencia arXiv:2507.08302 · bandas arXiv:2305.14604
**Instrumentos (verificado on-chain 2026-08-18):** Aave V3 docs + FLASHLOAN_PREMIUM_TOTAL=5bps · Balancer V2/V3 docs (0 fee) · Morpho Blue source (fee imposible) · dss-flash MIP25 · Uniswap V2/V3 flash docs · dYdX sunset notice · Aave V4 (sin flash en core)
**Producción:** flashbots/{simple-arbitrage, simple-blind-arbitrage, hindsight, docs} · paradigmxyz/artemis · evm-amm-search/state · pool_sync · libevm/subway · whack-a-mole (SolidQuant) · hotpath.rs · Uniswap SOR · noxx/cvxpy

---

*Doctrina viva. Cada fase cierra con gate + telemetría. El enumerador exhaustivo probado (DeferNeverDrop) es la fundación — todo lo demás se construye sobre la garantía de que ninguna ruta se pierde, jamás.*
