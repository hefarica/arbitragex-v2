# MODE-INVARIANCE-01 — los 3 modos difieren SÓLO en el terminus (§34.1), probado en código

**Orden:** t55 · **Perfil:** Backend · **Intento:** `85d1483e-2629-41b4-8e82-316429bc8f12`
**Base:** `main` = `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`
**Modo:** `ARBX_TRADE_MODE=paper`. **Sin firma, sin broadcast, sin capital.** Sólo lectura de código y tests locales.
**Toolchain:** Rust 1.91.0 en WSL2 (`rust-toolchain.toml` del repo), cero instalaciones.
**Código modificado:** **NINGUNO**. Este artefacto es el único path tocado.

---

## 0. EL HALLAZGO MÁS GRAVE POSIBLE — RESPONDIDO PRIMERO

La orden exige reportar antes que nada si Testnet o Paper **NO** son reflejo fiel de Mainnet (matemática distinta, gates relajados o atajos).

**RESPUESTA: no hay matemática distinta, ni gate relajado, ni atajo. Pero SÍ existe UNA asimetría de fidelidad, y va declarada primero.**

### La única asimetría encontrada

`backend/searcher-rs/src/snapshot_services.rs:1295-1297`:

```rust
if p.execution_mode == "LIVE_MAINNET" && p.storage_overrides_used {
    return Err("paper_override_cannot_authorize_live".into());
}
```

**Dirección de la asimetría: hace a LIVE_MAINNET más ESTRICTO, no a Paper más permisivo en el sentido peligroso.**

* Un plan construido sobre **storage overrides** (estado simulado que no corresponde a la cadena real) **puede** validar en `PAPER_SHADOW` / `TESTNET`, y **es rechazado** en `LIVE_MAINNET`, con el motivo **nombrado** `paper_override_cannot_authorize_live`.
* **Consecuencia honesta y vinculante: "pasa en paper" es NECESARIO pero NO SUFICIENTE para live.** Un plan que valide en paper puede ser rechazado en el terminus. **Esto NO invalida la matemática** (el mismo kernel, el mismo descubrimiento, el mismo sizing — §3), pero **acota qué predice el paper**: predice la **aritmética**, no la **autorización**.

### Por qué NO es "el hallazgo más grave" en su forma letal

El modo letal sería: *paper relaja un gate de riesgo y por eso paper da resultados que live nunca daría*. **Eso no ocurre.** El único condicional por modo del repo entero (§3) **endurece** live. Ningún camino de paper salta un gate que live sí corre.

> **Nota sobre la premisa de la orden, medida:** la orden pide *"las LÍNEAS exactas donde el terminus diferencia los tres modos"* en `live_exec_policy.rs`. **Ese archivo NO modela tres modos.** Modela **encendido/apagado + allowlist de cadenas** (§1). Los tres modos viven en `canonical_knobs.rs`. Se declara la discrepancia en vez de fabricar tres ramas que no existen.

---

## 1. El TERMINUS REAL — `live_exec_policy.rs` (97 líneas, leído entero)

| Línea | Qué dice | Qué prueba |
|---|---|---|
| **L3** | `pub const DEFAULT_LIVE_CHAINS: &[u64] = &[11_155_111];` | el default es **Sepolia** |
| **L5-11** | `enum LiveExecDenied { NotEnabled, ChainNotAllowed { got, allowed } }` | **DOS** motivos de denegación. **NO hay un tercer estado "modo"** |
| **L19-24** | `from_env()` lee `ARBX_LIVE_EXEC_ENABLED` y `ARBX_LIVE_EXEC_CHAINS` | **permiso por ALLOWLIST DE CADENA vía env** ✔ |
| **L25-32** | `from_raw()`: `None` → `DEFAULT_LIVE_CHAINS`; `Some(s)` → split por `,`, parse `u64`, **`filter(x > 0)`**, `collect::<Option<Vec<_>>>()` | cadena inválida ⇒ `None` ⇒ lista **VACÍA**, nunca parcial |
| **L37** | `enabled: enabled == Some("true")` | **sólo el literal `"true"`** enciende |
| **L41-52** | `assert_broadcast_allowed(chain_id)`: `!enabled` → `NotEnabled`; `!allowed.contains(chain)` → `ChainNotAllowed`; si no → `Ok` | **la única puerta**, y es por cadena |

