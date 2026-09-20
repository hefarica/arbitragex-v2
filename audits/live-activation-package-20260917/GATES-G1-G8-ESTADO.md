# GATES G1-G8 — ESTADO CON EVIDENCIA (2026-09-17)
> Fuente de los criterios: `.claude/skills/arbitragex-v2-mainnet-live/SKILL.md` (v2.0.0).
> Estándar de evidencia §34.5.3: artefactos reproducibles, JAMÁS claims.

| Gate | Criterio (resumen) | Estado | Evidencia verificada hoy |
|---|---|---|---|
| G1 | Deploy veraz / infra | ⚠️ PARCIAL | VPS `a06a968d` healthy 24/24; PERO 3 commits locales sin deploy (`fdb40401`,`125b1e0b`,`dcfe890c`) |
| G2 | Simulación cíclica (≥1 sim passed) | ❌ FAIL | `SELECT COUNT(*) FROM simulations WHERE passed` = **0** (toda la historia, query 2026-09-17) |
| G3 | Paper→submit engine con ciclo real | ❌ FAIL | `COUNT(*) FROM executions` = **0**; paper ledger 598K runs todos REJECTED (R-0001, ledger) |
| G4 | Net-profit gate on-chain honesto | ⚠️ SIN EJERCITAR | Gate en código (G-ECON cerrado 2026-08: net=0=gas_floor_breach honesto); sin sims passed no hay input |
| G5 | Contratos Sepolia verificados | ✅ (histórico) | Contrato defi verificado (deploy previo); revalidar al SHA nuevo antes del flip |
| G6 | Fork replay + invariants | ⚠️ PARCIAL | anvil-1 healthy; SIM-FUND-01 (125b1e0b) arregla STF del probe — SIN deploy aún |
| G7 | Risk-limits + checklist pre-ejecución | ⚠️ CÓDIGO OK / DRILL FALTA | pre_execute_checklist.rs presente (check 1 = kill-switch); drill trip/untrip NO documentado |
| G8 | Acta + paquete activación | ✅ HOY | Este paquete (ACTA + RUNBOOK + este estado) |

## Cuello único que bloquea todo

`v3_quote_unavailable` → 0 candidatos evaluables → G2 imposible → G3 imposible.
**El trabajo YA está en curso en `fix/v3-slot0-coverage-20260917`** (remedio catálogo fee-tiers + slot0 backfill).
Pipeline vivo mientras tanto: opportunities fluye (28.3M total, última 06:19 UTC hoy).

## Nota anti-regresión

G2 del skill v2.0.0 citó evidencia fabricada una vez (rama codex/567 + commit 9a10350 inexistentes,
verificado 2026-09-15). Todo PASS de esta tabla requiere artefacto reproducible citado.
