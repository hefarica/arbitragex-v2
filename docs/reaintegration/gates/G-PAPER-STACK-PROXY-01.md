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
- **Cross-check independiente (2026-10-18, `arbx-d1` sobre `main`@`42b68573`):** contra el
  mismo `anvil`, desplegando impl + proxy **por `cast`** con el fixture del repo:
  `hasRole(ADMIN, anvil#0)=true` y `grantRole(EXECUTOR_ROLE, anvil#0)` ejecuta
  (`hasRole(EXECUTOR, anvil#0)=true`). El contrato y el fixture son correctos.
- **Suite (sobre `main` + este parche):** `cargo test -p sim-core --lib` →
  **80 passed / 0 failed / 1 ignored**. (El `#[ignore]` es el gate de deploy, abajo.)

## Gate

`scripts/gates/g-paper-stack-01.sh` — ejecuta de verdad
`paper_stack_deploy_initializes_via_erc1967_proxy_ok` y exige `Ok`:

```
PAPER_STACK_RPC=<url> scripts/gates/g-paper-stack-01.sh
```

Exit `0` = PASS · `1` = FAIL · `2` = SKIP (sin `PAPER_STACK_RPC` o sin cargo). Sin RPC es
SKIP declarado — nunca verde (un gate manual sin fecha no corre).

## NO cerrado (R8, declarado — no se cuenta como cerrado)

1. **Gate de deploy PENDIENTE — el paso 4 revierte dentro del harness, y NO es el contrato.**
   Corrido contra un `anvil` desnudo (`PAPER_STACK_RPC=http://127.0.0.1:8545`),
   `deploy_paper_executor_stack` devuelve `Err(GrantRole("0x"))`. PR-D1 **avanza** el fallo
   (ya no hay `Err(AeInitialize)`; `hasRole(DEFAULT_ADMIN_ROLE, admin)` **sobre el proxy** =
   `true`), pero no lo cierra. Aislado por ejecución:
   - **No es el contrato ni el fixture:** el impl + proxy desplegados **por `cast`** contra el
     mismo `anvil`, con el fixture del repo, ejecutan `grantRole` sin problema.
   - **No es el estado creado dentro del `CacheDB`:** llamando por el harness al MISMO proxy
     que desplegó `cast` (`0xe7f1725e…`, code real en anvil), `grantRole` también revierte
     `"0x"` — el harness revm revierte `grantRole` incluso sobre un contrato desplegado fuera
     de él.
   - **No son los cambios de estado en general:** por el proxy, `setTokenApproval`,
     `setRouterApproval` y `pause()` (todos `onlyRole(ADMIN_ROLE)`, con SSTORE) **pasan**;
     `hasRole(bytes32,address)`, `EXECUTOR_ROLE()` y llamadas a una EOA también. Sólo
     `grantRole(bytes32,address)` (selector `0x2f2ff15d`, presente en el bytecode) revierte
     con datos vacíos — incluso en la ruta no-op `grantRole(0x00, admin)` (sin SSTORE).
   - **No es el hardfork:** cambiar `SpecId` (OSAKA→CANCUN) en `SequenceContext::new` no lo
     mueve.
   ⇒ Punto abierto para el cierre: por qué `grantRole` por el harness revm revierte con datos
   vacíos mientras `cast`/anvil lo ejecuta (¿bug revm/OSAKA con `AccessControl`, o ruta
   `new_verified`?); y la validación final del gate necesita un RPC de fork real como
   `variance_benchmark.rs`. Se declara, no se simula.
2. **Cableado del spec NO hecho.** `MultiStepExecutionConfig.paper_stack` sigue en `None`
   en TODOS los call sites (`candidate_simulation.rs:465`, `scanner.rs:3528`,
   `sim_runner.rs:209`, `canonical_plan_consumer.rs:162`, `execution_admission.rs:102`) y
   **no existe construcción de `PaperStackSpec`** en producción ⇒ el deploy **no se invoca
   nunca**. Esto NO se declara cerrado: es alcance real pendiente (construir el spec desde
   el candidato/config por cadena y hacer que el call site lo pase), no un detalle.

## Revert

Un paso: revertir el commit restaura el deploy directo del impl + `initialize` separado
(vuelve el `Err(AeInitialize)` con `0xf92ee8a9`).
