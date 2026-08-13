# OMEGA LOOP — Tracker de Corrección Total (ArbitrageX v2)

Fuente de verdad: `Auditoria_Frontend_ArbitrageX_v2_REAUDITORIA_5.md` (R5, 2026-08-12). Directiva v5.
**REGLA 0**: CLOSED solo con merge→main + deploy + L4 en vivo + evidencia. **REGLA 0b**: tracker reconciliado <24h. **REGLA 0c**: vectores separados. **REGLA 0e**: prohibido push directo a main (enforce_admins=True activado 2026-08-12).

> Reconciliado contra R5 el 2026-08-12. 9 anomalías CLOSED-VERIFIED + N-01 CLOSED. Residuales explícitos (A-02b histórico, N-01b enriched/gates).

| ID | Vector | Estado | Merge | Deploy | L4-live (evidencia) | Fecha |
|---|---|---|---|---|---|---|
| **A-01** | API pública | **CLOSED-VERIFIED** | main (equipo) | edge redeploy 08-11 | `/api/audit-logs`→404, `/admin/audit`→401 | 2026-08-11 |
| **A-01** | Página SSR | **CLOSED-VERIFIED** | `fc6c2c90` (#310) | auto-deploy CI | anónimo→gate, 0 emails, 0 eventos, 87KB (era 482KB) | 2026-08-11 |
| **A-02** | paper ledger | **CLOSED-VERIFIED** | `46b9d319` (#318) | auto-deploy CI | outlier guard activo (`paper_archiver.skip_no_sim_profit` en logs); **residual A-02b**: histórico ~36K filas pre-guard auto-expulsadas por ventana 24h | 2026-08-11 |
| **FASE0** | contract test | **CLOSED-VERIFIED** | `63d1791c` (#312) | auto-deploy CI | `schemas.defi-contract.test.ts` en CI de main | 2026-08-11 |
| **A-03** | pools/chains | **CLOSED-VERIFIED** | `63d1791c` (#312) | auto-deploy CI | 79 ACTIVE, 0 DISABLED, PancakeSwap V3 | 2026-08-12 |
| **B-01** | WalletConnect | **CLOSED-VERIFIED** | `5704c5f` (equipo) | desplegado | 0 WC calls en todas las páginas | 2026-08-11 |
| **B-03b** | multi-leg | **CLOSED-VERIFIED** | `d958c9f..ec2579d` (equipo) | searcher redeploy | 50/50 detecciones route_metadata, triangular 3-hop | 2026-08-11 |
| **B-03c** | flujo viable | **CLOSED-VERIFIED** | — | searcher redeploy | ventana 300s activa; último viable 2026-08-10 20:10Z (mercado 0-viable honesto) | 2026-08-12 |
| **C-01** | hydration | **CLOSED-VERIFIED** | `3db4136d` (#315) | auto-deploy CI | /risk: 0 React #425/#422 (Playwright); **residual C-01b**: ~20 superficies con toLocaleString aún (adopción incremental) | 2026-08-12 |
| **C-04** | /monitor | **CLOSED-VERIFIED** | `dcb40b89` (#314) | auto-deploy CI | 0× 92.0%, 0× 42.0%, 5× NOT_AVAILABLE | 2026-08-11 |
| **N-01** | contadores V2 | **CLOSED-VERIFIED** | #319 (`335581f5`) | auto-deploy CI | decoded_ok=1, decoded_err=0 en heartbeat (path V2 ya cuenta); **residual N-01b**: enriched_*/passed_all_gates/db_persisted siguen 0 (FASE 2) | 2026-08-12 |
| **P-02** | push directo | **CLOSED** | GitHub Settings | enforce_admins=True | push directo a main técnicamente imposible | 2026-08-12 |
| **B-02** | endpoints huérfanos | **SWAP_DONE_DEGRADED** | `60f3f70` (#321) | manual deploy | root cause: prod corría edge/dev-local (DEV-only Express) no edge/worker (canonical Hono). Swap vivo (#321). **R6-01 CORS** CLOSED (#326 REST + #328 WS same-origin), **R6-02 pools reshape** CLOSED (#329), **R6-03 rutas 404** CLOSED (#327). **Residual**: pipeline sigue `pending_received=0` — deserialize bug del searcher-rs en PublicNode WS (`invalid type: map, expected 32 bytes`); fix es PR Rust, no config. B-03 cancelado (creds ya presentes). | 2026-08-12 |
| **B-04** | WS read-only | **OPEN** | — | — | /sed, /apex WS_ERROR | 2026-08-09 |
| **C-02** | readiness unificado | **CLOSED-VERIFIED** | `54f0e22` (#324) + `0707db0` (#325) | manual deploy | home gate section server-driven (mató 12 gates fabricados); #325 curó el 429 del SSR. L4 Playwright: home gate live, no fail-honest. C-02b (useLiveTestnetStatus ruta /v1/) residual 1-línea. | 2026-08-12 |
| **C-03** | unidades fees | **OPEN** | — | — | v2 fee 30 vs v3 3000 | 2026-08-09 |
| **C-05** | títulos inflados | **OPEN** | — | — | crucible/worker-health/dex-registry | 2026-08-09 |
| **C-06** | estabilidad proxy/PG | **CURADA (R6-07)** | `6c7ed10` (#330) | manual deploy | root cause: readiness/decision saturaba el pool PG (16 verifiers × cada poll) → verifiers PR-2/G-PAP-1 hit 5s connectionTimeout → 503 cascade a opportunities/strategies. **Fix #330**: cache in-process `collectBlockers` 20s + dedup → PG demand cae ~10-20x → verifiers acquire limpio. L4 Playwright: 0× 503. Tier1 `PG_POOL_MAX` 20→35 aplicado (headroom). | 2026-08-12 |
| **C-07** | nav + typos | **OPEN** | — | — | "reloadso", nav huérfana | 2026-08-09 |
| **D-01** | adapter triangular | **OPEN** (roadmap) | — | — | needs_triangular_adapter | 2026-08-09 |
| **D-02** | RPC registry | **OPEN** | — | — | registry vacío | 2026-08-09 |
| **D-03** | verifier mount | **OPEN** | — | — | /repo/ mount | 2026-08-09 |
| **D-04** | dedupe alertas | **OPEN** | — | — | 171KB/24h | 2026-08-09 |
| **D-05** | writer risk_events | **OPEN** | — | — | event_source not_configured | 2026-08-09 |

## Resumen de cuenta (R6, 2026-08-12)
- **CLOSED-VERIFIED**: A-01 (×2), A-02, FASE0, A-03, B-01, B-03b, B-03c, C-01, **C-02**, C-04, N-01, P-02 = **12 ítems** (+C-02)
- **SWAP_DONE_DEGRADED**: B-02 — swap del worker Hono vivo (#321); R6-01 (CORS FASE 0⁵ #326+#328), R6-02 (pools #329), R6-03 (rutas #327) todos CLOSED; residual = deserialize bug searcher-rs (pipeline-0)
- **CURADA**: C-06 (proxy/PG 503 cascade) — por R6-07 fix #330 (readiness cache)
- **OPEN**: B-04, C-03, C-05, C-07, D-01…D-05, **R6-06** (auto-deploy silent fail), R6-04 (/risk hydration residual), R6-05 (pipeline watch) = **12 ítems**
- **Residuales explícitos**: A-02b (histórico paper), N-01b (enriched/gates/db_persisted), C-01b (LocalTime adopción), C-02b (useLiveTestnetStatus /v1/), WS-L4 (101 handshake sin admin session)

| **R6-06** | auto-deploy silent fail | **OPEN** (infra) | — | — | el workflow Auto-Deploy marca `success` sin anclar el repo VPS al SHA (picó 4/5 hoy). Workaround: `ssh arbx 'cd /opt/arbitragex-v2 && git rev-parse --short HEAD'` post-deploy; manual deploy si stale. Debug del pipeline YAML deferred. | 2026-08-12 |
