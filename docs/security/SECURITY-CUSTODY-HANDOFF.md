# Security — Handoff: custodia, identidad y límites de confianza on-chain (v1.0.0)

Rol: Security (identidad y confianza). Objetivo de escuadra: "SIGUE".
Base/candidato inspeccionado: rama `fix/perhop-reserves-01`, HEAD `858b943fd80c8b5e1606d220aa87d63b86f4ba15`.
Modo: INSPECCIÓN READ-ONLY. No se modificó ningún archivo de código. No se ejecutó forge (binario ausente en este equipo, igual que en el handoff de Contracts).

## Alcance de esta contribución

Contracts ya cubrió la disciplina de flash-loan (7 reglas) sobre `FlashLoanExecutor` +
`ArbitrageExecutor`. Esta contribución NO re-deriva eso; cubre la capa de **custodia,
identidad y límites de confianza**: quién puede drenar valor, quién puede upgradear,
dónde queda el valor, y si hay secretos expuestos. Es el eje de Security, no de Contracts.

## Superficie verificada (artefactos leídos)

- `contracts/src/FlashLoanExecutor.sol` (413 líneas)
- `contracts/src/ArbitrageExecutor.sol` (638 líneas)
- `contracts/src/AllowanceManager.sol` (211 líneas)
- `contracts/src/AdminTimelock.sol` (90 líneas)
- `contracts/script/DeployMainnet.s.sol` (310 líneas) — handoff de roles
- `contracts/test/DeployMainnetRoleCustody.t.sol` (194 líneas) — regresión P0
- `contracts/test/DeployMainnetTimelockAdminCustody.t.sol` (193 líneas) — regresión timelock admin
- `contracts/foundry.toml`, `contracts/remappings.txt`, `contracts/Makefile`, `contracts/DEPLOY.md`

## Matriz de scopes/custodia (deliverable Security)

| Rol | Superficie (evidence) | Implicación de custodia |
|---|---|---|
| `DEFAULT_ADMIN_ROLE` | `setRouterApproval`/`setTokenApproval`/`setRouterSelectorApproval`/`pause`/`emergencyWithdraw`/`withdrawETH` (AE:518-608); `setBalancerVault`/`setFlashLoanProvider`/`setReferralCode` (FLE:190-228) | Puede `grantRole(UPGRADER_ROLE, x)` a sí mismo → custodia total. DEBE estar timelock-gated. |
| `UPGRADER_ROLE` | `_authorizeUpgrade` (AE:637, FLE:412, AM:210) | Upgrade UUPS = `delegatecall` a código arbitrario → drenaje total. Es LA frontera de custodia. |
| `EXECUTOR_ROLE` (hot signer off-chain + proxy FlashLoanExecutor) | `executeArbitrage`/`executeArbitrageFlashFunded` (AE:252-342); `requestFlashLoan` (FLE:284) | Puede disparar rutas, pero NO retirar (`emergencyWithdraw`/`withdrawETH` son `ADMIN_ROLE`). Blast radius acotado por allowlist A5 + guards de retención. |
| `AdminTimelock` (self-admin) | `schedule`+`execute`; minDelay 86_400 (DeployMainnet.s.sol:179) | Choke point único de custodia. Multisig = proposer+executor, 24h. |

**Invariante de custodia:** *multisig (proposer/executor del timelock) + 24h = único punto de
custodia; el hot signer `EXECUTOR_ROLE` está operacionalmente separado de la custodia.*

## Hallazgos (superficie Security)

