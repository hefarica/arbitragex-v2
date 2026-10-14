# ADM-t5 — Track 𝒯 frontend/bus (FAMILIA G §12): coalescing + seq guards + watchdog

> Task t5 · frontend-dev · attempt `4b1a1929-4e52-42fb-8bbe-3ba8656ad9c7` · rama `feat/exp-2026-20261013` @ base 21feefb32 · 2026-10-13.
> Disciplina: π(t), μ(t;θ) vs x_ref, λ(t), δ = max(0.02·|J|, δ_abs). **δ_abs = 0.005** (derivada por t9 del x_ref t16: XREF_BASELINE.md/.json sha256 7a03e242…/089ba911…).
> Fail-honest: todo número de este registro cita su comando + salida literal. μ_wire NO medible ⇒ declarado, jamás rellenado (RG-01).

## 1. Registro ADM-n por candidato

| t | π(t) | μ_mecanismo (medido) | μ_wire (vs x_ref t16) | λ(t) | \|J\| | δ = max(0.02·\|J\|, 0.005) | veredicto |
|---|---|---|---|---|---|---|---|
| **t_bus** — priceBus coalescing `ARBX_PRICE_MIN_INTERVAL_MS`(default 100) + snapshot + seq monotónico por par | G-core | **0.9990** (bench §2.1) | **0 — `no wire producer`** (§3) | bajo (archivo nuevo aditivo) | 0.999 (colapsar ráfaga same-ms a 1 frame) | 0.01998 | **ADM-1 ✓ mecanismo** / **veredicto 0 wire** (razón `no wire producer`, estado terminal legítimo — ratificado por capitán) |
| **t_seq** — createSeqGuard cliente (STREAM-SEQ-01, replay ⇒ 0 efecto) | G-core | **1.0** — 2000/2000 no-nuevos rechazados, 200/200 vivos aceptados (vitest §2.2) | **0 — `no wire producer`** (§3) | bajo (función pura) | 1.0 (replays con efecto cero) | 0.02 | **ADM-2 ✓ mecanismo** / **veredicto 0 wire** (ídem) |
| **t_watchdog** — REEMITIR en vez de reconectar (mata `already in CLOSING or CLOSED state`) | G-core | **1.0** — 3→0 disconnects forzados en ventana 1h de silencio (modelo §2.3, citas de código) | **NO COMPUTADO** — t16 §7: instrumentos WS no disponibles desde wire público; sin productor no hay eventos de silencio que medir | bajo (hook nuevo; no toca usePricesStream) | 1.0 (eliminar disconnect-storm en recuperación de silencio) | 0.02 | **ADM-3 ✓ mecanismo (modelo)** / **veredicto 0 wire** (ídem) |
| **t_lifecycle** — socket-lifecycle.ts callbacks opcionales onPriceTick/onPricesSnapshot | G-wiring | **no-regresión**: 12 tests pre-existentes VERDES sin modificación + 7 tests nuevos (§2.4) | n/a por diseño (habilitador: sin callback ⇒ socket idéntico) | bajo (listeners condicionales) | — (mandato α11: integración manual OBLIGATORIA del diff v4) | — | **ADM-4 ✓ wiring con mandato** (no es técnica de mejora: es el cableado de t_feed/t_bus end-to-end) |

**Parada de admisión**: ningún candidato excluido del árbol (4/4 implementados). El **veredicto 0 de wire** se registra para los CLAIMS de wire de t_bus/t_seq/t_watchdog con razón `no wire producer` — A7 (t14) debe llevar estas filas. t excluido no aplica: nada quedó por debajo de δ a nivel mecanismo.

## 2. Evidencia por candidato (comando + salida literal)

### 2.1 μ(t_bus) — bench sobre el bus REAL (`backend/api-server/src/prices/priceBus.ts`)

