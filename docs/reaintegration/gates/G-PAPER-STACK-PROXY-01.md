# PR-D1 (P0) — deploy del paper stack (ERC1967Proxy + selector de `grantRole`)

- **Tipo:** fix de runtime + gate de fork.
- **Script/evidencia:** `backend/sim-core/src/paper_executors.rs` + fixture
  `backend/sim-core/src/fixtures/ERC1967Proxy.bytecode.hex`.
- **Base:** `origin/main` = `42b68573` (rama local `rea-pr-d1`).

## Dos fallos, no uno

`deploy_paper_executor_stack` NO volvía `Ok` por **dos** bugs encadenados. El
primero era el diagnosticado; el segundo sólo se ve después de arreglar el primero.

### Fallo 1 — `initialize` directo sobre el impl (revert `0xf92ee8a9`)

`ArbitrageExecutor.sol:197` / `FlashLoanExecutor.sol:171`:
`constructor() { _disableInitializers(); }`, y su `initialize` está documentado
*"Must be called exactly once via ERC1967Proxy"*. El código desplegaba el **impl
desnudo** y luego llamaba `initialize` → el modificador `initializer` revierte
`InvalidInitialization` (`0xf92ee8a9`) ⇒ `PaperStackError::AeInitialize`.

### Fallo 2 — selector de `grantRole` mal emitido (revert vacío, `"0x"`)

Con el proxy ya arreglado, el deploy seguía fallando en el paso 4
(`AE.grantRole(EXECUTOR_ROLE, FLE)`) con un revert **vacío** (`reason: "0x"`).
Causa: `encode_grant_role` emitía `[0x2f, 0x2f, 0xff, 0x15]` (`0x2f2fff15`),
pero `grantRole(bytes32,address)` es **`0x2f2ff15d`**. El dispatcher del AE no
encontraba la función y revertía sin datos (el AE no tiene `fallback`).
`encoders_emit_verified_selectors` fijaba **la misma constante equivocada**, por
eso el bug sobrevivió a los tests.

Aislado por ejecución (anvil + el MISMO fixture del repo):

- `cast sig "grantRole(bytes32,address)"` → `0x2f2ff15d`; el encoder emitía `0x2f2fff15`.
- Contra `anvil`, un `grantRole` con el selector correcto **dispatchea**
  (`AccessControlUnauthorizedAccount` con datos desde un caller sin rol); con los
  bytes del fixture revierte vacío.
- `setTokenApproval` —otra `onlyRole(ADMIN_ROLE)`— desde un caller sin rol revierte
  **con datos** (`0xe2517d3f…`): el harness preserva los datos de revert. No era el
  harness ni el hardfork (OSAKA→CANCUN no lo mueve). Era el **calldata**.

## Arreglo aplicado

1. **Tercer fixture** `ERC1967Proxy.bytecode.hex`, emitido por
   `scripts/gen_executor_fixtures.sh` (antes sólo 2 ejecutores).
2. `erc1967_proxy_init_code(proxy_creation, logic, data)` — ABI `(address,bytes)`
   del constructor del proxy.
3. `deploy_paper_executor_stack`: deploy del **impl** → deploy del
   **ERC1967Proxy** cuyo constructor hace `delegatecall(initialize(...))`. La
   dirección viva es el **proxy**; grantRole/allowlists apuntan al proxy.
4. `encode_grant_role` corregido a `0x2f2ff15d` (+ la constante del test).
5. **Guard**: `fixtures_match_forge_output_when_present` cubre también
   `ERC1967Proxy` (ya no es una lista de dos nombres hardcodeados).
6. Tests: `erc1967_proxy_init_code_abi_is_well_formed` (puro, corre siempre) y
   `paper_stack_deploy_initializes_via_erc1967_proxy_ok` (gate de RPC).

## Evidencia

- **Gate de deploy EN VERDE** contra `anvil` desnudo (`--ignored --nocapture`):
  ```
  PAPER_STACK_DEPLOY_OUTCOME=OK ae=0x15452ec016c4dc8c549e7fe6ff4b26324ea8b7a4 \
                                fle=0xb35b8b030a4bc592ea8ccf3684512ce083f108dc
  ```
  Antes de PR-D1: `Err(AeInitialize)` (`0xf92ee8a9`). Con sólo el proxy:
  `Err(GrantRole("0x"))`. Ahora: **`Ok`** con dos direcciones vivas distintas.
- **Suite sobre `main` + el parche:** `cargo test -p sim-core --lib` →
  **80 passed / 0 failed / 1 ignored**.
