# LEDGER-IMPL-01 — El ledger único: `flash_fee_usd`/`bribe_usd` no pueden ser 0 por omisión, y el UI lee `simulated_net`

**Tarea:** t195 · **Base:** `origin/main` = `4902a47c16ef21b7683e5bf405ab49cd1c525b77` · **Rama:** `docs/ledger-impl-01` · **Ámbito:** `docs/ledger-impl/`
**Runtime medido:** `77b42b3d` · **Estado:** PAPER. Cero mainnet, cero firmas, cero broadcast.

---

## 0. ALCANCE, DICHO ANTES DE TODO

**`backend/`, `shared-rs/` y `frontend/` están los TRES fuera de alcance ⇒ EL PATCH DE CÓDIGO NO PUEDE ATERRIZAR.** Lo que se entrega es: **el locus exacto (path + línea + blob)**, **el patch especificado línea a línea**, y **el falsificador CORRIDO como medición**. **Los tres archivos que habría que tocar están nombrados para que la sesión que tenga el scope los toque sin re-descubrir nada.**

**Y una corrección al contrato que la medición impone:** el defecto **NO está donde el contrato dice**. Está a un salto de ahí, y el salto es lo que hace que el patch sea distinto. §2.

---

## 1. EL FALSIFICADOR, CORRIDO — Y SU RESULTADO NO ES «ROJO»: ES **NO EVALUABLE**

**El test, tal como el OPERADOR lo escribió: `net_spine == net_sim ± ε` sobre la fila testigo `949a670e`.**

### 1.1 La fila testigo, leída

```
id = 949a670e-4d19-4a08-93c7-c336e56d6a9e
net_spine    = 0.049404645914     ← EXACTO al 0,049404645914378764 de #915
gross        = 0.674638495914     ← EXACTO al 0,6746384959143795  de #915
target_delta = -49.950595354086   ← EXACTO al −49,95059535408562  de #915
total_cost   = 0.625233850000
gas          = 0.61523385
flash_fee_usd = 0.0
bribe_usd     = 0.0
computation_status = "computed"
rejection_reason   = gas_floor_breach:own_capital
pair_symbol        = 236eb8(6-hop cycle)
not_computed_reasons = {"dex_fees_usd":"included_in_amount_out_post_fee",
                        "slippage_usd":"priced_by_amm_curve",
                        "amount_out_wei":"cycle_output_not_exposed_by_kernel",
                        "simulation_block":"revm_simulation_is_sim_ctl_scope"}
```

**Los tres valores de #915 reproducidos: `net`, `gross` y `target_delta` coinciden hasta el duodécimo decimal. El instrumento está calibrado.**

### 1.2 El test, corrido

```
M3 = n=1 | con_net_sim=0 | iguales=0
M2_sim  = (simulated_profit_usd VACÍO) | gas_estimate_wei VACÍO | slippage_pct VACÍO
          | revert_risk_pct = 100.0000 | passed = f
          | reverted: TransferHelper: TRANSFER_FROM_FAILED, data: Some(String("0x08c379a0…"))
```

**⇒ `net_sim` NO EXISTE. El test no puede pasar NI fallar: `n=1`, `con_net_sim=0`.**

**Y no es un accidente de esa fila — es de la tabla entera:**

```
M4 = 0 | 723097 | 723097 | 2026-10-08 20:50:08.956402+00
     con_valor | nulos | total | INSTANTE
```

