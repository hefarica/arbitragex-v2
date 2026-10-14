# PLAN T1-T8 (A4) — esquema del PROMPT MAESTRO

> Artefacto **A4** (§11-bis del prompt maestro). Semilla: `.tmp-inbox/integrado-supremo/00_PLAN/PLAN_T1_T8.md` (198 B, sha256 `6fce3e0bb3d88bc475a68005ee521d69272e9c1708450b602dc6dd9a572b4bf5`).
> Escritura: 2026-10-13 — equipo `prices-v4-integration`, tarea t1 (auditor), rama `feat/exp-20261013` @ 6719f079.
> Fuente autoritativa de insumos: `.tmp-inbox/integrado-supremo/` (paquete INTEGRADO manda; ZIPs previos = referencia).
> Deploy F5/F6 FUERA del plan (RG-01). Λ refiere al release activo del VPS (política 2026-10-13): sha_live=5f8ea3d67 (deploy 2026-10-13T06:16:24Z), drift vs origin/main = 0.

| tarea | descripcion | dep | estado |
| --- | --- | --- | --- |
| t₁ | SSOT + sha256: congelar strategy_hop_map (json/csv) + detector_policy.csv con sha256 registrados; regla de hash §11-bis (cada Aᵢ con sha256 dentro de A1) | — | PENDIENTE |
| t₂ | Rust asserts 264/60/1436: regenerar hop_cycle_bridge.rs + detector_policy.rs desde SSOT; DFAC['D_']:=0.80 | — | PENDIENTE |
| t₃ | harness: t1_schema.test.ts (A6 FIELD_REGISTRY) + t2_mask.test.ts (masks 3..7, hop_dist) montados en rutas reales | — | PENDIENTE |
| t₄ | ledger 0.005: quote_local=quote_onchain ±ε (ε=LEDGER_TOLERANCE_USD=0.005); \|gross\|≤5·principal; net en UNA sola evaluación | — | PENDIENTE |
| t₅ | Gate D SQL: rejection_reason IS NULL OR (computation_status='computed' AND net_amount>0); vocabulario basis + 9 cost labels invariante | — | PENDIENTE |
| t₆ | aceptación binaria: Φ(θ)=g1∧…∧g7 sobre A1 (g1..g6 SOLO del MANIFEST, anti-circularidad §7; g7 por INSPECCIÓN) | — | PENDIENTE |
| t₇ | CI: workflow de Φ sin filtros de paths; ≥5 PRs en observación antes de marcarlo required | t₆ | BLOQUEADO |
| t₈ | ingesta: A2 csv (264 filas) + A8 xlsx (TESTS_ATOMICOS 264.000 filas) con asserts vs SSOT; \|hojas\|≥60 al añadir REGISTRO_INCLUSIONES | t₇ | BLOQUEADO |

## Estado real al momento de escribir (2026-10-13)

- **Ninguna tᵢ del esquema está lista.** Los insumos existen y están verificados, pero nada se ha congelado, regenerado ni montado en el repo: t₁–t₆ = PENDIENTE.
- **t₇ y t₈ BLOQUEADOS** por dependencia (la semilla los marca "(BLOQ)"): sin Φ(θ) cerrado no hay CI de Φ; sin CI no hay ingesta final. Deploy F5/F6 fuera del plan (RG-01).
- Avance verificable ya realizado (t1-equipo, evidencia `docs/integration-v4/t1-evidence/T1_inventario_sha256.txt`):
  - sha256(xlsx CORREGIDO) = `eb5b803cb66d0dcbb5c17c9885e9a3dc041e01e94c5bc22a37ef0482e4e09cc3` == MANIFEST sha256(xlsx) == mismo hash del xlsx DENTRO de `arbitragex-prompt-supremo-INTEGRADO.zip` (cadena de custodia cerrada).
  - xlsx A8 tiene **59 hojas fijas** (LEEME, RESUMEN, CLASIFICACION_264, AGREGADOS_SUPERFICIE, AGREGADOS_DETECTOR, INVARIANTES, CRITERIOS_T6, PLAN_T1_T8, TESTS_ATOMICOS, A01–A20, F01–F14, S01–S10, I01–I06) ⇒ la invariante final **|hojas|≥60 queda PENDIENTE** de añadir REGISTRO_INCLUSIONES (A7) y ADM-n durante la ejecución.
- Base del paquete: MANIFEST `repo@5f8ea3d67c964d152ae8fda89883952de8282dfb` == origin/main local == release activo en VPS; HEAD de trabajo `feat/exp-20261013`=6719f079 (divergencia a reconciliar por t2-equipo).

> **Regla de hash §11-bis**: el sha256 de ESTE archivo (A4) debe quedar registrado dentro de A1 cuando se regenere (t6-equipo).
