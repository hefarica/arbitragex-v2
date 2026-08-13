# OMEGA LOOP — SOP + Handoff R6 (2026-08-12)

> **Sesión R6 cerrada.** La próxima sesión: la memory `arbx-r6-session-2026-08-12-handoff.md` carga auto con el detalle completo. Este doc es el repo-residente.
> main HEAD deployado: `6c7ed108` (VPS anclado). Topología: apex `arbx.ape-tv.net` (Next.js), API `edge-arbx.ape-tv.net` (worker).

## Estado verificado L4 (R6, 2026-08-12)

| Ítem | Estado | PR / fix |
|---|---|---|
| C-02 home gates server-driven | ✅ CLOSED | #324 (mató 12 gates fabricados) + #325 (rate-limit SSR) |
| R6-01 CORS (FASE 0⁵) | ✅ CLOSED | #326 (REST same-origin) + #328 (WS same-origin + CSP) |
| R6-02 pools reshape (FASE 1) | ✅ CLOSED | #329 (worker `{items}`→`{success,data}`) |
| R6-03 rutas 404 | ✅ CLOSED | #327 (46 rutas, agente concurrente) |
| C-06 / R6-07 503 cascade | ✅ CURADA | #330 (cache readiness 20s + dedup) + Tier1 PG_POOL_MAX |
| B-02 swap worker | 🔶 SWAP_DONE_DEGRADED | #321 — swap vivo; residual = pipeline-0 (deserialize bug searcher-rs) |

## Lo que queda (no breakage live)

**OPEN priorizado:**
- **R6-06** auto-deploy silent fail (infra) — workaround `git rev-parse HEAD` post-deploy obligatorio; manual deploy si stale.
- **B-02 residual / pipeline-0** — `pending_received=0` por deserialize bug de searcher-rs en PublicNode WS (`invalid type: map, expected 32 bytes`). Fix = PR Rust. **B-03 cancelado** (creds ya presentes; era código, no config).
- **R6-04** /risk hydration (C-01b residual) · **R6-05** pipeline watch.
- **FASE 4 backlog**: C-03 (unidades fees), C-05/C-07 (títulos/nav), B-04 (WS read-only), D-01…D-05.
- **Residual verifiers no-503**: G-SIM-1 (simulator not ready), PR-1 (probe frontend), V-AT-1 (repo mount).

## Cómo retomar (SOP)

```
1. La memory arbx-r6-session-2026-08-12-handoff.md carga auto (detalle completo).
2. Verificar estado live (3 curls + 1 SSH):
   curl -s https://edge-arbx.ape-tv.net/api/scanner/heartbeat?chain_id=1 | python -m json.tool
   curl -s -o /dev/null -w "%{http_code}\n" https://arbx.ape-tv.net/   # 200
   curl -s -o /dev/null -w "%{http_code}\n" https://edge-arbx.ape-tv.net/api/pools  # 200
   ssh arbx 'cd /opt/arbitragex-v2 && git rev-parse --short HEAD'      # debe ser 6c7ed108 (o mayor)
3. Tomar el siguiente OPEN (recomendado: B-02 residual = PR Rust del deserialize; o R6-06 auto-deploy debug).
4. Flujo: worktree aislado (¡contentión multi-agente es real!) → branch fix/omega-* → PR → CI (14 required) → auto-deploy → **verify SHA VPS (R6-06)** → manual deploy si stale → L4 Playwright.
```

## Lecciones R6 (operativas)

- **L1 — "L4" ≠ curl.** El operador audita Playwright. curl necesario, no suficiente para CLOSED-VERIFIED de UI.
- **L2 — Contención multi-agente.** En este repo, si hay agentes concurrentes, trabajar en `EnterWorktree` desde el inicio (la shared tree pierde edits uncommitted). Ver memory `arbx-multi-agent-worktree-discipline`.
- **L3 — Antes de "aplicar config", verificar que la causa sea config.** B-03 parecía falta-de-creds; era un bug de deserialize en Rust. Verificar logs primero.
- **L4 — auto-deploy no es confiable.** R6-06: marca success sin anclar el repo. SIEMPRE verify SHA + manual deploy si stale.
- **L5 — Pool PG saturable por polling storm.** #330 (cache) es el patrón canónico: endpoints pesados que se pollean deben cachearse in-process, no recompute por call.

## PRs de la sesión (8 merged + Tier1 env)
#321 (B-02 swap) · #324 (C-02) · #325 (rate-limit SSR) · #326 (FASE 0⁵ REST) · #327 (rutas) · #328 (FASE 0⁵ WS) · #329 (FASE 1 pools) · #330 (readiness cache). + `PG_POOL_MAX=35` (env, no PR).
