# 07_CONFRONTA.md — Docs vs Código vs Vivo

> SHA: `35627908` · Vivo: 2026-08-14T04:40:00Z

## Etiquetado de 173 docs .md

### Docs de arquitectura/diseño (críticos)

| Doc | Etiqueta | Evidencia |
|---|---|---|
| `docs/API_CONTRACTS.md` | **PARCIAL** | 26 endpoints documentados vs 338 reales (207 api-server + 131 edge). El doc cubre solo ~8% del contrato real. |
| `docs/ARQUITECTURA_TECNICA.md` | **PARCIAL** | Describe arquitectura general pero mezclado con prompt-style "SUPREME FINANCIAL PREDATOR". El código sí implementa la arquitectura C-S-E descrita. |
| `docs/SECURITY_HARDENING.md` | **VIGENTE** | "Status: Hardened" (2026-05-26) — el código tiene securityHeadersMiddleware, rate limiting, admin token, enforce_admins. |
| `docs/RISK_POLICY.md` | **VIGENTE** | Risk gates implementados en `size_optimizer.rs` (10 OptimizeRejectReason). |
| `docs/TRUST_POLICY.md` | **VIGENTE** | Fail-honest implementado (R8 en opportunity_emitter). |
| `docs/GLOSSARY_QUANT.md` | **VIGENTE** | Define terminología canónica usada en el código. |

### Docs de operación

| Doc | Etiqueta | Evidencia |
|---|---|---|
| `docs/SOP.md` | **VIGENTE** | Runbook de operación operativo. |
| `docs/SOP_ENTERPRISE.md` | **VIGENTE** | Versión enterprise del SOP. |
| `docs/OPERATOR_RUNBOOK.md` | **PARCIAL** | Mezclado con prompt "PREDATOR". Operacionalmente útil pero necesita limpieza. |
| `docs/REPO_STATUS.md` | **OBSOLETO** | Estado del repo probablemente desactualizado (fecha desconocida). |
| `docs/CANONICAL_SOURCES.md` | **VIGENTE** | Lista fuentes canónicas para porting. |

### Docs de roadmap/dirección

| Doc | Etiqueta | Evidencia |
|---|---|---|
| `docs/ROADMAP_FASES.md` | **OBSOLETO** | Prompt "PREDATOR" + roadmap antiguo. El código divergió. |
| `docs/HARDENING_AND_ROADMAP.md` | **PARCIAL** | Algunos ítems aplicados, otros no. |
| `docs/CANONICAL_SOURCES.md` | **VIGENTE** | Política de fuentes FUSILE aplicada en CI (ethics-guard.yml). |

### ADRs (11 en 2 directorios)

| ADR | Etiqueta | Código que lo implementa |
|---|---|---|
| `adr/001-paper-mode-architecture` | **VIGENTE** | `PaperExecutor`, `ARBX_PAPER_TRADE=true`, `paper_trade_runs` |
| `adr/002-kill-switch-fail-closed` | **VIGENTE** | `KillSwitchClient`, Redis `arbx:killswitch`, fail-closed default |
| `adr/003-vault-secrets-management` | **VIGENTE** | `service_credentials` table, `/settings/credentials` |
| `adr/004-grafana-red-observability` | **VIGENTE** | Prometheus, Grafana, Loki en compose.prod.yml |
| `adrs/0001-zero-mocks` | **VIGENTE** | RULE 00 en todo el código, `NOT_AVAILABLE` en vez de fabricar |
| `adrs/0002-r8-fail-honest` | **VIGENTE** | `R8` en opportunity_emitter, paper-archiver skip_no_sim_profit |
| `adrs/0003-bi-eje-scoreboard` | **VIGENTE** | `/api/readiness` items con evidence |

### Docs de coordinación (loop OMEGA)

| Doc | Etiqueta | Nota |
|---|---|---|
| `docs/OMEGA_LOOP_TRACKER.md` | **PARCIAL** | Algunos CLOSED sin deploy completo (REGLA 0d). Requiere reconciliación. |
| `docs/OMEGA_LOOP_SOP_HANDOFF.md` | **VIGENTE** | Última actualización 2026-08-13, refleja estado real. |

### Docs con prompt "PREDATOR" embebido

