# PR-D1 (P0) — deploy del paper stack vía ERC1967Proxy (revert `0xf92ee8a9`)

- **Tipo:** fix de runtime + gate de fork.
- **Script/evidencia:** `backend/sim-core/src/paper_executors.rs` + fixture `backend/sim-core/src/fixtures/ERC1967Proxy.bytecode.hex`.

## Diagnóstico confirmado

`deploy_paper_executor_stack` devolvía `Err(AeInitialize)` con revert `0xf92ee8a9`
(`InvalidInitialization`). El CREATE del **impl** completa; el fallo está en el
`initialize` siguiente: el constructor del impl llama `_disableInitializers()`
(patrón OZ UUPS), así que `initialize` **directo sobre el impl** revierte.

## Arreglo aplicado

1. **Tercer fixture** `ERC1967Proxy.bytecode.hex` (968 bytes, de `contracts/out/ERC1967Proxy.sol`),
   emitido también por `scripts/gen_executor_fixtures.sh` (antes sólo emitía los 2 ejecutores).
2. `erc1967_proxy_init_code(proxy_creation, logic, data)` — ABI `(address,bytes)` del
   constructor del proxy.
3. `deploy_paper_executor_stack` ahora: deploy del **impl** → deploy del **ERC1967Proxy**
   cuyo constructor hace `delegatecall(initialize(...))`. La dirección viva es el **proxy**;
   todos los pasos siguientes (grantRole, allowlists) apuntan al proxy. El fallo del deploy
   del proxy se mapea a `AeInitialize`/`FleInitialize` (taxonomía intacta).
4. Tests: `erc1967_proxy_init_code_abi_is_well_formed` (puro, corre siempre) y
   `paper_stack_deploy_initializes_via_erc1967_proxy_ok` (`#[ignore]`, requiere RPC —
   construye el `PaperStackSpec`, ejecuta el deploy y afirma `Ok`, igual que
   `variance_benchmark.rs`).

## Evidencia

- **Validación del encoding + inicialización (hardware real, mismo fixture):** contra
  `anvil` con `cast` — se despliega el impl y el proxy con el MISMO init code que produce
  `erc1967_proxy_init_code`, y `hasRole(DEFAULT_ADMIN_ROLE, admin)` sobre el proxy devuelve
  **`true`** ⇒ el constructor del proxy inicializa correctamente.
  ```
  IMPL = 0x5fbdb2315678afecb367f032d93f642f64180aa3
  PROXY = 0xe7f1725e7734ce288f8367e1bb143e90bb3f0512
  hasRole(ADMIN, 0x1111…1111) = true
  ```
- **Suite:** `cargo test -p sim-core` → **81 passed / 0 failed / 1 ignored**.

## NO cerrado (R8, declarado — no se cuenta como cerrado)

1. **Gate de fork PENDIENTE de un RPC real.** El test `#[ignore]`
   `paper_stack_deploy_initializes_via_erc1967_proxy_ok` corrió contra un `anvil` desnudo:
   los pasos 2–3 (deploy impl+proxy+initialize) **pasan** (ya no hay `AeInitialize`), pero el
   paso 4 (`AE.grantRole(EXECUTOR_ROLE, FLE)`) revierte con `0x`. La MISMA secuencia
   verificada con `cast` contra el mismo `anvil` **sí** funciona, y una llamada directa al
   impl también revierte dentro del harness ⇒ el entorno REVM+`LazyDb`→anvil de este
   sandbox no ejerce correctamente las cuentas creadas en el `CacheDB`. La validación válida
   requiere un RPC de fork real (como `variance_benchmark.rs`); queda sin ejecutar aquí.
2. **Cableado del spec NO hecho.** `MultiStepExecutionConfig.paper_stack` sigue en `None`
   en TODOS los call sites (`candidate_simulation.rs:465`, `scanner.rs:3528`,
   `sim_runner.rs:209`, `canonical_plan_consumer.rs:162`, `execution_admission.rs:102`) y
   **no existe construcción de `PaperStackSpec`** en producción ⇒ el deploy **no se invoca
   nunca**. Esto NO se declara cerrado: es alcance real pendiente (construir el spec desde
   el candidato/config por cadena y hacer que el call site lo pase), no un detalle.

## Revert

Un paso: revertir el commit restaura el deploy directo del impl + `initialize` separado
(vuelve el `Err(AeInitialize)` con `0xf92ee8a9`).