- **Cross-check `cast`/anvil:** `grantRole(EXECUTOR_ROLE, anvil#0)` por el proxy
  ejecuta (`hasRole(EXECUTOR, anvil#0)=true`).

## Gate

`scripts/gates/g-paper-stack-01.sh` corre de verdad el test de deploy y exige `Ok`:

```
PAPER_STACK_RPC=<url> scripts/gates/g-paper-stack-01.sh
```

Exit `0` = PASS · `1` = FAIL · `2` = SKIP (sin `PAPER_STACK_RPC` o sin cargo). Sin RPC
es SKIP declarado — nunca verde.

Un RPC de fork real sigue siendo la validación canónica (como `variance_benchmark.rs`);
`anvil` desnudo es suficiente para el gate de deploy porque el stack se despliega
DENTRO del fork (no lee estado externo salvo `eth_blockNumber`).

## Aviso antifraude — gas y direcciones (declarado, no "ajustado")

Pasar a ERC1967Proxy **cambia gas y direcciones**: el CREATE del proxy añade coste y
las direcciones vivas son las del proxy (CREATE de `admin`, nonces 1 y 3), no las del
impl. Cualquier cifra de paper calibrada sobre el camino impl-directo puede moverse y
**se documenta, no se "ajusta"**. La dirección del AE desplegado sustituye a
`executor_address` en la config efectiva (`sim_multistep.rs`), así que el
`trace_hash`/`route_hash` de un mismo candidato puede diferir del camino previo.

## Hallazgo (no bloqueante) — `allowanceManager` es inerte

`allowanceManager` en `ArbitrageExecutor` **no se lee en ninguna parte** del camino
de ejecución (sólo declaración, evento `AllowanceManagerUpdated` y
`setAllowanceManager`). ⇒ `address(0)` en paper **no** es divergencia del camino de
ejecución y no hace falta desplegarlo para la paridad. Se registra: el cableado de
producción (`DeployMainnet.s.sol` líneas 280-281) es hoy **inerte**.

## Cableado del spec (hecho)

`MultiStepExecutionConfig.paper_stack` era `None` en TODOS los call sites y **no
existía construcción de `PaperStackSpec`** ⇒ el deploy no se invocaba nunca.
Ahora:

- `PaperStackSpec::for_round_trip(ctx, chain_id, gas_price_wei, gas_limit)` deriva
  el spec de **la misma** `RoundTripContext` que usa el encoder: `tokens` =
  `{token_in, token_out}`; `routers`/`router_selectors` = las patas
  `forward_router`/`backward_router` con el **selector leído del calldata del plan**
  (no un literal); `admin`/`flashloan_callers` = `ctx.caller`. El pool Aave V3 no
  se inventa: se resuelve fail-closed por cadena desde `AAVE_POOL_<chain_id>`
  (`shared_rs::chains::resolve_aave_pool_address`, mismo patrón que
  `EXECUTOR_<id>` / `FLASHLOAN_EXECUTOR_<id>`; sin fallback hardcodeado).
- Los 5 call sites pasan el spec vía
  `sim_core::paper_executors::paper_stack_for_round_trip(...)`:
  `candidate_simulation.rs`, `scanner.rs`, `sim_runner.rs`,
  `canonical_plan_consumer.rs`, `execution_admission.rs`.
- **Flag OFF por defecto (P3):** sin `PAPER_STACK_DEPLOY` el helper devuelve
  `None` ⇒ el comportamiento de todos los call sites es **byte-a-byte el de antes**
  (sim contra el executor de entorno). Con `PAPER_STACK_DEPLOY=1` **y**
  `AAVE_POOL_<chain_id>` se invoca el deploy. Si el flag está puesto y el pool no
  se resuelve, se **degrada a `None` con `warn`** (nunca falla la simulación
  entera), declarado.
- Tests nuevos (corren siempre): derivación desde la ruta + pool por env,
  fail-closed sin `AAVE_POOL_*`, y OFF por defecto sin el flag.

### Pendiente declarado del cableado (R8)

La **activación en producción** exige dos insumos de operador que no son código y
no se inventan: (a) exportar `AAVE_POOL_<chain_id>` con el pool real por cadena, y
(b) decidir **qué** call sites arrancan con `PAPER_STACK_DEPLOY=1` (hoy los tres de
`sim-ctl`/`scanner` ya son `paper_mode=true`; `candidate_simulation` y
`execution_admission` son `paper_mode=false` y no se voltean por defecto). El
cableado está; la decisión de encenderlo es del propietario.

## Revert

Un paso: revertir los commits de PR-D1 restaura el deploy directo del impl +
`initialize` separado (vuelve el `Err(AeInitialize)` con `0xf92ee8a9`) y el selector
mal emitido.
