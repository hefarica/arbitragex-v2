# 18. GESTIÓN DE RIESGO CUANTITATIVA PARA ARBITRAGE

> Parte II de la biblioteca (refs 11-22). Extiende el núcleo v1.0.0 (§1-§10) en profundidad de
> riesgo cuantitativo; no lo repite. Citas al núcleo como "ver núcleo §N".

CUÁNDO CARGAR ESTA REFERENCIA: diseñar o auditar sizing/caps de una estrategia (per-trade, per-bloque, per-pool correlacionado); calibrar stop-loss, cooldowns o breakers tras un drawdown o racha perdedora; investigar caída de inclusion rate o sospecha de selección adversa/backrun; definir checks de oráculo, screening de contratos o el runbook de key compromise; auditar un backtest antes de creerle al Sharpe; calcular EV con probabilidad de inclusión o sensibilidad al gas.

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Taxonomía de riesgo | Matriz capa→riesgo→señal→acción | Riesgo sin señal medible ≤1 bloque y acción automática = riesgo aceptado a ciegas |
| Toxicidad de flujo | VPIN (Easley/O'Hara) adaptado a AMM | El swap AMM tiene dirección nativa: no hay que clasificar buyer/seller |
| Selección adversa | Win-matrix por bloque vs competidores + inclusion rate | Distinguir "outbid" (pricing tarde) de "detectado" (flujo tóxico) |
| Sizing | Kelly asimétrico `f* = p/ℓ − q/g`, fractional 0.25-0.5 | Apostar 2× Kelly = crecimiento cero; el error de estimación convierte full-Kelly en overbet |
| EV de bundle | `EV = P_incl·(P_hold·π − (1−P_hold)·gas_burn)` | `P_incl` se mide con stats del relay, nunca se asume |
| Exposición correlacionada | Cap por pool: Σ amount_in de rutas vivas que cruzan P | Mismo pool en N rutas = 1 apuesta, no N |
| Drawdown | MaxDD / Calmar / recovery factor + ladder sesión/día/semana | Cada nivel del ladder define quién puede re-habilitar |
| Riesgo de ruina | `P ≤ q^(ln r / ln(1−c))` | Reducir el cap c a la mitad ≈ duplicar N ⇒ elevar la cota al cuadrado |
| Cola izquierda | EVT-POT + GPD + Hill plot + mean excess | Atomicidad trunca la cola de mercado; queda la cola de fallo correlacionado |
| Stress tests | gas 10×, oráculo ±10%, depeg, exploit, builder censor | Stress = cuantificar degradación y verificar breakers, no solo aprobar |
| Oráculo | Chainlink staleness + mediana multi-fuente + breaker por divergencia | Nunca derivar `minOut` del precio del pool que la propia ruta toca |
| Backtest honesto | Walk-forward + universo point-in-time + replay en Anvil fork | El gap sim-vs-real se descompone y se mide en shadow, no se discute |

## 18.1 TAXONOMÍA DE RIESGO Y MATRIZ CAPA→RIESGO→SEÑAL→ACCIÓN

Cinco familias de riesgo en arbitraje on-chain:

1. **Mercado**: reserves/slippage se mueven entre la sim y la inclusión; gas price; depeg de
   estables; desaparición del edge por competencia.
2. **Contrato/contraparte**: pool explotable, pausable, blacklistable, fee dinámico, token
   fee-on-transfer; el builder/relay como contraparte de inclusión (censura, latencia).
3. **Operativo**: key compromise, fat-finger, misconfig, error humano de proceso.
4. **Infra**: divergencia/cuota RPC, Redis/PG caídos, red del VPS, relay down.
5. **Técnico/latencia**: detección tardía, sim lenta, cola de eventos saturada; incluir el
   riesgo de observabilidad misma (LOGFLOOD-01: un log-flood destruyó el diagnóstico).

Principio rector: **todo riesgo aceptado debe tener (a) una señal medible con ventana ≤1
bloque y (b) una acción automática**. Si falta (a), el riesgo es invisible; si falta (b), la
mitigación es una esperanza. Esto extiende el Decisor de Riesgo y los circuit breakers del
núcleo (§1.2, §9) con la capa cuantitativa que los calibra.

| Capa | Riesgo | Señal (métrica, ventana) | Acción automática |
|---|---|---|---|
| Mercado | Slippage realizado > simmed | EWMA de `(out_real − out_sim)/out_sim` por pool, 100 trades | Subir `minOut`/margin del pool; excluir si persiste >5m |
| Mercado | Gas spike | `base_fee` actual vs EWMA 5m; ratio >2× | Filtrar universo por `g*` (§18.5); pausa si 100% del universo queda EV<0 |
| Mercado | Depeg estable | \|P_pool − P_ref multi-oráculo\| > umbral | Excluir pares del token; breaker de divergencia (§18.6) |
| Contrato | Pool pausable/blacklist | Screening pre-trade (§18.7) | Exclusión de ruta antes del sizing |
| Contrato | Fee dinámico sube post-sim | Re-leer feeParams en el bloque objetivo (re-sim) | Cancelar bundle si fee > cap por ruta |
| Infra | RPC divergente/lento | Quorum 2/3 y delta >500ms (núcleo §1.2) | Bloqueo táctico: no despachar |
| Infra | Relay/builder censor | `mev_getBundleStatsV2`: rejects y no-inclusions por relay | Fail-closed; sin fallback a mempool público por encima del cap |
| Técnico | Latencia detección→submit | p99 del histograma vs presupuesto por estrategia | Reducir universo (menos cómputo) y bajar agresividad |
| Operativo | Misconfig de límites | Boot fail-fast + diff de config al deploy | Crash on boot (RULE 00); alarm en deploy con diff |

Los caps forman una pila anidada — cada capa nunca puede exceder a la que la contiene, y el
check se hace en orden inverso al despacho (trade→pool→bloque→sesión→día→semana):

```
┌ cap semanal (stop-loss drawer; re-habilita: operador) ─────────┐
│ ┌ cap día (re-habilita: operador) ──────────────────────────┐  │
│ │ ┌ cap sesión (re-habilita: cooldown automático) ────────┐  │  │
│ │ │ ┌ cap por bloque (Σ principal vivo en el bloque) ──┐  │  │  │
│ │ │ │ ┌ cap por pool (exposición agregada, §18.3) ──┐  │  │  │  │
│ │ │ │ │ ┌ cap por trade (Kelly fraccional + hard) ┐ │  │  │  │  │
│ │ │ │ │ └────────────────────────────────────────┘ │  │  │  │  │
```

## 18.2 SELECCIÓN ADVERSA Y TOXICIDAD DE FLUJO (VPIN)

**VPIN** (Easley, López de Prado, O'Hara, 2012) mide la probabilidad de que el volumen
transado provenga de agentes informados. Se computa sobre buckets de volumen fijo (no de
tiempo), clasificando cada unidad como compra o venta:

```
VPIN = Σ_{i=1..n} |V_buy(i) − V_sell(i)| / (n · V_bucket)
```

En un AMM la clasificación es trivial y exacta: cada swap declara su dirección
(`token0→token1` vs `token1→token0`). Bucket por `V_bucket ≈ volumen_medio_diario/50`.
Interpretación: VPIN alto ⇒ el flujo es mayormente unidireccional/informado ⇒ el precio del
pool se moverá contra cualquiera que cite contra ese flujo. Para un searcher: VPIN alto en un
pool significa que el edge medido sobre flujo reciente decae más rápido de lo que la sim
sugiere — el precio simado ya fue consumido.

**Order-flow imbalance (OFI) de reserves**: sobre ventana corta (p.ej. 20 swaps),
`I = (Δx⁺ − Δx⁻)/(Δx⁺ + Δx⁻)` por token del par. `|I|` sostenido alto = flujo direccional:
ensanchar el margen de `min_profit` exigido a rutas que tocan ese pool.

**Señales de que te están backrunneando o cancel-racing** (defensa; para la frontera ética
ver `arbx-mev-ethics-gate`):

| Síntoma | Diagnóstico | Respuesta |
|---|---|---|
| Sim pass alto + inclusion rate ~0 + mismo competidor en el bloque objetivo | Outbid (tu bid o tu latencia, no tu sim) | Subir bid (bid shading, §18.3); medir latencia §18.9 |
| Inclusion alta + PnL neto decreciente | Pagando de más por inclusión | Bajar bid; el óptimo está en `(π−b)·P_win(b)`, no en P_win sola |
| Tx pública ejecutada a peor precio que el firmado, sistemáticamente | Eres víctima de backrun | Migrar a ruta privada (`eth_sendPrivateTransaction`, núcleo §1.2) |
| Intents/order-flow propio llenado por terceros justo al límite | Cancel-racing / OFA te lee | Expiry corto, flujo exclusivo, o dejar de publicar ese intent |
| VPIN del pool sube + tus reverts on-chain suben | Flujo tóxico consume el edge pre-inclusión | Cooldown del pool; re-medir antes de volver |

Escalera de respuesta operativa, de menos a más costosa: (1) subir `min_profit_threshold`
y/o bajar bid; (2) migrar sumisión a rutas privadas (bundles con `mev_sendBundle` al relay,
sin exposición a mempool público); (3) exclusión temporal del pool/token; (4) pausa de la
estrategia y re-medición completa de `P_incl`. La respuesta nunca es "atacar de vuelta".

## 18.3 SIZING: KELLY ASIMÉTRICO, FRACTIONAL KELLY, EV CON INCLUSIÓN

**Derivación** para payoff asimétrico: apostar fracción `f` del bankroll; con prob `p` gana
fracción `g`, con prob `q=1−p` pierde fracción `ℓ`. Maximizar crecimiento log:

```
G(f)  = p·ln(1 + f·g) + q·ln(1 − f·ℓ)
G'(f) = p·g/(1+f·g) − q·ℓ/(1−f·ℓ) = 0
     ⇒ f* = (p·g − q·ℓ)/(g·ℓ) = p/ℓ − q/g        (Kelly asimétrico)
```

Aproximación continua (Merton): `f* ≈ μ/σ²`. **Interpretación**: `f*` es la fracción del
capital *en riesgo* — con flash loan eso es gas de reverts + inventario atascado (solo
piernas no-atómicas) + fee del principal,
NO el principal del flash loan. Kelly sobre el notional es un error clásico de 2-3 órdenes
de magnitud que infla el sizing.

**Por qué fractional Kelly (0.25-0.5×)**: el crecimiento a fracción `c·f*` es
`G(c·f*) = (2c − c²)·G(f*)` (caso continuo):

| c | Crecimiento relativo | Comentario |
|---|---|---|
| 0.25 | 0.44 | Casi todo el crecimiento, drawdown ínfimo |
| 0.50 | 0.75 | Estándar con estimación ruidosa |
| 1.00 | 1.00 | P(drawdown ≥50%) ≈ 0.5 a largo plazo |
| 1.50 | 0.75 | Mismo crecimiento que 0.5× con 9× su varianza |
| 2.00 | 0.00 | Sobre-apuesta total: crecimiento nulo |

La asimetría es brutal: si tu edge real es la mitad del estimado y apuestas el Kelly del
estimado, estás efectivamente en `c=2` ⇒ crecimiento cero. Con `p` estimado de muestras
pequeñas (y con drift según el régimen de competencia), 0.25-0.5× es lo único defendible.

**EV integrando probabilidad de inclusión** (el bundle es una subasta first-price sellada
contra otros searchers):

```
EV(bundle) = P_incl · ( P_hold·π_net − (1−P_hold)·gas_burn ) − c_intent
  P_incl  = win-rate del bundle contra bids competidores (se MIDE con los stats
            de inclusión del relay, p.ej. mev_getBundleStatsV2 en Flashbots)
  P_hold  = prob. de que la sim siga válida on-chain en el bloque objetivo (re-sim)
  π_net   = profit bruto − bid (coinbase transfer) − flash fee − protocol fees
  gas_burn = gas si el bundle incluye y revierte (evitable marcando la tx como
             no-revertible: solo despachar si la re-sim es verde)
```

Bid shading: maximizar `(π−b)·P_win(b)`; no hay forma cerrada — se mide la curva empírica
bid→win por pool y por hora. `P_incl` degradado no es ruido: es el precio de la competencia
y debe entrar tanto al EV como al Kelly (reduce `p` efectiva).

**Caps y exposición correlacionada**. Un cap absoluto por trade (fracción del equity o
techo en USD del capital en riesgo) y un cap por bloque. El riesgo sutil: varias rutas
vivas que cruzan el MISMO pool no son apuestas independientes — si P se mueve o es
explotado, fallan a la vez:

```rust
// Patrón: exposición agregada por pool ANTES de despachar el siguiente plan
fn pool_exposure(live: &LivePlans, pool: Address) -> U256 {
    live.iter()
        .filter(|p| p.route().contains_pool(pool))
        .map(|p| p.capital_at_risk())
        .fold(U256::zero(), U256::add)
}
// gate: pool_exposure(...) + capital_at_risk_nuevo <= cap_por_pool
```

Regla: el riesgo marginal de la n-ésima ruta por el mismo pool crece convexo con la
concentración; `cap_por_pool` debe escalarse con la liquidez del pool (p.ej. ≤0.5% de sus
reservas), no con el número de rutas.

## 18.4 DRAWDOWN, STOPS, COOLDOWN Y RIESGO DE RUINA

Métricas sobre la curva de equity (del ledger paper/shadow, núcleo §3.2):

- **MaxDD** = máximo pico-a-valle. **Calmar** = CAGR/MaxDD. **Recovery factor** =
  P&L neto total/MaxDD. **Time-under-water** = tiempo desde el último pico.
- El Calmar de una estrategia de arbitraje sana es alto porque la cola está truncada por
  atomicidad; un Calmar que se degrada mes a mes es la primera señal de que la competencia
  se comió el edge (antes de que el P&L absoluto lo muestre).

| Nivel | Umbral (frac. capital asignado) | Acción | Re-habilitación |
|---|---|---|---|
| Sesión | −2% | Pausa automática 30-60 min + re-medir gas/P_incl | Cooldown automático |
| Día | −4% | Halt hasta revisión | Operador (explícito) |
| Semana | −8% | Halt + post-mortem escrito | Operador + post-mortem |

**Cooldown tras racha perdedora**: k pérdidas consecutivas ⇒ pausa 15-30 min. Racional: las
rachas correlacionan con cambio de régimen (gas spike, competencia nueva, depeg), no con
mala suerte; el cooldown da tiempo a que las EWMA de §18.1 reflejen el nuevo régimen. Si el
sizing usa win-rate reciente, verificar el signo: una racha NUNCA debe subir la agresividad.

**Riesgo de ruina con caps**. Definir ruina como equity ≤ `r·E₀` (p.ej. `r=0.5`). Con
pérdida por trade truncada a `c` (fracción del equity actual, cap duro):

```
Pérdidas netas consecutivas para tocar r:   N = ln(r)/ln(1−c)
Cota:  P(ruina por racha) ≤ q^N            (q = prob. de pérdida por trade)

r = 0.50, q = 0.4:
  c = 10%  →  N = 6.6   →  P ≤ 2.4e-3
  c =  4%  →  N = 17.0  →  P ≤ 1.7e-7
  c =  2%  →  N = 34.3  →  P ≤ 2.2e-14
```

Por qué los caps la reducen **geométricamente**: `N ≈ |ln r|/c`, así que reducir `c` a la
mitad duplica `N` y eleva la cota al cuadrado (cada punto de cap extra multiplica la
protección exponencialmente). Nota honesta: la cota asume independencia; los escenarios de
§18.5 (fallos correlacionados en un mismo bloque/builder) violan esa suposición y son
exactamente para lo que sirve el cap por bloque.

## 18.5 COLA IZQUIERDA: EVT Y STRESS TESTS OBLIGATORIOS

**EVT conceptual** (peaks-over-threshold, Pickands 1975 / Balkema–de Haan 1974): los
excesos `X−u` sobre un umbral `u` alto convergen a una Pareto Generalizada
`GPD(ξ, σ)`; el shape `ξ` gobierna la cola (`ξ>0` pesada, `ξ=0` exponencial, `ξ<0`
acotada).

```
Hill (ξ>0):  ξ̂ = (1/k)·Σ_{i=1..k} [ ln X_(n−i+1) − ln X_(n−k) ]
Mean excess: e(u) = E[X−u | X>u]   → lineal creciente en u ⇒ cola pesada
k se elige en la meseta del Hill plot (ξ̂ estable en rango amplio de k), no en un punto
```

Aplicado a: distribución de pérdida diaria realizada (paper ledger), gas burn agregado por
bloque, slippage adverso realizado-vs-sim. Uso operativo: fijar stops y alertas POR ENCIMA
del ruido (p.ej. p99 empírico × factor) y verificar con el mean-excess plot si la cola de
pérdidas es pesada (`ξ>0` ⇒ los extremos no "se promedian": los stops deben ser duros, no
estadísticos).

**Insight estructural**: la atomicidad del bundle trunca la cola de mercado (revert =
coste conocido, no pérdida abierta). Las colas que quedan son (a) fallo correlacionado
muchos bundles a la vez (gas spike, builder censor), (b) inventario no-atómico
(cross-block, CEX-DEX), (c) inventario congelado por blacklist/pausa post-compra.

**Batería de stress obligatoria** (antes de promover cualquier config, y en CI):

| Escenario | Recalcular | Criterio |
|---|---|---|
| Gas 10× | EV del universo con `base_fee×10` | Degradación cuantificada; breakers disparan; % supervivientes reportado (0 es válido si es honesto) |
| Oráculo ±10% | Mark de inventario + filtros de precio | Ninguna ruta sobrevive SOLO por la dirección favorable; breaker por divergencia no dispara con shock de fuente única |
| Depeg estable | Universo con pares del stable | Cero rutas "arb al infinito" contra pools pegados; filtro \|P_pool−P_ref\| activo |
| Exploit del pool objetivo | Re-sim con reserves drenadas | El bundle revierte (atomicidad); §18.7 ya había excluido el patrón |
| Builder censor / latency spike | Failover de relay | Fail-closed sin fallback a mempool público; alarm se dispara |

**Sensibilidad del EV al gas**: `EV(g) = π_gross − fees − gas_used·g` es afín decreciente
con pendiente `−gas_used`. Break-even: `g* = (π_gross − fees)/gas_used`. Margen de
seguridad: operar solo si `g_actual ≤ g*/2`. Consecuencia: las rutas de muchos hops tienen
beta-gas alta y mueren primero en el spike — la duración del universo bajo stress es una
métrica de portfolio, no solo por-ruta.

## 18.6 RIESGO DE ORÁCULO

**Spot vs TWAP**: leer el precio spot del mismo pool que comercias es autoreferencial y
manipulable en un bloque (el atacante mueve reserves, tu bot marca/decide al precio falso,
y el atacante deshace el pump con un swap inverso en el mismo bloque). Uniswap V3 TWAP vía `observe(uint32[] secondsAgos)`
(devuelve `tickCumulatives`; tick promedio = ΔtickCumulatives/T) es manipulable solo
sosteniendo el desplazamiento durante la ventana T, a un costo ≈ impacto + fees + IL sobre
T — caro pero no imposible en pools delgados.

Checks obligatorios:

- **Chainlink** `latestRoundData()`: validar staleness (`updatedAt` dentro del heartbeat
  del feed) y `answer` dentro de bandas de deviation. Feed-specific: no hardcodear un
  heartbeat global.
- **Consenso multi-oráculo**: mediana de N fuentes (Chainlink + TWAP V3 + referencia CEX
  firmada); descartar la fuente con `|x − mediana| > k·MAD` (outlier), no promediarla.
- **Circuit breaker por divergencia**: si `max pairwise divergence` supera umbral (stables
  0.5-1%, volátiles 3-5%) durante 2 lecturas consecutivas (1 sola para stables), pausar
  dependencia de precios y re-medir. Nunca permitir que una única fuente sea
  decision-critical.
- **L2**: chequear el Sequencer Uptime Feed antes de operar en Arbitrum/Optimism (post-gap
  de downtime los feeds vienen stale).
- **Regla dura**: nunca derivar `minOut` del estado del pool que la propia ruta toca; usar
  referencia independiente (§18.3 de nuevo: el sizing hereda el precio del breaker, no del
  pool).

## 18.7 RIESGO DE CONTRATO: SCREENING PRE-TRADE

Heurísticas de screening (la checklist profunda vive en `arbx-token-safety-screen`; esto
es la capa de riesgo):

| Señal | Detección | Acción |
|---|---|---|
| Proxy upgradeable | `eth_getStorageAt(pool, EIP1967_IMPL, "latest") != 0` con slot canónico `0x360894…382bbc` | Bajar cap o excluir; auditar admin |
| Admin del proxy es EOA | Slot EIP-1967 admin `0xb53127…b5d6103` resuelve a address sin code | Excluir: upgrade unilateral = rug vector |
| Pausable | Selector `pause()` (`0x8456cb59`) presente / evento `Paused` en histórico | Excluir o cap mínimo (inventario congelable) |
| Blacklistable | `blacklist(address)` / `isBlacklisted(address)` / hooks `beforeTokenTransfer` | Cap por token: inventario puede congelarse post-compra |
| Fee-on-transfer / deflacionario | Diff de balances en revm simulando el transfer REAL | Excluir; nunca confiar en metadata/label |
| Fee dinámica | Pools con fee por hook (V4/Algebra-style) o `setFee` owner-callable | Re-leer fee en el bloque del bundle; fee máx por ruta |
| Verificación joven | Explorer API: fecha de verificación vs fecha de creación; re-verificación reciente de contrato viejo = fuente cambiada | Cuarentena del par |
| TVL/reservas bajo mínimo | `getReserves()` (V2) / `liquidity()` + `slot0()` (V3) | Excluir (manipulación barata + slippage estructural) |

El screening corre pre-trade en el hot path (cacheado por pool, invalidado por edad y por
eventos de upgrade) y de nuevo post-deploy del plan como check de barrera.

## 18.8 RIESGO OPERATIVO: KEY COMPROMISE Y FAT-FINGER

**Runbook de key compromise** (orden estricto; el kill-switch es el del núcleo §6.3):

1. **Kill-switch** vía API/file/edge (target <10ms): cesa TODO nuevo despacho.
2. **Contención on-chain**: `emergencyPause()` del executor (núcleo §2.1);
   `revokeRole(EXECUTOR_ROLE, clave_comprometida)`; zero allowances (`approve(spender, 0)`)
   desde la clave comprometida hacia todos los routers/pools aprobados.
3. **Barrido/rotación**: mover balances residuales a dirección de seguridad; generar
   keypair nueva; re-grantar roles; actualizar `SIM_SIGNER_ADDRESS` en `.env` (crash on
   boot si falta, RULE 02); redeploy de config.
4. **Forensics**: replay desde PG/Redis (núcleo §9.2) para acotar blast radius y ventana.
5. **Post-mortem** escrito (§37: todo incidente cierra con revert + gate nuevo).

Diseño que blinda el runbook: clave **guardian** separada cuyo único privilegio es pausar
(separation of duties — puede contener sin poder operar); txs de revocación pre-firmadas y
almacenadas offline; gas pre-cargado en la guardian para ganar la carrera de revocación.
**Drills trimestrales** en testnet/paper con cronómetro: medir kill→revoke→rotate.

**Fat-finger guards** (fail-closed en el pre-execute checklist):

- `max_amount_in` absoluto por trade y por bloque; además bound estadístico:
  `amount_in ≤ k × mediana histórica` de tamaños ejecutados por esa ruta (unidades
  normalizadas por token, USDC-6 vs WETH-18).
- `max_hops` por ruta; allowlist de pools/routers (núcleo §1.2) — deny por defecto.
- Sanity de magnitud en wei: `0 < amount_in < 2^128` y coherencia de decimales en ingestión.
- **Misconfig**: validación de config en boot con `deny_unknown_fields` (serde) y fail-fast
  (RULE 00); en cada deploy, imprimir y alertar el diff de config; la config canary vive
  primero en paper.

## 18.9 BACKTESTING HONESTO

Cuatro sesgos que matan backtests de arbitraje:

- **Lookahead**: usar en la decisión datos que solo existen después (reserves post-arb del
  bloque N para sizar el trade del bloque N; precio de cierre del bloque). Guard: replay en
  Anvil fork pinneado (`--fork-url … --fork-block-number N`) alimentando SOLO estado/mempool
  ≤ N. Toda feature del modelo debe llevar timestamp de disponibilidad.
- **Survivorship**: analizar solo pools/tokens que aún viven (los explotados/rugueados se
  cayeron del universo) infla el edge. Guard: universos point-in-time snapshotteados; los
  pools muertos permanecen en la historia con su fecha de muerte.
- **Overfitting**: walk-forward (fit en [t0,t1], trade [t1,t1+h], rodar; nunca re-fit
  dentro de la ventana evaluada); holdout out-of-sample intacto hasta la decisión final;
  plateau de parámetros: perturbar ±20% cada parámetro y exigir degradación pequeña (un
  pico afilado es ruido celebrado). Grados de libertad << número de trades.
- **Gap sim-vs-real**: descomponer y medir cada componente en shadow, nunca discutirlo:

```
PnL_real ≈ PnL_sim − Δlatencia − Δcompetencia − Δinclusión − Δgas
  Δlatencia    : histograma t_detect→t_submit (p50/p99) vs vida media del edge
  Δcompetencia : win-rate en bloques disputados (win-matrix §18.2)
  Δinclusión   : inclusion rate por bundle (stats del relay)
  Δgas         : ratio gas_estimado/gas_real por ejecución
```

**Por qué paper/shadow primero** (defer `arbx-paper-trade-first` y
`arbx-simulation-mandatory`): los insumos de TODO este documento — `p`, `g`, `ℓ`, `P_incl`,
la distribución de pérdidas para EVT, los q de la cota de ruina — solo existen medidos con
un ledger real de intents; sin shadow son suposiciones y Kelly sobre suposiciones es
overbet garantizado. Precedente interno de este repo: campañas de ~1M de simulaciones con
0 candidatos viables (certificación HG 2026-09-06) — el shadow honesto y vacío es
información; el backtest optimista es la fuente de la ruina de §18.4.

## GOBERNANZA

Este conocimiento está subordinado a los gates `arbx-*` (`arbx-paper-trade-first`,
`arbx-simulation-mandatory`, `arbx-risk-limits-enforcement`, `arbx-pre-execute-checklist`)
y a CLAUDE.md §34: LIVE_MAINNET es gated, el terminus de ejecución (`relays-client`) es
default-deny. Ninguna fórmula de sizing, stop o Kelly aquí autoriza flip a live ni
broadcast con capital real; los vectores ofensivos citados son exclusivamente señales a
detectar y mitigar (frontera ética: `arbx-mev-ethics-gate`).