### S1 — VERIFICADO (positivo): la frontera de custodia (upgrade UUPS) sí está timelock-gated
`_authorizeUpgrade` en los tres contratos UUPS está gated SOLO por `UPGRADER_ROLE`
(AE:637, FLE:412, AM:210) — y `initialize()` otorga `UPGRADER_ROLE` al deployer EOA
(AE:209, FLE:176, AM:89). La secuencia de handoff en `DeployMainnet.s.sol:226-258` transfiere
ATÓMICAMENTE `UPGRADER_ROLE` **y** `DEFAULT_ADMIN_ROLE` al `AdminTimelock`, revoca ambos del
deployer, y renuncia el bootstrap-admin del deployer sobre el timelock (`tl.renounceRole`,
DeployMainnet.s.sol:258). Orden crítico documentado (grant→revoke, admin LAST, DeployMainnet.s.sol:203-211).
**No es un gap: es la corrección del P0 (auditoría mainnet-readiness 2026-07).** Dos suites de
regresión lo fijan ejecutando el script REAL: `DeployMainnetRoleCustody.t.sol:109-193`
(deployer sin `UPGRADER_ROLE`, timelock con `UPGRADER_ROLE`, upgrade del deployer REVIERTE,
upgrade vía timelock funciona) y `DeployMainnetTimelockAdminCustody.t.sol:115-192` (deployer sin
admin del timelock, self-admin anti-brick intacto, multisig sigue administrando vía schedule+execute).

### S2 — MEDIA (custodia/operativa): sin attestation on-chain del estado de roles en LIVE
La regresión P0 ejercita el script **en fork**. La verificación del estado real en producción es
MANUAL: checklist post-deploy paso 9 (`cast call hasRole(...)`, DeployMainnet.s.sol:304-306). Si un
deploy ejecutó la versión PRE-fix (o un broadcast abortado a mitad del handoff), el deployer EOA
podría conservar `UPGRADER_ROLE` = upgrade instantáneo = drenaje total, y nada lo detectaría
automáticamente. **Fix path (requisito operativo → SRE/Release, no código de contrato):** step
post-deploy (script o CI) que afirme contra la cadena LIVE, para los tres proxies:
`hasRole(UPGRADER_ROLE, deployer)==false`, `hasRole(UPGRADER_ROLE, timelock)==true`,
`hasRole(DEFAULT_ADMIN_ROLE, deployer)==false`. Sin eso, la custodia correcta es "de confianza
en el deploy", no verificada en runtime.

### S3 — confirma C1 de Contracts desde custodia: el bleed por premium consume saldo pre-existente
`ArbitrageExecutor.executeArbitrageFlashFunded` SÍ protege su working capital B con identidad de
retención (`FlashFundedPullMismatch` + `FlashFundedCapitalRetentionViolation`, AE:319-341). Pero
`FlashLoanExecutor` repaga `amount+premium` de su PROPIO saldo (`_executeAndRepayAave`, FLE:397-401)
con único guard `balanceOf >= amount+premium` (FLE:399-400). Si `profit ∈ (0, premium)`, el
executor repaga consumiendo su saldo pre-existente SIN revert — el guard solo dispara cuando el
saldo total (incluido el pre-existente) no alcanza. **El activo en riesgo es el saldo acumulado de
`FlashLoanExecutor`, no el B de `ArbitrageExecutor`.** Security endosa el fix path de Contracts C1 y
lo afina: el invariante on-chain debe ser "el flash jamás reduce el saldo pre-existente del
executor", i.e. snapshot pre-loan y exigir `balanceAfter >= balanceBefore + premium` (o exigir
`profit >= premium`). `premium` ya llega como argumento (FLE:310, 357) — no hardcode.

### S4 — MEDIA (custodia/liquidez): `FlashLoanExecutor` no tiene ruta de extracción de valor
Inventario de funciones de `FlashLoanExecutor` (413 líneas): `initialize`, `setReferralCode`,
`setFlashLoanProvider`, `setBalancerVault`, `selectCheapestProvider`, `requestFlashLoan`,
`executeOperation`, `receiveFlashLoan`, `_executeAndRepayAave`, `_authorizeUpgrade`.
**No existe `withdraw`/`sweep`/`emergencyWithdraw`.** El net profit del flujo flash-funded retorna a
`FlashLoanExecutor` (`msg.sender` de `executeArbitrageFlashFunded`, que es el proxy del wrapper,
DeployMainnet.s.sol:226), y ahí queda: la única salida es el upgrade UUPS timelock-gated (añadir un
sweep) o la pérdida (S3). Esto acopla la liquidez al camino de upgrade. **Fix path (Contracts, con
revisión Security):** añadir APPEND-ONLY un sweep `onlyRole(DEFAULT_ADMIN_ROLE)` espejo de
`ArbitrageExecutor.emergencyWithdraw` (AE:594-598), gated por timelock. Nota fail-honest: observable
desde source; no re-verificado on-chain (forge ausente). Si el operador solo usa el camino
self-funded (`executeArbitrage`, profit en `ArbitrageExecutor` vía `emergencyWithdraw`), S4 queda
dormido — confirmación operativa pendiente.