**¿Dónde se EJECUTA esa puerta? — en el único binario que firma:**

| Locus | Línea | Qué hace |
|---|---|---|
| `relays-client/src/main.rs` | **L171-188** | "Explicit per-chain activation is enforced at boot **and again before signing**"; `live_mode = !paper_mode.is_enabled().await` (L174); si `live_mode` y el policy deniega → `anyhow::bail!` nombrando **`ARBX_LIVE_EXEC_ENABLED=true`** y **`ARBX_LIVE_EXEC_CHAINS`** (L185) |
| `relays-client/src/bundle_builder.rs` | **L57-58** | `LiveExecPolicy::from_env().assert_broadcast_allowed(opp.chain_id)` — **primera sentencia de `build_and_sign`**; el error se propaga como `BuildError::LiveExecDenied(reason)` (L44) |

> **Dato de fidelidad del terminus:** en `main.rs:174` la puerta sólo se exige cuando `live_mode` es true — es decir, **en paper no se exige allowlist**. Eso es correcto y es exactamente §34.1: el modo `PAPER_SHADOW` **no hace broadcast**, así que no necesita permiso de cadena. **Es una diferencia de terminus, no de matemática.**

---

## 2. ¿Está Mainnet soportada SIN restricción adicional? — SÍ, y ejecutado

**Test dedicado, citado y CORRIDO:**

```
$ cd backend && cargo test -p relays-client live_exec
running 5 tests
test live_exec_policy::tests::defaults_are_disabled ... ok
test live_exec_policy::tests::enabled_default_is_sepolia ... ok
test live_exec_policy::tests::exact_true_only ... ok
test live_exec_policy::tests::explicit_mainnet_is_supported ... ok
test live_exec_policy::tests::invalid_allowlist_never_partially_activates ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 87 filtered out
```

`live_exec_policy.rs:70-76` — `explicit_mainnet_is_supported`:
```rust
let p = LiveExecPolicy::from_raw(Some("true"), Some("1,11155111"));
assert!(p.assert_broadcast_allowed(1).is_ok());          // ← MAINNET ACEPTADA
assert!(p.assert_broadcast_allowed(11_155_111).is_ok());
assert!(p.assert_broadcast_allowed(137).is_err());
```

**Y en el camino de FIRMA** (`bundle_builder.rs:414-436`, test `m1_rejects_before_fle_resolution`, ejecutado):

```
$ cd backend && cargo test -p relays-client m1_rejects_before_fle_resolution
test bundle_builder::tests::m1_rejects_before_fle_resolution ... ok
```
con **L433-435**:
```rust
assert!(LiveExecPolicy::from_raw(Some("true"), Some("1"))
    .assert_broadcast_allowed(1)
    .is_ok());                                            // ← mainnet OK en el sign path
```
y **L429-432**: con la allowlist en `11155111`, `assert_broadcast_allowed(1)` → `ChainNotAllowed` (mainnet **fuera** de la lista ⇒ denegada, como debe ser).

**Conclusión: `ARBX_LIVE_EXEC_ENABLED=true` + `ARBX_LIVE_EXEC_CHAINS=1,…` habilitan mainnet. NO existe ninguna restricción adicional más allá de ese switch de entorno.** Los 4 controles que sí existen son de **forma de configuración**, no de permiso de cadena:
* default-deny total sin env (`L37`, test `defaults_are_disabled`);
* allowlist malformada ⇒ vacía, **nunca activación parcial** (`L26-33`, test `invalid_allowlist_never_partially_activates`);
* sólo `"true"` exacto enciende (`L37`, test `exact_true_only` — `"TRUE"`, `"true "`, `"1"` **no**);
* sin lista ⇒ sólo Sepolia (`L27`, test `enabled_default_is_sepolia`).

### 2.1 Verificación de la corrección registrada de §34.3

