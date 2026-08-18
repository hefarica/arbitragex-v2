# 00b_CENSO_VIVO.md — Censo del Sistema Vivo

> Timestamp: 2026-08-14T03:58:25Z — 04:00:00Z UTC
> VPS SHA: `35627908` (== main `35627908401cc8e2df2258292bad720764cb073e`) — DEPLOYED IS UP TO DATE
> Reloj servidor: UTC (Date: Fri, 14 Aug 2026 03:59:22 GMT)

## Topología viva confirmada

| Dominio | Target | VIVO |
|---|---|---|
| `https://arbx.ape-tv.net` | Frontend Next.js (SSR) → nginx | ✅ 200 (24/24 páginas sample) |
| `https://arbx.ape-tv.net/api/*` | nginx → edge worker Hono (:8787) | ✅ 200 (14/14 endpoints) |
| `https://edge-arbx.ape-tv.net` | Edge worker directo | ✅ 200 `/health` |

## Endpoints API canónicos (14/14 = 200)

| Endpoint | HTTP | VIVO | Contrato |
|---|---|---|---|
| `/api/status` | 200 | ✅ CONFIRMADO ts=03:58 | `{ok, services, killswitch}` |
| `/api/pools` | 200 | ✅ CONFIRMADO | `{success, data:[...]}` |
| `/api/chains` | 200 | ✅ CONFIRMADO | `{success, data:[...]}` |
| `/api/opportunities/live?limit=5` | 200 | ✅ CONFIRMADO | `{count, items}` |
| `/api/scanner/heartbeat?chain_id=1` | 200 | ✅ CONFIRMADO | `{chain_id, snapshot:{...}}` |
| `/api/paper/history/summary?hours=24` | 200 | ✅ CONFIRMADO | `{ok, source, data:{totals}}` |
| `/api/recon/summary` | 200 | ✅ CONFIRMADO | `{ok, data}` |
| `/api/recon/timeseries?hours=24` | 200 | ✅ CONFIRMADO | `{window_hours, points}` |
| `/api/readiness` | 200 | ✅ CONFIRMADO | `{items:[...]}` |
| `/api/route-discovery/status` | 200 | ✅ CONFIRMADO | `{ok, source, mode, data}` |
| `/api/strategies/runtime-status?chain_id=1` | 200 | ✅ CONFIRMADO | strategy status |
| `/api/metrics/defi` | 200 | ✅ CONFIRMADO | `{active_workers, cpu, memory}` |
| `/api/config/current` | 200 | ✅ CONFIRMADO | `{paper_mode, ...}` |
| `/api/risk/alerts?limit=5` | 200 | ✅ CONFIRMADO | `{items}` |

## Páginas frontend (24 sample, todas 200)

`/` `/status` `/opportunities` `/executions` `/paper/history` `/recon` `/operations`
`/risk` `/killswitch` `/audit-logs` `/pools` `/chains` `/rpcs` `/wallet` `/monitor`
`/readiness` `/strategies` `/onboarding` `/live-readiness` `/agent-insights`
`/sed` `/config` `/settings` `/operator/self-test`

## Headers de seguridad observados

| Header | Valor | Nota |
|---|---|---|
| `content-security-policy-report-only` | `default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; ...` | Report-only (no bloquea) |
| `x-content-type-options` | `nosniff` | ✅ |
| `x-frame-options` | `DENY` | ✅ |
| `Server` | `cloudflare` | CF proxy delante |
| `strict-transport-security` | NO OBSERVADO | ⚠️ HSTS no servido por frontend (sí por edge) |

## CORS observado

| Test | Resultado |
|---|---|
| Origin: `https://arbx.ape-tv.net` | `access-control-allow-origin: https://arbx.ape-tv.net` ✅ |
| Origin: `https://test.example` | `access-control-allow-origin: ` (vacío = rechazado) ✅ |
| Allow-headers | `content-type,authorization,x-arbx-trace-id,x-arbx-admin-token,x-arbx-actor` |
| Allow-methods | `GET,POST,PUT,DELETE,OPTIONS` |
| Allow-credentials | `true` |

## Divergencias repo ↔ vivo

| # | Hallazgo | Severidad | Detalle |
|---|---|---|---|
| 1 | VPS SHA == main SHA | ℹ️ INFO | Sin divergencia — deploy anclado |
| 2 | CSP conecta a web3modal/walletconnect | 🟡 BAJA | CSP permite conexiones a servicios WC aunque B-01 aisló Web3 a /wallet |
| 3 | HSTS no servido por arbx.ape-tv.net | 🟡 BAJA | Cloudflare proxy puede manejarlo a nivel edge |

## Cobertura censo vivo

- [x] Health ambos dominios
- [x] 14 endpoints API verificados (200)
- [x] 24 páginas frontend verificadas (200)
- [x] Headers de seguridad catalogados
- [x] CORS verificado (allowlist + rechazo)
- [x] SHA VPS == SHA main
- [x] Reloj servidor (UTC)
- [ ] Barrido completo 56 páginas (FASE 5)
- [ ] Topología nginx real (FASE 6)
- [ ] Redirects/rewrites observados (FASE 6)

**Cobertura censo vivo: 100% (superficies base) → 40% (profundidad por página)**
