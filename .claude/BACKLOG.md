# BACKLOG — fuera de scope de los 5 pasos (RG-01 v3.0 §4.3/§8.8)

> Todo lo que aparece durante la ejecución del plan y NO pertenece al paso en curso
> se registra acá y NO se arregla en el paso. Referencia: `.claude/RG-01.md`.

| Ítem | Origen | Estado RG-01 | Nota |
|---|---|---|---|
| Deploy de `NLEG-DUST-BAND-01.patch` (grid middle_probes → dust) | sesión 2026-10-12 | en código (patch sin commit), NO desplegado | parche en raíz del árbol local, +594/−21, base c4b61470 |
| Decisión allowlist YELLOW (`0x236eb848…`) / SD (`0x30d20208…`) | sesión 2026-10-12 | NO MEDIDO (política del operador) | únicas bases con ε plausible histórica; gate: `arbx-token-safety-screen` |
| Cobertura catálogo V3 + transporte quoter (852k rechazos/24h: `v3_pool_not_catalogued` 455k + `v3_quote_unavailable` 397k) | sesión 2026-10-12 | medido en PG | mayor palanca post-STEP-1 |
| Exposición del ledger del kernel en `dex_engine` (`cycle_output_not_exposed_by_kernel`, 91.6% de filas computed sin legs) | sesión 2026-10-12 | medido en PG | ε no exhibible en 15/17 cards |
| Normalización de bases de notional mezcladas (14 intent / 3 kernel en cards) | sesión 2026-10-12 | medido en wire | salto $0.01↔$2,547 en pantalla |
| Masa de pies RG-01 en 4,832 `*.md` del árbol principal — SIN COMMIT | orden 2026-10-12 | aplicado en working tree | decision de commit/push del operador |
| Árbol local `fix/perhop-reserves-01` a 353+ commits de `origin/main`, 927+ mods sin commitear | diagnóstico 2026-10-12 | medido (git) | riesgo R13 |
| Reboot pendiente post-fix SAC (`VerifiedAndReputablePolicyState=0` escrito, efecto completo al reiniciar) | operador 2026-10-12 | CONFIRMADO registro=0 | falsificación: worktree frío + `cargo check` post-reboot |