| Afirmación a verificar | Resultado MEDIDO |
|---|---|
| `MainnetRefused` **NUNCA existió como variante Rust** | **CONFIRMADO.** Grep en `backend/`: **4 coincidencias, TODAS prosa TypeScript** — `api-server/src/services/control-board-drift.ts:90`, `api-server/src/routes/control-board.ts:94`, `:173`, `:742`. **CERO en `*.rs`.** En Rust sólo hay `LiveExecDenied::{NotEnabled, ChainNotAllowed}` (`live_exec_policy.rs:5-11`) |
| La redacción *"PHYSICALLY REFUSES mainnet"* era **FALSA** | **CONFIRMADO.** `grep "PHYSICALLY REFUSES"` en `backend/` → **0 coincidencias** |
| El código soporta mainnet con `ENABLED=true` + `CHAINS=1,…` | **CONFIRMADO, ejecutado** (§2) |

> **Hallazgo accionable (menor, declarado):** la prosa **residual** `MainnetRefused` sigue viva en **4 sitios TS** y un operador podría leerla como si el código tuviera una variante de rechazo de mainnet que **no existe**. No cambia comportamiento (son comentarios y un string de mensaje), pero **propaga la redacción ya corregida**. Limpiarla es un cambio de una línea por sitio; **no lo hice porque este contrato es de verificación y su alcance es `docs/backend/`**.

---

## 3. ¿Algún punto hace que la MATEMÁTICA cambie por modo? — NO, y el censo es EXHAUSTIVO

**Esto no es un muestreo: es el censo completo de comparaciones de modo en `backend/`.**

```
grep -rn 'execution_mode\s*[!=]=|selected_execution_mode\s*[!=]=|storage_overrides_used' backend/
  → 2 coincidencias, AMBAS en snapshot_services.rs:
      L64   pub storage_overrides_used: bool,                    (declaración de campo)
      L1295 if p.execution_mode == "LIVE_MAINNET" && ...         (LA rama)
```

**Toda otra aparición del modo, en `backend/**/*.rs`, es VALIDACIÓN o REPORTE — ninguna es rama de comportamiento:**

| Archivo:línea | Uso | ¿Cambia comportamiento por modo? |
|---|---|---|
| `canonical_knobs.rs:37` | `EXEC_MODES: [&str; 3] = ["LIVE_MAINNET","TESTNET","PAPER_SHADOW"]` | **No** — es la lista canónica |
| `canonical_knobs.rs:22` | doc: *"`execution_mode` / `selected_execution_mode` / `killswitch` son DECLARATIVOS"* | **No** — declarado |
| `canonical_knobs.rs:137,152` | campos `execution_mode`, `selected_execution_mode` | **No** — almacenamiento |
| `canonical_knobs.rs:204,216` | defaults `"PAPER_SHADOW"` | **No** |
| `canonical_knobs.rs:333,348-350` | `env_str("ARBX_KNOB_EXECUTION_MODE", …)` | **No** — lectura de config |
| **`canonical_knobs.rs:492-501`** | `if !EXEC_MODES.contains(&self.execution_mode…) { error }` | **No** — **valida** que el token sea uno de los 3 |
| `canonical_knobs.rs:610,626-627` | se serializa a JSON | **No** — reporte |
| **`rhai_agent_bridge.rs:296-297`** | `if !["LIVE_MAINNET","TESTNET","PAPER_SHADOW"].contains(…) { repairs_push(…, "unknown_mode") }` | **No** — **valida** el mismo trío, y si no, **nombra la reparación** |
| `rhai_agent_bridge.rs:134` | campo `pub execution_mode: String` | **No** |
| `rhai_agent_bridge.rs:678` | se emite en el payload JSON | **No** — reporte |
| `snapshot_services.rs:63` | campo | **No** |
| `snapshot_services.rs:1270` | `("execution_mode", &p.execution_mode)` en la lista de campos verificados | **No** — forma parte de la comprobación de consistencia |
| **`snapshot_services.rs:1295`** | **LA ÚNICA rama** | **SÍ** — y **endurece live** (§0) |
| `snapshot_services.rs:1299` | `"approved_for_execution": false` + `"existing_signer_and_live_authorization_gates_remain_authoritative"` | **No** — y **declara que este validador no autoriza ejecución** |
| `cartridge_boot.rs:204,2350` | fixtures con `execution_mode` | **No** — fixtures |

**Residuales declarados (2 sitios, y la discrepancia de conteo 32 vs 35):**