Comando (node v24.12.0, `--experimental-strip-types`, script temporal fuera del repo):
```
node --experimental-strip-types bus-bench.mts   # publica 1000 ticks same-ms de "BENCH/BUS"
```
Salida literal:
```
{"N":1000,"published":1,"coalesced":999,"mu_coalesced":"0.9990"}
```
μ_mecanismo = 999/1000 = **0.9990** ≥ δ=0.01998 ⇒ ADMITIDO. Contrato v4 adicional verificado: `npx vitest run src/prices` → `priceBus.test.ts (4 tests)` ✓ (seq monotónico, coalescing, snapshot fresco, anti-fuga 25 subscripciones).

### 2.2 μ(t_seq) — vitest sobre el contrato canónico

Comando:
```
node node_modules/vitest/vitest.mjs run src/prices-contracts.test.ts   # cwd shared-ts
```
Salida literal:
```
 ✓ src/prices-contracts.test.ts (7 tests) 6ms
```
Bench interno (asserts): `200 live accepted; 1000 replays and 1000 out-of-order ALL rejected (0 state effect)` — `accepted=200, replayRejected=1000, outOfOrderRejected=1000` ⇒ μ=**1.0** ≥ δ=0.02 ⇒ ADMITIDO.

### 2.3 μ(t_watchdog) — simulación de modelo (1h de silencio, tab visible)

Políticas citadas por código: LEGACY `usePricesStream.ts:204-220` (check 15s; >90s ⇒ `socket.disconnect(); socket.connect()` en :218-219, máx 3, luego degrada a polling) vs V4 `usePricesFeed.ts:39-45` (check 5s; >45s ⇒ `setStale(true)`, CERO disconnect — "Reemitir en vez de reconectar: no resetea el contador interno de 5").

Salida literal (sandbox_js):
```
{"window_h":1,"legacy":{"forcedDisconnects":3,"forcedReconnects":3,"degradedToPolling":1},"v4":{"forcedDisconnects":0,"forcedReconnects":0,"staleFlagEvents":712},"mu_watchdog_disconnect_elimination":1}
```
μ_mecanismo = **1.0** ≥ δ=0.02 ⇒ ADMITIDO (modelo). NOTA-RECONEXION v4 respetada: CONNECT_OPTS NO se cambia (`socket-lifecycle.ts:50` — `reconnectionAttempts: 5, timeout: 2000` intacto, verificado por test pre-existente "passes the configured url and connection opts").

### 2.4 μ(t_lifecycle) — no-regresión + brazo prices

Comandos y salidas:
```
node node_modules/vitest/vitest.mjs run features/opportunities lib/prices lib/hooks/usePricesStream.test.ts   # cwd frontend
 ✓ features/opportunities/socket-lifecycle.prices.test.ts (7 tests) 11ms
 ✓ features/opportunities/socket-lifecycle.test.ts (12 tests) 15ms        ← INTACTO, sin modificación
 ✓ lib/hooks/usePricesStream.test.ts (9 tests) 4ms                        ← INTACTO, sin modificación
node node_modules/vitest/vitest.mjs run    # cwd frontend (SUITE COMPLETA)
 Test Files 141 passed (141)   Tests 1418 passed (1418)
```
Contrato del parche verificado por los 7 tests nuevos: sin callbacks ⇒ exactamente 4 listeners y 4 offs (socket observable idéntico); con onPriceTick ⇒ 5/5; con ambos ⇒ 6/6; `connect` emite `subscribe:prices` SIN payload (1 arg — shape del dispatcher α11); reconexión re-emite (2 emits payload-less); `prices:tick`/`prices:snapshot` forwardean a los callbacks.

## 3. μ_wire = 0 — `no wire producer` (RG-01, estado terminal legítimo)