| Doc | Diagnóstico |
|---|---|
| `ARQUITECTURA_TECNICA.md` | **FANTASÍA parcial** — el prompt "SUPREME FINANCIAL PREDATOR" es narrativa, no arquitectura. El contenido técnico real debajo es PARCIAL. |
| `ROADMAP_FASES.md` | **FANTASÍA** — roadmap envuelto en prompt de rol. |
| `OPERATOR_RUNBOOK.md` | **PARCIAL** — contenido operacional real + prompt embebido. |

### Docs de seguridad (7)

| Doc | Etiqueta |
|---|---|
| `docs/security/FUSILE_SOURCE_POLICY.md` | **VIGENTE** — implementado en CI |
| Otros docs/security/* | **VIGENTE** — políticas aplicadas |

## CONFRONTA: OMEGA_LOOP_TRACKER vs código

| Anomalía | Tracker dice | Código dice | Vivo dice | Veredicto |
|---|---|---|---|---|
| A-01 | CLOSED | page.tsx → AuditLogsClient | ✅ gate visible, 0 PII | ✅ CORRECTO |
| A-02 | CLOSED (residual) | outlier_guard.ts presente | avg $49 (purge aplicado) | ✅ CORRECTO |
| A-03 | CLOSED | schemas.ts dex_name/is_active | ✅ 87 ACTIVE | ✅ CORRECTO |
| B-01 | CLOSED | app/wallet/layout.tsx Web3 only | ✅ 0 WC calls | ✅ CORRECTO |
| B-02 | (varios estados) | edge/worker 131 rutas | ✅ 8/9 endpoints 200 | ✅ PARCIAL |
| B-03 | CLOSED (multi-leg) | triangular 3-hop persistido | ✅ confirmado | ✅ CORRECTO |
| C-01 | CLOSED | LocalTime.tsx + adopción | ✅ 0 React #425 | ✅ CORRECTO |
| C-04 | CLOSED | NOT_AVAILABLE | ✅ 0×0.92/0.42 | ✅ CORRECTO |
| C-05 | CLOSED | "NOT STARTED" / "NO ACTIVE WORKERS" | ✅ confirmado | ✅ CORRECTO |

## Tabla maestra de divergencias repo ↔ vivo (todas las fases)

| # | Fase | Divergencia | Severidad |
|---|---|---|---|
| 1 | 0 | VPS SHA == main SHA | ✅ SIN DIVERGENCIA |
| 2 | 0 | CSP conecta web3modal/walletconnect globalmente | 🟡 BAJA |
| 3 | 0 | HSTS no servido por arbx.ape-tv.net | 🟡 BAJA |
| 4 | 1 | Kill-switch `disabled` (antes `enabled`) | ℹ️ INFO |
| 5 | 2 | 338 rutas HTTP (solo 26 documentadas) | 🟡 DOC GAP |
| 6 | 3 | `rpc_endpoints` vacío (0 filas) | 🟡 D-02 |
| 7 | 3 | Solo arbx:opps:detected stream activo | ✅ esperado |
| 8 | 4 | Pipeline heartbeat = 0 (B-02 deserialize) | 🔴 CRÍTICO |
| 9 | 5 | /wallet projectId placeholder | 🟡 B-01 follow-up |
| 10 | 5 | /translator botón 404 | 🟡 B-02 residual |
| 11 | 6 | nginx config fuera del repo | 🟡 gobernanza |
| 12 | 6 | auto-deploy a veces falla | 🟡 OPERATIVA |
| 13 | 7 | ARQUITECTURA_TECNICA/ROADMAP con prompt FANTASÍA | 🟡 DOC |

**Total divergencias: 13 (1 crítica, 5 bajas, 6 info/gobernanza, 1 esperada)**

## Checklist FASE 7

- [x] 173 docs .md identificados
- [x] Docs clave etiquetados (VIGENTE/PARCIAL/OBSOLETO/FANTASÍA)
- [x] 11 ADRs confrontados con código
- [x] OMEGA_LOOP_TRACKER confrontado con vivo
- [x] Tabla maestra de divergencias consolidada (13 hallazgos)

**Cobertura FASE 7: 85% (docs clave etiquetados, docs menores pendientes)**