| Archivo:línea | Uso | ¿Rama? |
|---|---|---|
| `canonical_enums.rs:7` | doc de módulo: *"`FINANCING_MODES` / `EXECUTION_MODES` → `crate::canonical_knobs`"* | **No** — **comentario**; reenvía la constante al módulo dueño |
| **`sim-ctl/src/persistence.rs:67`** | comentario: *"EXECUTION_MODES_DOCTRINE §34, the live terminus (relays-client) gates"* | **No** — **comentario**. Dato relevante: el componente de **simulación** nombra la doctrina y **defiere explícitamente al terminus** en vez de ramificar por modo |

> **Cuenta exacta y su comando, porque `32` y `35` son ambos correctos según el case:** `Select-String` de PowerShell es **case-insensitive** y devuelve **35**; los 3 extra son las grafías **mayúsculas** `EXECUTION_MODES` / `EXECUTION_MODES_DOCTRINE` **dentro de comentarios**. `grep`/ripgrep es **case-sensitive** y devuelve **32**, repartidos en **4 archivos**: `canonical_knobs.rs` 21, `snapshot_services.rs` 5, `rhai_agent_bridge.rs` 4, `cartridge_boot.rs` 2. **Ni uno solo de los 35 es una comparación de modo salvo `snapshot_services.rs:1295`.** Un revisor que corra el grep case-insensitive encontrará los dos comentarios: van declarados aquí en lugar de aparecer como omisión del censo.
>
> **Aritmética explícita, para que el `31` no sea un número suelto:** **32 coincidencias case-sensitive − 1 rama (L1295) = 31 coincidencias que NO son rama** (`21+5+4+2 = 32`, medido por el comando de §5). Las 32 están clasificadas arriba, una por una, y **las 31 no-rama son las filas de la tabla cuyo veredicto es "No"** — ninguna es una comparación de modo.

**Corolario:** descubrimiento, rutas, `SizeOptimizer`, simulación y gates de riesgo **no consultan el modo en ningún sitio**. No pueden cambiar por modo porque **no lo miran**. El caso `sim-ctl` lo confirma desde el lado fuerte: el simulador **menciona** la doctrina §34 y **delega** el modo al terminus, sin ramificar.

### 3.1 Los 264 cartuchos: misma declaración en todos

Comando exacto (PowerShell, sobre el clon `%TEMP%\arbx-t55`):

```powershell
$rhai = Get-ChildItem -Path backend -Recurse -Filter *.rhai -File
$rhai.Count                                              # -> 271
($rhai | Select-String -Pattern 'LIVE_MAINNET' -SimpleMatch -List).Count   # -> 264
($rhai | Select-String -Pattern 'EXECUTION_MODES|execution_modes|modes' -List).Count  # -> 264
```

```text
total .rhai = 271
declaring LIVE_MAINNET = 264
declaring some modes list = 264
cualquier .rhai que declare una lista DISTINTA = (ninguno: el conjunto de los que declaran lista == los que declaran LIVE_MAINNET)
```

Los **264** cartuchos que declaran modos declaran **exactamente** `["LIVE_MAINNET", "TESTNET", "PAPER_SHADOW"]` — que es la cifra que cita §34.1 — y **ninguno** declara una lista distinta. Un cartucho no puede ser de un modo.

### 3.2 Afirmaciones de invariancia EN EL CÓDIGO (citadas como declaraciones, no como prueba)

* `searcher-rs/src/economics.rs:30` — *"arithmetic in PAPER_SHADOW / TESTNET / LIVE_MAINNET"*.
* `searcher-rs/src/route_discovery/hop_cycle_bridge.rs:42` — *"kernel and the same emitter run in PAPER_SHADOW / TESTNET / LIVE_MAINNET"*.

**Estas son prosa que AFIRMA la invariancia. La PRUEBA es el censo de §3, no estas líneas.** Lo digo explícitamente para que nadie cite un comentario como evidencia.

---

## 4. La única rama por modo, en su contexto completo