**⇒ `0` de `723.097` filas tienen `simulated_profit_usd`.** (#915 midió `0` de `719.987`; la tabla crece y el cero sigue siendo `0`. **Cada cifra lleva su instante.**)

**⇒ VEREDICTO DEL FALSIFICADOR: `NO EVALUABLE`, y eso ES el resultado — más fuerte que rojo.** **Rojo significaría «los dos números existen y difieren». Aquí UNO DE LOS DOS LADOS NO EXISTE.** **Y la sim SABÍA que la fila no servía: `revert_risk_pct = 100.0000`, `passed = f`. Lo que no hizo fue persistir ninguna cifra.**

**⇒ Y el contrato lo anticipó: *«si el test spine==sim exige un término que hoy no se persiste, ESO ES PARTE DEL PATCH.»* Confirmado: es la primera pieza del patch. §4.**

---

## 2. EL `0` POR OMISIÓN — LOCALIZADO, Y **NO ESTÁ DONDE EL CONTRATO DICE**

### 2.1 Lo que el contrato afirma, y lo que la medición dice

> *«Kernel: `flash_fee_usd: 0` · `bribe_usd: 0` ← 0 POR OMISIÓN, que es lo que pinta +$0.05»*

**`backend/searcher-rs/src/economics.rs`, blob `173e9227d308867bd561fba9c44a06e7d395a85c`, líneas `157-192` — el sitio que el contrato señala:**

```rust
let (amount_in_usd, gas_usd, flash_fee_usd, bribe_usd, other_costs_usd) = match econ {
    Some(e) if amount_known => (
        Some(e.start_amount_usd),
        Some(e.gas_usd),
        Some(e.flash_fee_usd),          //  ← NO es 0: viene del economics calculado
        Some(e.builder_tip_usd),        // :162  // 0.0 is its TRUE computed value today
        Some(e.other_cost_usd),
    ),
    Some(e) => { /* :167-179  principal_not_exposed_by_rejecting_path → los 5 a None + not_computed */ }
    None     => { /* :180-191  net_economics_absent                   → los 5 a None + not_computed */ }
};
```

**⇒ `economics.rs` NO escribe `0`. Toma `Some(e.flash_fee_usd)` del `RouteNetEconomics`, y en los dos caminos sin datos pone `None` + `not_computed` con su razón — que ES la regla R8 que el contrato exige.** **Y `:162` es literalmente el «cero declarado que se conserva» que el contrato nombra, con su comentario.**

**Y `total_cost_usd` (`:204-207`) suma con `.flatten().reduce(...)`: los cuatro términos van juntos (`Some` los cuatro, o `None` los cuatro) ⇒ NO hay suma parcial silenciosa.** **Se declara porque era la trampa plausible y la medición la descarta.**

### 2.2 Dónde SÍ está el `0`, y la prueba estadística

**`backend/searcher-rs/src/net_bps_ranking.rs`, blob `f817894d10a63a14b74698229ab1a516121981cb`:**

```rust
:61  /// Explicit R8 marker: no sizing components available (engine-rejected or
:62  /// hand-built). Every derived bps metric on it is `None` — an entry built
:63  /// from this ranks LAST, never mid-table on fabricated zeros.
:64  pub fn not_computable() -> Self {
:65      Self { start_amount_usd: 0.0, gross_over_input_usd: 0.0, gas_usd: 0.0,
:69          flash_fee_usd: 0.0,        //  ← EL 0
:70          builder_tip_usd: 0.0, other_cost_usd: 0.0, }
:73  }
:75  /// ... a flash-backed borrow pays the selected mode's fee; own capital pays none.
:89      let flash_fee_usd = borrow_usd * mode.fee_bps() / 10_000.0;   // ← el cálculo CORRECTO ya existe
```

**⇒ El `:69` es un CENTINELA R8 DELIBERADO, con su razón escrita en `:61-63`, y su contrato dice que sus métricas derivadas son `None`.** **⇒ NO es «0 por omisión»: es «0 como marcador de no computable», y el contrato prohíbe exactamente esa confusión — pero del lado contrario al que supone.**
**Y `:376 assert_eq!(own.flash_fee_usd, 0.0)` pinea el caso `own_capital`, donde el flash fee es CERO CORRECTO porque no hay préstamo (`:75-80`).**

**⇒ LA PRUEBA QUE CIERRA, y sale de la tabla viva, no del código:**

```
M8 (30 min)  flash_null = 34142 | flash_cero = 92208 | flash_POSITIVO = 0
             bribe_null = 34142 | bribe_cero = 92208 | bribe_POSITIVO = 0
             total = 126350
M6 (30 min)  filas = 126358 | net_positivo = 5 | net_no_positivo = 92241 | net_nulo = 34112
             mejor_net = 0.264471 | bribe_cero = 92217 | flash_nulo = 34141 | flash_cero = 92217
```

**⇒ EN `126.350` FILAS VIVAS, `flash_fee_usd` ES POSITIVO EN `0` Y `bribe_usd` ES POSITIVO EN `0`.** **Un cero en el 100 % de una población no es un valor computado: es un CAMPO QUE NADIE RELLENA.** **Si fuera computado habría distribución.**

**★ Y la fila testigo lo demuestra con su propia aritmética, sin invocar el censo:**

```
total_cost = 0.625233850000
gas        = 0.615233850000
total_cost − gas = 0.010000000000    ← EXACTO
```

**⇒ `total_cost_usd = gas_usd + other_costs_usd(0,01)`, con `flash_fee_usd = 0.0` y `bribe_usd = 0.0`, y `computation_status = "computed"`.** **Y `not_computed_reasons` enumera CUATRO términos (`dex_fees_usd`, `slippage_usd`, `amount_out_wei`, `simulation_block`) y NO menciona `flash_fee_usd` ni `bribe_usd`.**

**⇒ AHÍ ESTÁ LA VIOLACIÓN DE LA REGLA, CON LOS DATOS DE LA PROPIA FILA DEL OPERADOR: un lector no puede distinguir «flash fee computado y vale 0» de «flash fee no computado». El `0.0` con `"computed"` y sin razón declarada ES el 0 por omisión.**
**⇒ Y el defecto NO es que `economics.rs` los escriba en 0 — es que `flash_fee_usd` LLEGA como `0.0` desde `RouteNetEconomics` sin que ninguna capa lo declare ausente, y `economics.rs` lo publica tal cual como si fuera computado.**

**⇒ Y aquí está la refutación de la justificación escrita del kernel, que el contrato pide que desaparezca o cambie:** `not_computed_reasons` declara `dex_fees_usd: included_in_amount_out_post_fee` y `slippage_usd: priced_by_amm_curve`. **Esos dos SÍ están declarados.** `flash_fee_usd` y `bribe_usd`, **no** — y son los que valen 0.

---

## 3. EL LP POR POOL — Y SON **TRES** SITIOS, NO DOS

**El contrato localiza el proxy en `dex_engine.rs:439-440`. Medido, son tres:**

```
backend/searcher-rs/src/engines/dex_engine.rs   blob f6d8c3787629e01912405e3c705f9005eb22128f
:439   let fee_a = pool_a.fee_bps.unwrap_or(30);
:440   let fee_b = pool_b.fee_bps.unwrap_or(30);
:896   let fee   = pool.fee_bps.unwrap_or(30);      ← TERCER SITIO, no listado en el contrato
:1276  // class of error that produced the `fee_tier` ×100 episode. The literal is gone:
```

**⇒ `pool.fee_bps` es `Option`: el `unwrap_or(30)` se aplica EXACTAMENTE cuando el fee REAL NO SE CONOCE.** **⇒ El defecto no es el número 30: es que un fee DESCONOCIDO se convierte en 30 bps en vez de declararse NO COMPUTADO — la misma clase que el `18` de `decimals` que t187 midió, y que el ×100 de `fee_tier`.**

**★ Y `:1276` merece leerse entero en la sesión que toque el código: alguien ya dejó escrito que el literal desapareció *«the class of error that produced the `fee_tier` ×100 episode»* — que es exactamente lo que t191 midió en `3 de 32` pools V3 (`tabla=100` vs `fee()=10000`).** **No se transcribe más: `backend/` está fuera de alcance y no se re-deriva (eso es t197).**

**La regla que se especifica: `lp_fee[pool]` con el fee real — V2 `30 bps` · V3 `100/500/3000/10000` millonésimas — y sin fee real, la arista es `NO COMPUTADO`, nunca `unwrap_or(30)`.**

---

## 4. G-SIM-1: **SIGUE ABIERTO**, Y LA RAZÓN ES DE CAPA

### 4.1 Hay TRES capas, no dos — y el contrato llama «sim» a la que no es

| # | Capa | Fichero (blob) | Produce | Quién lo lee |
|---|---|---|---|---|
| **1** | **Kernel** (Rust) | `searcher-rs/src/economics.rs` (`173e9227…`) + `size_optimizer.rs` (`a1411d82…`) | `economics.net_profit_usd` (JSONB) | **la card, PRIMERO** |
| **2** | **SIM-TS** (TypeScript) | `api-server/src/simulation/computeSimulatedNet.ts` (**`b8ded1f91cfc6c3c1aa7d3601259fd7a71b9a22d`**) | `simulated_net_profit_usd` | **la card, como FALLBACK** |
| **3** | **sim-ctl** (Rust/revm) | `sim-ctl/src` | `simulations.simulated_profit_usd` | **NADIE: `0` de `723.097`** |

**⇒ El «ledger sim» que el contrato describe (`LP_proxy30bps − slip − flash − relay`) es la CAPA 2.** **Y `git grep -n -E "lp_fee|slippage_usd|flash_fee" -- backend/sim-ctl/src` devuelve `exit=1`: CERO coincidencias ⇒ LA CAPA 3 NO CALCULA ECONOMÍA.** **⇒ La refutación de #915 queda reproducida por mi propia corrida.**

### 4.2 De dónde lee la card AHORA, con path + línea + blob

```
frontend/components/OpportunityTradeCard.tsx   blob cd70397750a645572466e68cd9b2273f461948e2
:289   * renders `net_expected_profit_usd ?? simulated_net_profit_usd`
:301        : opp.simulated_net_profit_usd != null
:302          ? { usd: opp.simulated_net_profit_usd, source: "simulated" }
:940-952   opp.economics.net_profit_usd  →  "logrado {usd(...)}"

frontend/app/page.tsx   blob 59413d1a0481741ce33fffd619c47807459b750e
:57    .map((o) => o.net_expected_profit_usd ?? o.simulated_net_profit_usd ?? null)
```

**⇒ LA CARD YA LEE `simulated_net_profit_usd` — PERO COMO FALLBACK, DESPUÉS DEL KERNEL.** **La precedencia `net_expected_profit_usd ?? simulated_net_profit_usd` es exactamente lo contrario de lo que G-SIM-1 pide.** **Y la precedencia está DUPLICADA: `OpportunityTradeCard.tsx:301` y `app/page.tsx:57` — hay que cambiarla en los DOS sitios o la pantalla seguirá discrepando entre la lista y el detalle.**

**Y el productor de ese campo, medido:** `backend/api-server/src/routes/opportunities-live.ts` (**blob `750c313deae43d237ce2d4d9de9a6a4b95654747`**), **`:977 const simulated_net_profit_usd = sim?.forward?.net_usd ?? null;`** ⇒ **es el `forward` del SIM-TS (capa 2), no una simulación revm.**

### 4.3 Qué mostraría la MISMA fila DESPUÉS — y por qué **no se puede cerrar desde aquí**

**Con el patch I (invertir la precedencia), la fila testigo mostraría `simulated_net_profit_usd`, que hoy vale `null` ⇒ la card caería al tercer término o a vacío.** **⇒ NO CIERRA G-SIM-1.** **G-SIM-1 exige que sim-ctl simule la misma ruta y el UI muestre ESE neto — y la capa 3 hoy:**
1. **no calcula economía** (`git grep` = 0 coincidencias),
2. **no persiste ninguna cifra** (`0` de `723.097`),
3. **y no puede persistirla donde vive la economía, porque `simulations` NO tiene columna `economics`** (defecto **#15**, corroborado por mi propio `information_schema` en t187: `simulations` sin `economics`; los económicos viven en el JSONB de `opportunities`).

**⇒ G-SIM-1 SIGUE ABIERTO, y el patch es más grande que «invertir la precedencia»: exige (i) que sim-ctl emita economía, (ii) un destino donde persistirla —columna nueva o JSONB—, y (iii) recién entonces la precedencia invertida en los DOS sitios del frontend. SE DICE, no se maquilla.**

---

## 5. EL EFECTO SOBRE LA PANTALLA VIVA — LÍNEA BASE, CON SU INSTANTE

**Instante de la medición: `2026-10-08 20:50:08Z – 20:50:09Z`.** Ventana: 30 minutos.

```
M6  filas = 126358 | net_positivo = 5 | net_no_positivo = 92241 | net_nulo = 34112
    mejor_net = +0.264471
    flash_cero = 92217 | bribe_cero = 92217 | flash_nulo = 34141
M8  flash_positivo = 0 | bribe_positivo = 0        (de 126350)
```

**⇒ `5` de `126.358` filas tienen `net > 0`. El mejor net vivo es `+$0,264471`.**

### 5.1 La predicción del operador, contrastada con aritmética

> *«con ledger completo, NINGUNA fila viva pasa; el 6-hop mejor es el primer candidato que A+B dejarían de emitir»*

**Con `flashloan_fee_pct = 0.0009` (medido en config), el flash fee solo borra el mejor net vivo (`+$0,264471`) para cualquier notional por encima de `$293,86`** (`0,264471 / 0,0009 = 293,857`). **Por debajo de ese notional, hace falta además el LP por pool y el relay para decidir.**

**⇒ La predicción se cumple para todo notional `> $293,86`, y ese umbral es aritmética, no opinión.**
**⇒ LO QUE NO MEDÍ, y se declara: el `amount_in_usd` POR FILA de esas `5`. Sin él no se puede decir cuántas caen por el umbral. NO COMPUTADO con su razón** — y no se sustituye por el notional de la fila testigo, que es una fila distinta.

### 5.2 La fila del 6-hop, lo que la card muestra HOY

```
M7 (19:45–19:47)  236eb8(6-hop cycle)
net_card = 0.050491 | gross = 0.675725 | coste_kernel = 0.625234 | flash = 0.0 | bribe = 0.0
n_sims = 1 | reverted: TransferHelper: TRANSFER_FROM_FAILED
```

**⇒ `+$0,050491` es lo que pinta la card, con `flash = 0.0` y `bribe = 0.0` en el ledger que la produce. Los `+$0.05` del contrato quedan reproducidos hasta el quinto decimal.**

---

## 6. LO QUE NO SE TOCA

- **`trading_config` chain 1, leído: `1000.00 | 50.0000 | 50.0000`.** **INTACTOS, y la condición del OPERADOR se respeta: siguen intocados *hasta que el test `spine==sim` sea verdad* — y el test es `NO EVALUABLE` (§1), luego la condición NO se cumplió y no se toca nada.**
- **NO se bajó `min_ev_usd` ni el target a `$0.05`.** **NO se subió `max_hops`.** **No se añadieron long-tail ni cartuchos. No se encendió Live.**
- **CERO cambios a producción, umbrales o configuración.** CERO mainnet, firmas o broadcast. Paper.

---

## 7. INSTRUMENTO Y CONTROLES

| Control | Resultado |
|---|---|
| Canal `SELECT 1` | `1`, **exit=0** |
| Negativo `SELECT esto_no_existe` | **exit=1** + `ERROR: column "esto_no_existe" does not exist` — **sin tubería** |
| **Positivo del `LIKE`** | `fail_reason LIKE 'reverted:%'` → **`5910`** @ **`2026-10-08 20:50:09.111195+00`** — serie propia `4007 → 5322 → 5498 → 5607 → 5910`; **el `4007` de t184 NO se reutiliza** |
| Frontera pre/post | `searcher` `2026-10-08T14:16:07.102088235Z` · `sim-ctl` `...1195963Z` — **sin cambio** |
| `simulations` sin `economics` | **#15** corroborado en t187 por `information_schema` |
| `fail_reason` prefijado | `LIKE 'prefijo%'`; **nunca `IN (...)`** |
| `revert_risk_pct` | **el campo de rastro que SÍ tiene la fila testigo (`100.0000`)**; no `raw_trace` |
| `:9090` externo | **`000`, exit=7** — defecto #9 |
| La tabla es viva | **cada cifra lleva su instante** (§1.2, §5, §7) |
| Ningún control pipeado a `head` | los tres, sin tubería |

---

## 8. AJUSTES PROPIOS, CAZADOS Y DECLARADOS

1. **Path equivocado**: busqué `backend/api-server/src/**services**/computeSimulatedNet.ts` y **`git rev-parse` devolvió `exit=128`**. La ruta real es **`backend/api-server/src/simulation/computeSimulatedNet.ts`**. **Corregido.**
2. **Quoting roto en el canal**: mi primera corrida del test murió con **`ERROR: trailing junk after numeric literal at or near "949a670e"`** porque el escape de comillas de `LIKE '949a670e%'` no sobrevivió PowerShell→ssh→bash. **Re-corrido por el motor Python (sin comillas de shell) y dio el resultado de §1.** El fallo era del canal, no del SQL.
3. **Hipótesis mía descartada por medición, y se declara:** planteé que `total_cost_usd` podría quedar **sumado a medias** porque `reduce` sobre `Option` descarta los `None`. **Medido en `economics.rs:157-192`: los cuatro términos van juntos (`Some` los cuatro o `None` los cuatro) ⇒ NO hay suma parcial silenciosa.** **Se declara porque era la trampa plausible y la medición la mata.**

---

## 9. EL PATCH ESPECIFICADO — PARA QUIEN TENGA EL SCOPE

**No se aplica aquí (`backend/` y `frontend/` fuera de alcance). Se especifica con path + línea + blob:**

| # | Fichero (blob) | Línea | Qué cambia |
|---|---|---|---|
| **A1** | `backend/searcher-rs/src/engines/dex_engine.rs` (`f6d8c3787629e01912405e3c705f9005eb22128f`) | `:439`, `:440`, **`:896`** | **`unwrap_or(30)` → fee real del pool; sin fee ⇒ `NO COMPUTADO`, nunca 30.** |
| **A2** | `backend/searcher-rs/src/net_bps_ranking.rs` (`f817894d10a63a14b74698229ab1a516121981cb`) | `:64-73` | **`not_computable()` conserva los `0.0` (es su contrato declarado) pero sus derivadas ya son `None`; NADA que cambiar si se respeta `:61-63`.** |
| **A3** | `backend/searcher-rs/src/economics.rs` (`173e9227d308867bd561fba9c44a06e7d395a85c`) | `:157-192` | **AÑADIR `flash_fee_usd` y `bribe_usd` a `not_computed` cuando llegan `0.0` sin razón que los respalde** — hoy declara `dex_fees_usd` y `slippage_usd` y **no** los dos que valen 0. |
| **A4** | `backend/api-server/src/simulation/computeSimulatedNet.ts` (`b8ded1f91cfc6c3c1aa7d3601259fd7a71b9a22d`) | — | **el segundo ledger; su unificación con A1-A3 es la parte TS del patch.** |
| **I1** | `frontend/components/OpportunityTradeCard.tsx` (`cd70397750a645572466e68cd9b2273f461948e2`) | `:289`, `:301-302` | **invertir la precedencia: `simulated_net_profit_usd ?? net_expected_profit_usd`.** |
| **I2** | `frontend/app/page.tsx` (`59413d1a0481741ce33fffd619c47807459b750e`) | `:57` | **la MISMA inversión, o la lista y el detalle discreparán.** |
| **I3** | `simulations` (esquema) | — | **sin columna `economics` (#15): persistir el net de sim-ctl exige columna nueva o JSONB. Sin esto, G-SIM-1 no cierra por más que se invierta la precedencia.** |

**Rama + PR en DRAFT, SIN MERGE** (§10), porque el scope no incluye `backend/` ni `frontend/`.

---

## 10. PUBLICACIÓN

**Rama `docs/ledger-impl-01`, PR en DRAFT, sin merge.** **El patch de código NO aterriza: `backend/` y `frontend/` fuera de alcance.** `git add` normal · integridad por `git hash-object` · **nunca `Out-File`** · `mergeStateStatus` como esté.

---

## 11. HUECOS ABIERTOS — DECLARADOS

1. **El test `spine==sim` es `NO EVALUABLE`** (§1). **Persistir `net_sim` es la primera pieza del patch** (A/I3).
2. **G-SIM-1 SIGUE ABIERTO** (§4.3): tres capas, y la que el contrato llama «sim» no es la que G-SIM-1 exige.
3. **El `amount_in_usd` por fila de las `5` vivas NO se midió** ⇒ no se dice cuántas caen bajo el umbral de `$293,86`. **NO COMPUTADO con su razón** (§5.1).
4. **NO SE DECLARA MEJORA DE PnL.** Con el ledger completo el net baja (el flash fee solo resta): **es una mejora de INSTRUMENTO, y la predicción del operador («ninguna fila viva pasa») se cumple para todo notional `> $293,86`.**
5. **`decimals` y `fee_tier` NO se tocan aquí**: su locus es t197 y t191 respectivamente. **No se re-derivan.**
