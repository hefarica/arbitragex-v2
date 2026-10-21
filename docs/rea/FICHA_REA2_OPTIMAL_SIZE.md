# FICHA REA-2 — OPTIMAL-SIZE-01: tamaño óptimo en forma cerrada (dos pools CPMM, fee 0.3 %)

Fuente fijada: `jacksonConrad/better-simple-arbitrage@f8b96f3be6306ffc48336903852f6b82d4ca4310`
(rama default `dev`, HEAD, último push 2021-07-09 — el repo nunca se movió de ese sha).
Ancestro citado por el propio fuente ("Fixed & improved key aspects of Flashbots' boilerplate
simple-arbitrage bot", README del repo): `flashbots/simple-arbitrage@03cf1058b5bd8079561e2505efc85f77e357b829`.

## 0. Estado de gate (rea_adopt, corrido 2026-… para este destino)

- Destino declarado: `backend/searcher-rs/src/size_optimizer.rs` (no es ruta protegida; G5 OK).
- Licencia en el sha citado: **ninguna** (`license: null` en GitHub API; sin archivo `LICENSE`
  en el árbol a `f8b96f3`). ⇒ verbatim BLOQUEADO; **números = hechos citables** (FUSILE binding rule 3).
- Allowlist FUSILE (`docs/security/FUSILE_SOURCE_POLICY.md:23-43`): **NO está**. Veredicto
  `rea_adopt`: **BLOQUEADO para código**, `modo_permitido: solo_vectores_numericos`.
- Enmienda requerida (firma del OPERADOR, no del agente):
  `CONFIRMED: allowlist amendment jacksonConrad/better-simple-arbitrage` → sección
  "Approved sources (allowlist)", tier propuesto 2 (arquitectura/executor).
- Esta ficha trabaja íntegramente en modo vectores: deriva y verifica números; no porta código.
- G7 respetado: el fuente contiene `TOKEN_BLACKLIST` con 3 addresses (`src/Arbitrage.ts:9-13`),
  `UNISWAP_LOOKUP_CONTRACT_ADDRESS`, `WETH_ADDRESS` (`src/addresses.ts`) — **ningún valor de
  operador del fuente entra al destino**.

## 1. Segmento leído (cuaderno, sha f8b96f3)

```
segmento: getBestCrossedMarket (sizing óptimo) + getAmountOut/getAmountIn (matemática de swap)
repo@sha: jacksonConrad/better-simple-arbitrage@f8b96f3be6306ffc48336903852f6b82d4ca4310
archivos: src/Arbitrage.ts (315 líneas), src/UniswappyV2EthPair.ts (370), src/utils.ts (23),
          test/Arbitrage.test.ts, test/UniswappyV2EthPair.test.ts
```

### 1.1 Matemática de swap entera (orden exacto de operaciones)

`src/UniswappyV2EthPair.ts:299-304` — fee **sobre el input, antes de dividir**, enteros BigNumber:

```
getAmountOut(reserveIn, reserveOut, amountIn):
  amountInWithFee = amountIn * 997              // :300  ← aquí se resta la fee (×0.997)
  numerator       = amountInWithFee * reserveOut
  denominator     = reserveIn * 1000 + amountInWithFee
  return floor(numerator / denominator)         // :303  ← floor (div de BigNumber trunca)
```

`src/UniswappyV2EthPair.ts:293-297` — inversa con techo:

```
getAmountIn(reserveIn, reserveOut, amountOut):
  numerator   = reserveIn * amountOut * 1000
  denominator = (reserveOut - amountOut) * 997
  return floor(numerator / denominator) + 1     // :296  ← +1 = ceil (cubre el residuo)
```

**Orden que importa**: la fee se aplica multiplicando el input (997/1000) ANTES de la división;
el redondeo es floor en output y ceil(+1) en input. El round-trip in→out→in no es identidad:
recupera 1 wei de más (verificado en V-A3 abajo).

### 1.2 Forma cerrada del sizing (el corazón del port)

`src/Arbitrage.ts:79-94`, verbatim semántico (floats JS en el fuente):

```
kBuy  = buyWETHReserves  * buyTokenReserves      // :79   k_B = w1·t1
kSell = sellWETHReserves * sellTokenReserves     // :80   k_S = w2·t2
gamma = 0.997                                     // :81

numeratorA   = sqrt(kSell) * buyTokenReserves            // :83
numeratorB   = gamma^-1 * (sqrt(kBuy) * sellTokenReserves)
denominator  = sqrt(kBuy) + sqrt(kSell)

deltaAlpha   = (numeratorA - numeratorB) / denominator    // :87   δα* (tokens a mover)

deltaBeta    = gamma^-1 * (kBuy/(buyTokenReserves - deltaAlpha) - buyWETHReserves)   // :90  δβ (WETH in)
deltaBetaPrime = sellWETHReserves - kSell/(sellTokenReserves + gamma*deltaAlpha)     // :92  δβ′ (WETH out)
profit       = deltaBetaPrime - deltaBeta                 // :94
```

Notación: pool compra `(t1, w1)` = (reserva token, reserva WETH) donde se gasta WETH para
comprar token; pool venta `(t2, w2)` donde se vende el token por WETH. γ = 997/1000.

**Guard de dirección**: `src/Arbitrage.ts:99-101` — si `deltaBeta < 0` el par está orientado
al revés (o no hay arb) y se descarta con `continue`.

**Asignación de dirección**: fork `src/Arbitrage.ts:34-35` → `crossedMarket[0]` = BUY,
`[1]` = SELL. El ancestro `flashbots/simple-arbitrage@03cf1058` tenía la asignación
INVERTIDA (`[0]` = sellToMarket, `[1]` = buyFromMarket). El fork la invirtió al reescribir.

**Pre-filtro de población** (pertenece a POBLACION-FILTER-01, se cita por completitud):
`src/Arbitrage.ts:168` — `arbIndex > 1.003**2` con `arbIndex` = cociente de precios
marginales WETH/token entre ambos pools.

### 1.3 Redondeo/reconversión del fork (convención a DESCARTAR)

- Reservas Big→float: `src/utils.ts:5-8` — `(v·10000/10^decimals).toNumber()/10000` ⇒
  **granularidad 1e-4** en unidades decimales. (Fallback `veryBigNumberToDecimal` `:12-15`,
  sin el truco del ×10000.)
- Float→wei al ejecutar: `src/Arbitrage.ts:249-250` —
  `Math.round(volume·10^10)·10^8` ⇒ **granularidad 1e-10 WETH** en el tamaño ejecutado.
- El destino (Rust, u128/U256 wei) no hereda NINGUNA de las dos: computa en entero exacto.

### 1.4 Convenciones descartadas (no se portan)

MongoDB/Token DAO (`src/models/Token.ts`, dependencia del cálculo de decimals — el test del
fork ni siquiera puede correr sin DB), floats 1e-4, blacklist de addresses ajena, Flashbots
provider, `console.table`, su sistema de errores. La semántica que viaja: fórmulas, orden de
fee/rounding, guard de dirección y los números de test.

## 2. Derivación (verificada independientemente, Decimal 60 dígitos + enteros exactos)

### 2.1 Equivalencia con la cuadrática en dx

Parametrizando por `dx` = WETH que entra al pool de compra (fee sobre input, γ=997/1000):

```
dy = t1·γ·dx / (w1 + γ·dx)          [compra: token que sale del pool 1]
dz = w2·γ·dy / (t2 + γ·dy)          [venta: WETH que sale del pool 2]
π(dx) = dz − dx ;  dπ/dx = 0  ⇒  (t2·w1 + γ·x·(t2 + γ·t1))² = γ²·t1·t2·w1·w2
                                    ⇒  dx* = (γ·√(t1·t2·w1·w2) − t2·w1) / (γ·(t2 + γ·t1))
```

La raíz positiva de esa cuadrática y el `deltaBeta` de la forma cerrada del fork
(`src/Arbitrage.ts:87-90`) son **el mismo número** — verificado numéricamente a 1e-40
(script de §4): con `(t1,w1)=(1,1)`, `(t2,w2)=(1,2)` ambos dan `dx* = 0.205911134347446829…`.
π es estrictamente cóncava (d²π/dx² < 0 en todo el dominio): la raíz es el máximo único.

### 2.2 Bordes (todos verificables en el port)

| Borde | Resultado | Fundamento |
|---|---|---|
| `f = 0` (γ=1) | `dx* = (√(t1·t2·w1·w2) − t2·w1)/(t2 + t1)` | límite directo de la cuadrática; setup del vector: (√2−1)/2 = 0.20710678… |
| Pools idénticos (simétricos) | `√(t1t2w1w2) = tw = t2·w1` ⇒ numerador < 0 ⇒ **sin arb** | test del ancestro "do not cross" ( reservas iguales ⇒ profit < 0) |
| `dx → 0` | `π(0)=0`; rentable sii `dπ/dx|₀ > 0` ⇔ `(w2·t1)/(w1·t2) > γ⁻² = 1.00601803…` | el pre-filtro del fork usa `1.003² = 1.006009` (holgura 9.0e-6 menor — NO son la misma constante: 1.003 es la tasa, γ⁻² el multiplicador) |
| `dx → ∞` | `dz → w2·γ·t1/(t2+γ·t1)` (finito, setup: 0.998498 WETH), `π → −∞` | sin patología; máximo único garantizado por concavidad |
| Dominio | `t1−δα > 0` y `t2+γ·δα > 0` exigibles; si `δβ < 0` → par invertido/perdedor (guard `Arbitrage.ts:99-101`) | el port debe canonizar la dirección ANTES de aplicar la fórmula |

## 3. Vectores numéricos (con procedencia)

### V-A — matemática de swap entera (upstream directo, fuerza máxima)

`test/UniswappyV2EthPair.test.ts`, pool `[TOKEN=1e18, WETH=2e18]`. Reproducidos BYTE-EXACTO
por simulación entera independiente (script §4):

| # | Operación | Entrada | Esperado (upstream) | Verificado |
|---|---|---|---|---|
| A1 | `getAmountIn(TOK→WETH)` | amountOut=1e17 | `52789948793749671` | ✅ exacto |
| A2 | `getAmountIn(WETH→TOK)` | amountOut=1e17 | `222890894906943052` | ✅ exacto |
| A3 | `getAmountOut(TOK→WETH, A1)` | — | `1e17 + 1` | ✅ exacto (round-trip +1 wei) |
| A4 | `getAmountOut(WETH→TOK, A2)` | — | `1e17` | ✅ exacto |

Procedencia: `test/UniswappyV2EthPair.test.ts` del sha `f8b96f3` (tests 2 y 3 del archivo).

### V-B — sizing: valores del test heredado, procedencia CERRADA vía el ancestro

`test/Arbitrage.test.ts` del fork espera, para pools buy=`[T=1e18, W=1e18]` y
sell=`[T=1e18, W=2e18]`:

- `volume = 208333333333333333` wei (= ⌊5/24·10¹⁸⌋)
- `profit = 84409320961485006` wei (= `0x012be1d487a428ce`)

**Procedencia verificada**: esos dos valores NO los produce la implementación del fork
(ver hallazgo §3.1). Son la salida del algoritmo de escalera entera del ANCESTRO
`flashbots/simple-arbitrage@03cf1058` (`getBestCrossedMarket`: ladder
`[E/100, E/10, E/6, E/4, E/2, E, 2E, 5E, 10E]` + "meet halfway" `(size+best)/2`, todo en
BigNumber con el `getAmountOut` de §1.1, dirección `[0]`=sell/`[1]`=buy). Simulación entera
exacta del ancestro reproduce AMBOS valores byte-exacto:

```
traza escalera (wei):  0.01e18→9492090719486850 · 0.1e18→65792386612335959
                       0.1666…e18→82115183034666418 · 0.25e18→81833585961897235 (baja ⇒ halfway)
halfway (E/6+E/4)/2 = 5/24·1e18 → 208333333333333333 / profit 84409320961485006  ✅✅
```

El `5/24` es mecánico: halfway entre los peldaños E/6 y E/4 de la escalera.

### V-C — óptimo exacto de la forma cerrada (DERIVADO, corroborado por el fork)

Mismo setup que V-B, aritmética Decimal 60 dígitos; el float del fork (sin el recorte 1e-4
de reservas) coincide en todos los dígitos impresos:

```
dx*  = 0.205911134347446829…  ⇒  ⌊dx*·10¹⁸⌋ = 205911134347446829 wei
dy*  = 0.170326495427210820…
π*   = 0.084417577533827558…
```

Ejecución ENTERA (la que haría el port en u128) del tamaño cerrado `205911134347446829`:

```
profit_u128 = 84417577533827556 wei  ≥  84409320961485006 (escalera ancestro)  ✅ dominancia
```

La dominancia `cerrado ≥ escalera` es la invariante teórica (el óptimo supera a cualquier
heurística) y DEBE afirmarse en el test del port.

### 3.1 Hallazgo (fail-honest): el test del fork no es reproducible por el código del fork

Tres razones mecánicas, cada una verificada por simulación:

1. **Dirección**: el test pasa `[[m0, m1]]` con m0=`[T=1,W=2]` como BUY (`Arbitrage.ts:34`),
   pero el arb rentable de ese setup es comprar en m1 y vender en m0 (precio marginal 1 vs 2).
   Con la orden del test, `δα* < 0` ⇒ `δβ < 0` ⇒ el guard `:99-101` descarta y devuelve
   `undefined` (el test haría `fail("No crossed Market")` si llegara a correr la rama).
2. **Granularidad**: `bigNumberToDecimal` (`utils.ts:5-8`) cuantiza reservas a 1e-4 — no puede
   emitir esperas de 18 decimales.
3. **Dependencia DB**: el cálculo del fork llama `Token.getToken()` (mongoose) para los
   decimals; sin MongoDB el path de test ni siquiera computa.

⇒ Los valores V-B son **hechos del ancestro** (verificados contra su código @03cf1058),
heredados por el test del fork. El port cita la cadena completa:
`flashbots@03cf1058 (valores, escalera entera) → jacksonConrad@f8b96f3 (test heredado +
forma cerrada)`.

## 4. Script de verificación (reproducible)

Artefacto: `docs/rea/verifica_vectores_rea2.py` (determinista, stdlib pura, sin red) —
salida capturada en `docs/rea/verifica_vectores_rea2.log` (10/10 OK, exit 0):

1. `get_amount_out`/`get_amount_in` enteros → V-A ✅ (4/4 byte-exacto).
2. Escalera del ancestro (TEST_VOLUMES + halfway) → V-B ✅ (2/2 byte-exacto + identidad 5/24).
3. Forma cerrada en δα (fork) vs raíz de la cuadrática en dx → igual a 1e-40 ✅.
4. Ejecución entera en `⌊dx*·10¹⁸⌋` → `84417577533827556 ≥ 84409320961485006` ✅.

## 5. Traducción al destino (`backend/searcher-rs/src/size_optimizer.rs`)

Convenciones del destino verificadas en el árbol base (HEAD `f27bdb33`):

- `SizedCandidate.optimal_amount_in: U256` (wei entero) — `size_optimizer.rs:604`;
  `OptimizeOutcome::optimal_amount_in(&self) -> Option<U256>` — `:556`. f64 SOLO para vistas
  USD. R8: `None` = no computado.
- El sizing 2-leg actual es NUMÉRICO: `golden_section_search_2leg` (`size_optimizer.rs:3650`)
  + brackets `geom_probes` log-espaciados. La forma cerrada lo reemplaza para el caso
  V2-V2 puro: O(1) con dos raíces cuadradas enteras en lugar de N evaluaciones de curva,
  mismo resultado exacto (V-C). Para rutas con legs V3/fee heterogéneo, el golden-section
  existente sigue mandando (la cerrada solo aplica a CPMM puro con fee uniforme en input).

Reglas del port:

1. **γ como racional exacto**: `997/1000` en enteros; jamás `0.997_f64`.
2. **Raíces sin float**: `√(t1·t2·w1·w2) = √(t1·w1)·√(t2·w2)` — dos `isqrt` enteros sobre
   `k = t·w` en U256 (k de pools reales ≈ 1e36–1e40 no cabe en u128: computar k y su raíz en
   U256; el resultado de cada raíz SÍ cabe en u128 para reservas on-chain reales).
3. **Un solo punto de redondeo**: floor final de `dx*` (una división entera con resto);
   sin floats intermedios. Ejecutar el trade con el `getAmountOut` entero de §1.1
   (fee 997/1000 sobre input antes de dividir, floor en output) y reportar el profit
   entero resultante — no el π* continuo.
4. **Checked math**: `checked_mul`/`checked_div`/`checked_sub` en cada paso; `Option::None`
   (R8) en: numerador ≤ 0 (sin arb), denominador 0, dominio violado (`t1−δα ≤ 0`,
   `t2+γ·δα ≤ 0`), overflow.
5. **Canonización de dirección** ANTES de la fórmula: elegir buy = pool con menor precio
   marginal WETH/token (`w/t`), o evaluar ambas orientaciones y descartar la de `δβ < 0`
   (equivalente al guard `Arbitrage.ts:99-101`, pero sin perder el candidato).
6. **Test del port** (debe FALLAR si el fuente fue mal leído):
   - V-A completo (4 asserts byte-exacto) contra el `getAmountOut/getAmountIn` entero local.
   - V-B: escalera ancestro → `(208333333333333333, 84409320961485006)`; el port debe
     afirmar `profit_cerrado_entero ≥ 84409320961485006` en ese setup.
   - V-C: `optimal_amount_in == 205911134347446829` para el setup V-B.
   - Bordes: pools idénticos ⇒ None; umbral de rentabilidad `γ⁻²` (un par a razón de precios
     1.0060 < γ⁻² no pasa; 1.0070 sí).
   - Invariante de monotonía/paridad: `profit(dx*±1 wei) ≤ profit(dx*)` en entero.
7. **Cita obligatoria** en código y PR: `jacksonConrad/better-simple-arbitrage@f8b96f3` +
   `flashbots/simple-arbitrage@03cf1058` (valores del vector V-B).
8. **No se toca** (`G5`): `configs/app.toml` paper_mode,
   `backend/relays-client/src/live_exec_policy.rs`, broadcast path de
   `backend/relays-client/src/bundle_builder.rs`. El destino `size_optimizer.rs` está limpio.

## 6. Checklist para `rea_verify` del PR OPTIMAL-SIZE-01

- [ ] P1 cita `repo@sha` doble (fork + ancestro) en código y PR — artefacto: diff.
- [ ] P2 vectores V-A (4) + V-B (2) + V-C (1) como tests con valores literales y procedencia
      en comentario — artefacto: `cargo test` verde con los asserts exactos.
- [ ] P3 paridad numérica: `84417577533827556 ≥ 84409320961485006` afirmado — artefacto: test.
- [ ] P4 bordes §2.2 cubiertos — artefacto: tests de None/dominio.
- [ ] P5 rutas protegidas intactas — artefacto: diff vacío en las 3 rutas.
- [ ] A1 zero-mock: valores del test citados como hechos, sin mocks de pools on-chain.
- [ ] A2 no-hardcode: ningún address/constante de operador del fuente en el diff.
- [ ] A3 license: cero líneas verbatim del fuente; solo números + derivación propia.
- [ ] A4 allowlist: enmienda firmada por el operador ANTES de mergear código (esta ficha
      trabaja en modo solo-vectores y no lo requiere).