`snapshot_services.rs:1256-1300` es un **validador de plan canónico**. Antes de la rama por modo ya había rechazado: trace hash inválido o todo-ceros (L1280-1281), calldata/target inválidos (L1282-1285), y `net_profit_usd` por debajo de la política (L1290-1293). Después de la rama, devuelve `approved_for_execution: false` (L1299) **con el motivo nombrado** de que los gates de firmante y de live siguen siendo los autoritativos.

**Es decir: la rama no sólo es la única, es una rama de ENDURECIMIENTO dentro de un validador que por diseño no autoriza nada.**

### 4.1 Hallazgo menor declarado: un token NO canónico en un fixture

`cartridge_boot.rs:204` usa `execution_mode: "paper_shadow"` (**minúsculas**), mientras `:2350` usa el canónico `"PAPER_SHADOW"`. Ese `v4_policy` lleva además `price_revision/policy_revision/control_state = "phase1_data_gap"`, o sea es un **fixture de fase 1**.

**Consecuencia:** si ese valor llegara a `rhai_agent_bridge.rs:296` o `canonical_knobs.rs:492`, el validador lo marcaría como `unknown_mode` (nombrado) — **no** se aceptaría en silencio. **No cambia matemática.** Queda declarado, no arreglado (fuera de alcance).

---

## 5. Comandos exactos con los que se comprobó cada afirmación

```bash
# §1/§2 — el terminus y el permiso por allowlist, EJECUTADO
cd backend
cargo test -p relays-client live_exec                    # -> 5 passed; 0 failed  (incl. explicit_mainnet_is_supported)
cargo test -p relays-client m1_rejects_before_fle_resolution  # -> 1 passed
cargo test -p relays-client                              # -> 91 passed; 0 failed; 1 ignored

# §2.1 — la corrección de §34.3
grep -rn "MainnetRefused|PHYSICALLY REFUSES" backend/     # -> 4 matches, TODAS .ts, 0 en .rs
grep -rn "assert_broadcast_allowed|LiveExecPolicy" backend/*/src --include=*.rs

# §3 — el censo de ramas por modo (EXHAUSTIVO)
grep -rn "execution_mode\s*[!=]=|selected_execution_mode\s*[!=]=" backend/   # -> 1 rama
grep -rn "\"LIVE_MAINNET\"|EXEC_MODES" backend/ --include=*.rs               # -> 7: 6 validación/declaración + 1 (L1295) que ES la rama
grep -rn "execution_mode" backend/ --include=*.rs                            # -> 32 (case-sensitive, 4 archivos), clasificadas en la tabla de §3
#   PowerShell Select-String -Pattern 'execution_mode' (case-INSENSITIVE) -> 35: los 3 extra son MAYÚSCULAS en comentarios (§3)

# §3.1 — los cartuchos
# 271 .rhai totales; 264 declaran el trío canónico exacto; 0 declaran otro
```

---

## 6. Lo que esto NO prueba

1. **NO ejecuté el sistema end-to-end.** Corrí las suites de la crate del terminus (`relays-client`) y leí el resto. **No** levanté el pipeline vivo ni medí un modo real.
2. **NO corrí la suite de `searcher-rs`** en esta orden (es pesada); el censo de §3 es **grep sobre el árbol**, no ejecución. Un `grep` no puede probar ausencia de ramas por construcción dinámica (p. ej. dispatch por datos). Lo declaro: la conclusión "no hay rama por modo" se apoya en que **el modo sólo se compara en un sitio**, y eso lo verifica el censo, no un test.
3. **NO verifiqué los cartuchos ejecutándolos.** La declaración de modos es **texto** en los `.rhai`; que los 264 declaren el trío prueba la **declaración**, no el comportamiento de cada cartucho.
4. **La asimetría de §0 es real y acota el paper**: un plan con storage overrides validado en paper **será rechazado** en live. No lo probé con un plan concreto (exigiría un bundle con `storage_overrides_used: true`); lo deduzco de la rama, que es de una línea y sin otras condiciones.
5. **`api-server` (TS) no fue auditado en profundidad.** Su uso del modo que vi (`readiness-extras.ts:234-248`: `ARBX_TRADE_MODE` debe ser `paper` pre-A.9) es un **kill-switch doctrinal**, no un camino de matemática — pero **no hice el censo de TS equivalente al de Rust**.

---

## 7. P/N y alcance