El wire vivo (https://arbx.ape-tv.net, x_ref t16) NO tiene eventos `prices:tick`/`prices:ready`: el paquete v4 NO trae publicador hacia `priceBus` (solo bus→room fan-out; verificado: 0 llamadas a `priceBus.publish` fuera de tests en el paquete y en mi aterrizaje). En consecuencia μ_wire de t_bus/t_seq/t_watchdog contra x_ref es **0 por ausencia de wire**, y así se registra — veredicto 0 wire, razón `no wire producer`.

**Seam "publicador pendiente" (para el futuro bridge, con ruta:línea):**
- Punto de entrada: `backend/api-server/src/prices/priceBus.ts:publish()` (actualmente sin llamantes en runtime).
- Puerta de salida al wire: `backend/api-server/src/websocket.ts:registerPricesFanout()` → `broadcastPriceTick()` (`io.to('prices').volatile.emit('prices:tick', …)`).
- Bridge natural futuro: searcher-rs price_worker → Redis (canal propio) → puente en api-server que llame `priceBus.publish({pair, mid, ts, src})` — MISMO patrón que `prices-stream.ts:subscribeToPriceUpdates()` (Redis pub/sub → broadcast), sin tocar el hash canónico (fuente única).

## 4. Dispatcher α11 — decisión documentada (SEAM t17-FALSATOR)

Restricción VINCULANTE (intel backend-dev + ratificación capitán): el parche v4 anotado, aplicado tal cual, creaba un SEGUNDO handler de `subscribe:prices` — y siendo `io.on('connection')` aditivos, AMBOS disparaban (contaminación cruzada: legacy recibiría `{ticks}` extra; v4 recibiría `prices:error invalid_chain_id`).

**Diseño implementado — dispatcher por shape de payload, 2 listeners físicos, 1 dispatch observable:**

| Shape de payload | Brazo que actúa | Brazo que hace yield | Evidencia |
|---|---|---|---|
| `undefined`/`null` (v4, emit sin args) | **v4** `websocket.ts` (`socket.on(PRICES_SUBSCRIBE_EVENT, …)` con guard `payload !== undefined && payload !== null ⇒ return`) → join `prices` + `prices:ready` + `prices:snapshot {ticks}` unicast | legacy `prices-stream.ts:110-115` (guard null-yield) | `prices-dispatch.test.ts` caso 1 ✓ (recibe ready+snapshot, CERO prices:error) |
| `{chain_id: N}` válido | **legacy** `prices-stream.ts:110-119` → join `prices:<chain_id>` + snapshot per-chain | v4 (guard shape) | caso 2 ✓ (snapshot legacy, CERO prices:ready) |
| `{}` o `chain_id` inválido | **legacy** → `prices:error invalid_chain_id` | v4 (guard shape) | caso 3 ✓ (contrato prices-stream.test.ts:85-96 INTACTO — ese test pasó sin modificación) |

- Fan-out aislado: `publish()` → `prices:tick` SOLO al suscriptor del room global (caso 4 ✓ — el suscriptor legacy NO recibe tick).
- **Guard en el seam compartido**: `prices-stream.ts:104-115` — `+6/-0` (1 línea de código `if (payload === undefined || payload === null) return;` + 5 de comentario). Ventana α11 respetada: ese es el ÚNICO edit mio en el archivo; t4 aterriza sus 2 exports después (secuencia anti-carrera confirmada al capitán, msg 55e77f3c).
- Orden de registro preservado: `index.ts:1793` gateway → `:1815` attachPriceRooms (producción intacta — el test prices-dispatch replica ese orden).
- Registro de fan-out: especificación v4 "llamar UNA vez tras crear el io" implementado como **once-per-process dentro de `setupWebSocketGateway`** (websocket.ts, bloque antes de `return io`) — `index.ts` queda INTACTO (mapa de archivos α11); el export `registerPricesFanout` queda disponible para tests con io dedicados.

## 5. Mapa de archivos (ownership α11 — productor t5)

| Archivo | Acción | sha256 |
|---|---|---|
| `shared-ts/src/prices-contracts.ts` | NUEVO (v4 verbatim: schemas zod, 5 constantes, isPriceTick, createSeqGuard) | ver §7 |
| `shared-ts/src/prices-contracts.test.ts` | NUEVO (7 tests: contrato + bench μ_seq) | ver §7 |
| `backend/api-server/src/prices/priceBus.ts` | NUEVO (v4 + tipo estructural local PriceTick — ver desviación D2) | ver §7 |
| `backend/api-server/src/prices/priceBus.test.ts` | NUEVO (v4 + determinización del test de snapshot — ver D3) | ver §7 |
| `backend/api-server/src/prices/prices-dispatch.test.ts` | NUEVO (4 tests del dispatcher α11 sobre gateway REAL) | ver §7 |
| `backend/api-server/src/prices/prices-contract-parity.test.ts` | NUEVO (3 tests de paridad contrato↔mirrors) | ver §7 |
| `backend/api-server/src/websocket.ts` | MODIFICADO (+constantes locales, +brazo v4, +broadcastPriceTick/registerPricesFanout, +import priceBus) | ver §7 |
| `frontend/features/opportunities/socket-lifecycle.ts` | MODIFICADO (parche manual del diff anotado: callbacks opcionales + emit payload-less + listeners/off condicionales) | ver §7 |
| `frontend/features/opportunities/socket-lifecycle.prices.test.ts` | NUEVO (7 tests del brazo prices) | ver §7 |
| `frontend/lib/prices/usePricesFeed.ts` | NUEVO (v4 VERBATIM, imports relativos intactos — tsc frontend EXIT=0) | ver §7 |
| `backend/api-server/src/prices-stream.ts` | **SEAM COMPARTIDO** (+6/-0 guard null-yield :104-115) | intermedio: `3c26994aedbc80c9e28dfec2e6f40916872d8118aae96659d2a01683e408a8b5` (post-guard, pre-exports-t4) |

**Coexistencia probada (nada se borró/reemplazó):** `usePricesStream.ts` (266 líneas) y `prices-stream.test.ts` SIN MODIFICACIÓN — 9 y 4 tests verdes respectivamente dentro de las corridas completas. `prices-stream.ts` solo recibió el guard ratificado.

## 6. Registro ℛ (seam compartido — 2 filas por archivo, verificador→t17)

- `(prices-stream.ts, productor=t5, edit=guard null-ruteo α11 +6/-0 @ :104-115, sha_intermedio=3c26994a…8a8b5, verificador=t17-FALSATOR)` — confirmado al capitán (msg 55e77f3c) ANTES del aterrizaje de t4.
- `(prices-stream.ts, productor=t4, edit=exports buildSnapshot/nextSeq [pendiente t4], sha_final=PENDIENTE t4 → diff intencional t6)`.

## 7. Verificación final (batería completa, salidas literales)

Hashes de los entregables (sha256, prefijo 16; `prices-stream.ts` = sha intermedio post-guard, pre-exports-t4):

```
cbb36d513abed22e  shared-ts/src/prices-contracts.ts
b80e36ece2a2360c  shared-ts/src/prices-contracts.test.ts
21ad2863536ad5e5  backend/api-server/src/prices/priceBus.ts
546610c0b735f6b7  backend/api-server/src/prices/priceBus.test.ts
d146de61b10ef497  backend/api-server/src/prices/prices-dispatch.test.ts
b702545c691a50a2  backend/api-server/src/prices/prices-contract-parity.test.ts
e928e666e68b3757  backend/api-server/src/websocket.ts
503560ccb2734a55  frontend/features/opportunities/socket-lifecycle.ts
10bcc28bcdd3193e  frontend/features/opportunities/socket-lifecycle.prices.test.ts
5f920c6c4fc29fc0  frontend/lib/prices/usePricesFeed.ts
d019391608d8ac7d  backend/api-server/src/prices-stream.ts          ← seam compartido (intermedio)
```

Comandos y salidas:

```
# api-server (cwd backend/api-server)
npx tsc --noEmit -p tsconfig.json            → TSC_EXIT=0
node node_modules/vitest/vitest.mjs run src/prices src/prices-stream.test.ts src/websocket.test.ts src/edge-parity.test.ts
  → Test Files 7 passed (7) · Tests 31 passed (31)
# frontend (cwd frontend)
npx tsc --noEmit                             → TSC_EXIT=0
node node_modules/vitest/vitest.mjs run      → Test Files 141 passed (141) · Tests 1418 passed (1418)
# shared-ts (cwd shared-ts)
npm run build                                → EXIT=0 (dist generado; prices-contracts compila en su tsc)
node node_modules/vitest/vitest.mjs run      → Test Files 8 passed (8) · Tests 46 passed (46)
```

## 8. Registro de desviaciones (v4 verbatim → adaptado, con razón)

| # | Desviación | Razón | Mitigación |
|---|---|---|---|
| D1 | El diff v4 citaba contexto `onStreamEvent?` que NO existe en la base real | Anotación v4 asumía un estado previo distinto; solo se agregaron las líneas `+` | Test "sin callbacks ⇒ 4 listeners/4 offs" congela el contrato |
| D2 | `websocket.ts` NO importa constantes de `shared-ts/src/prices-contracts.ts`; declara literales locales + `priceBus.ts` declara tipo estructural local `PriceTick` | **TS6059 medido**: api-server tsconfig (`moduleResolution NodeNext` + `rootDir "src"`) PROHÍBE imports relativos cross-package en build (salida literal: `error TS6059: File …prices-contracts.ts is not under 'rootDir'`). El diff v4 ORIGINAL declaraba `PRICES_ROOM` localmente — esta forma ES v4-fiel | `prices-contract-parity.test.ts` (3 asserts: literales byte-identical, campos del schema, safeParse round-trip) — drift-gate en CI. Alternativa limpia futura (propuesta al capitán): 1 línea `export * from "./prices-contracts.js"` en shared-ts/src/index.ts + import `@arbx/shared` — requiere escalación (archivo fuera de mapa α11) |
| D3 | `priceBus.test.ts` test "snapshot solo devuelve lo fresco" determinizado (timestamps explícitos ±6000ms) | El original v4 era FLAKY: `snapshot(0)` sobre tick del mismo ms (`0 <= 0` ⇒ true) — pasó en run1, falló en run2 del mismo árbol (evidencia en esta sesión) | Semántica del bus (`<=`) INTACTA; el test ahora fija edades explícitas (viejo excluido, fresco incluido) |
| D4 | `registerPricesFanout` se invoca inside `setupWebSocketGateway` (once-per-process) en vez de desde `index.ts` | `index.ts` fuera del mapa α11; la spec v4 ("llamar UNA vez tras crear el io") queda satisfecha literalmente | Export disponible para io dedicados de tests; documentado §4 |
| D5 | `connect` emite `subscribe:prices` SIN condicional (todos los sockets de la factory) | Es el diseño v4 (línea `+` incondicional del diff: sala pública sin token) — el hook v4 NO crea socket propio | Test "connect emits … payload-less" + filtro de emits en tests pre-existentes FE-0047 (verde) |

## 9. Insumos y trazabilidad

- Referencia v4: `.tmp-inbox/v4-diffs/` (00-LEEME.md, PATCHES/socket-lifecycle.ts.diff, PATCHES/websocket.ts.diff, PATCHES/nota-reconexion.md, frontend/lib/prices/usePricesFeed.ts, shared-ts/src/prices-contracts.ts, backend/api-server/src/prices/priceBus{.ts,.test.ts}, .github/workflows/prices-contract.yml). INTEGRADO manda (t2 §a); ZIPs previos = referencia.
- x_ref: `XREF_BASELINE.md`/`.json` (t16, sha256 7a03e242…/089ba911…) — δ_abs=0.005 derivada por t9 (release R-20261013T085048Z).
- Decisiones vinculantes: α11 ownership + coexistencia (capitán); dispatcher unificado por shape de payload (capitán + intel backend-dev); guard ratificado con ventana +6/-0 y secuencia anti-carrera; fail-honest del bus sin publicador como estado terminal legítimo.

> Regla de hash §11-bis: el sha256 de ESTE archivo debe quedar registrado dentro de A1 al regenerarlo (t6).