### S5 — no-hardcode PASS (secretos): superficie limpia
Toda clave privada entra por `vm.envUint("DEPLOYER_PRIVATE_KEY")` / `vm.envUint("GAS_SPONSOR_PRIVATE_KEY")`
/ `--private-key $VAR` (Deploy*.s.sol:59-60, Makefile:99-275, Makefile.crucible:148-167). Cero
literales `0x[a-f0-9]{64}` en `contracts/src` (los 2 hits de grep son `DEFAULT_ADMIN_ROLE=0x00…00`,
DEPLOY.md:168/218). Dos addresses canónicas universales presentes como constant/comment — Aave V3
Pool `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2` (DeployMainnet.s.sol:51, override por env M3) y
Balancer Vault `0xBA12222222228d8Ba445958a75a0704d566BF2C8` (FLE:222, DeployMainnet.s.sol:298) —
ninguna es secreto (constantes universales públicas). Verdict: PASS.

## SBOM evaluado (deliverable Security)

- Superficie de dependencias on-chain: `@openzeppelin/contracts` + `@openzeppelin/contracts-upgradeable`
  (remappings.txt:1-2), señal de versión **5.x** (comentario "OZ 5.x AccessControl revert",
  DeployMainnetRoleCustody.t.sol:39); `forge-std` (solo tests); Solidity `^0.8.20` (optimizer=true,
  runs=200, `via_ir=false`, foundry.toml:11-13).
- **Evaluación de advisories: NO_VERIFICADA.** No se ejecutó herramienta SBOM/advisory (sin forge,
  sin cargo/npm audit). Versión exacta de OZ no fijada (submodule `lib/` no resuelto en este entorno).
  Etiqueta honesta: NO_VERIFIED, no "limpio".

## Estado y fronteras

- **Tests: NO_VERIFICADO.** `forge.exe` ausente (Windows, `ResourceUnavailable`); ningún `.t.sol`
  ejecutado aquí. S1/S5 son inspección estática + lectura del script/tests; S2-S4 son hallazgos de
  source con file:line, no resultados de ejecución.
- **Sin cambios de archivo de código** en esta contribución (solo lectura + este handoff).
- **Sin firma/broadcast/deploy.** S2-S4 son propuestas de reparación/operación, no aplicadas.
- **Frontera respetada:** el fix de C1 (S3) es de Contracts (implementación del snapshot); el
  requisito operativo S2 es de SRE/Release. Security provee el threat model, la matriz de custodia
  y la confirmación del impacto de custodia.

## Siguiente acción recomendada (para Planner)

1. **S2** → SRE/Release: attestation post-deploy del estado de roles en cadena LIVE (3 proxies,
   2 asserts por proxy). Cierra el único hueco entre "custodia correcta en script" y "custodia
   verificada en runtime".
2. **S4** → Contracts (con revisión Security): confirmar operativamente si el camino flash-funded
   se usa; si sí, añadir sweep APPEND-ONLY gated por timelock (espejo `emergencyWithdraw`).
3. **S3** → Contracts (ya en cola como C1): implementar snapshot pre-loan + `balanceAfter >=
   balanceBefore + premium` en `FlashLoanExecutor`; Security revisa el diff.
4. **SBOM** → Release: fijar versión exacta de OZ y correr `forge`/advisory para convertir la
   etiqueta NO_VERIFIED en evidencia.