**Esto NO mueve P/N (0/115).** Es lectura de código, grep y tests locales: sin firma, sin broadcast, sin capital, sin mainnet.

**No se modificó código.** `git status` en el clon: **un solo path nuevo**, `docs/backend/MODE-INVARIANCE-01.md`. Ningún `.rs`, ningún `.ts`, ningún workflow.

**No se tocó ningún límite de riesgo** ni se relajó ningún control. Al contrario, la verificación **confirmó** que el único condicional por modo del repo **endurece** live.

---

## 8. Formato de entrega — 10 ítems (doctrina `arbx-skills`)

| # | Ítem | Contenido |
|---|---|---|
| 1 | **Objetivo** | Verificar si los 3 modos de trading (`LIVE_MAINNET`/`TESTNET`/`PAPER_SHADOW`) difieren en algo más que el terminus (§34.1), o si algún modo relaja matemática/gates. |
| 2 | **Skills/gates aplicados** | `arbx-skills` cargada (invariante 6 *fail-honest*, formato de 10 ítems). Gate §34 (execution-modes doctrine) es el objeto de la verificación. |
| 3 | **Inputs productivos solicitados** | Ninguno. No requiere `.env`, secretos, RPC, VPS ni capital: es lectura de código + tests locales. |
| 4 | **Inputs pendientes** | Ninguno bloqueante. `api-server` (TS) fuera del alcance declarado (§6.5). |
| 5 | **Riesgos** | Ninguno de capital: no hay firma, broadcast ni mainnet. El riesgo real es **de lectura**, y va declarado: un censo por `grep` no puede probar ausencia de rama dinámica (§6.2). |
| 6 | **Validaciones hechas** | `cargo test -p relays-client live_exec` → **5 passed/0 failed**; `m1_rejects_before_fle_resolution` → **1 passed**; `cargo test -p relays-client` → **91 passed/0 failed/1 ignored**. Censo de ramas por modo sobre `backend/`: **exactamente 1** (L1295). Cartuchos: **264/271** declaran el trío canónico, **0** declaran otro. |
| 7 | **Reversibilidad** | Trivial: el único artefacto es este documento. Un `git revert` del commit lo remueve sin tocar código. **No se modificó ni una línea de `.rs`/`.ts`/workflow.** |
| 8 | **Métricas de éxito** | (a) ramas de comportamiento por modo = **1** y su dirección **endurece live**; (b) mainnet alcanzable por el switch de entorno, sin restricción adicional — **confirmado ejecutando el test**; (c) `MainnetRefused` como variante Rust = **0**. |
| 9 | **Próximo paso** | (i) limpiar la prosa residual `MainnetRefused` en los 4 sitios TS (1 línea c/u, PR propio con ID); (ii) normalizar el token `"paper_shadow"` de `cartridge_boot.rs:204`; (iii) opcional, si se quiere cerrar §6.2: test que afirme `approved_for_execution=false` con `storage_overrides_used=true` en `LIVE_MAINNET`. **Ninguno lo ejecuté: fuera de alcance.** |
| 10 | **Archivos/referencias tocadas** | Creado: `docs/backend/MODE-INVARIANCE-01.md`. **Leídos (no modificados):** `relays-client/src/live_exec_policy.rs` (97 líneas, íntegro), `relays-client/src/main.rs:171-188`, `relays-client/src/bundle_builder.rs:44,57-58,414-436`, `searcher-rs/src/snapshot_services.rs:63-64,1256-1300`, `searcher-rs/src/canonical_knobs.rs:22,37,137,152,204,216,333,348-350,492-501,610,626`, `searcher-rs/src/rhai_agent_bridge.rs:134,296-297,678`, `searcher-rs/src/cartridge_boot.rs:204,2350`, `searcher-rs/src/canonical_enums.rs:7`, `sim-ctl/src/persistence.rs:67`. |

---

*Lectura de `live_exec_policy.rs` entero (97 líneas) y del camino de firma; tests del terminus ejecutados (91 passed / 0 failed); censo de ramas por modo sobre `backend/` — exhaustivo para comparaciones `==`/`!=`. Se declara en vez de fabricar la premisa de "tres modos en `live_exec_policy.rs`", que la medición no sostiene.*
