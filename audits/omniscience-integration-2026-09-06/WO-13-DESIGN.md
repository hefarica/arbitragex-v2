# WO-13 · DESIGN — Adjudicación de la contradicción B-02 (edge de producción)

- **WO:** WO-13 · kind: apply (charter interno: design/adjudicación READ-ONLY + documentación del estándar)
- **Agente:** devops-platform (Gang Omniscience, rubric ecc:code-architect + ecc-docker-patterns)
- **Fecha:** 2026-09-07T00:4xZ local (VPS en UTC). Árbol local: branch `a6-cbprom-01` @ `f7db6867`.
- **Presupuesto dominio público usado:** 0/1 requests HTTP públicos. 1 sesión SSH read-only (`docker inspect` + `docker ps` + `docker logs --tail 3`). VPS NO mutado (§32/§33).
- **Verificación del apply:** `npm run typecheck` en `edge/worker` → EXIT=0 · `docker/compose.prod.yml` parse YAML OK (`edge.dockerfile = edge/worker/Dockerfile.node`). NO-GIT respetado: cero commit/push/PR/deploy.

---

## §0 · VEREDICTO BINARIO

> ## **BRECHA CERRADA — B-02 fue resuelto el 2026-08-11. El edge de producción corre el worker Hono CANÓNICO servido vía `node-server.ts`. La afirmación del board ("prod = Dockerfile.node dev-local, worker Hono es solo POC") es STALE (captura pre-swap de 2026-08-12 repetida por el informe externo). La afirmación del roadmap N2 (línea 17: "VPS corre worker Hono canónico; B-02 resuelto") es VERDADERA y confirmada con evidencia fresca.**
>
> WO-13 se cierra como **documentación del estándar** (opción (a) del charter): los cambios aplicados son comentarios que declaran `Dockerfile.node → node-server.ts` como entrypoint CANÓNICO de producción. NO se requiere plan de cierre con diffs de código.

---

## §1 · Evidencia (1) — `docker/compose.prod.yml`: qué Dockerfile usa `edge`

Hoy (post-comentario WO-13, +4 líneas):

- `docker/compose.prod.yml:410-414` — servicio `edge`: `build.context: ..`, `dockerfile: edge/worker/Dockerfile.node`. Sin `command:` override → el CMD del Dockerfile manda.
- `edge/worker/Dockerfile.node:30` — `CMD ["node","/app/edge/worker/dist/node-server.js"]`, `ENTRYPOINT ["/usr/bin/tini","--"]` (línea 29).
- **El switch es historia datada, no estado ambiguo.** Commit `60f3f702` — *"fix(B-02): switch prod edge from dev-local (DEV-only) to canonical Hono worker (#321)"*, 2026-08-11 23:24:34 -0500 — diff exacto en `docker/compose.prod.yml`:
  ```diff
   build:
     context: ..
  -  dockerfile: edge/dev-local/Dockerfile
  +  dockerfile: edge/worker/Dockerfile.node
   environment:
     ...
     EDGE_PORT: '8787'
  +      REDIS_URL: ${REDIS_URL:-redis://redis:6379}
  ```
  El mismo commit añadió `edge/worker/Dockerfile.node` (25 líneas), `edge/worker/src/node-server.ts` (45), `edge/worker/src/kv-redis.ts` (52) — es decir, el "POC" aterrizó en el MISMO PR que lo promovió a producción.

## §2 · Evidencia (2) — qué corre HOY en el VPS (ssh read-only, 2026-09-07T00:4xZ)

`ssh arbx docker inspect arbitragex-v2-edge-1` + `docker ps` + `docker logs --tail 3`, output textual exacto:

```
IMG=arbitragex-v2-edge
CMD=["node","/app/edge/worker/dist/node-server.js"]
ENTRYPOINT=["/usr/bin/tini","--"]
CONFIG_FILES=/opt/arbitragex-v2/docker/compose.prod.yml
SERVICE=edge
WORKDIR=/app
STARTED=2026-09-06T23:45:32.779179084Z
STATUS=running
---PS---
arbitragex-v2-edge-1 | Up 2 hours (healthy) | arbitragex-v2-edge
---LOGS-TAIL3---
{"event":"edge-worker.node.listen","port":8787,"api_server_url":"http://api-server:8080"}
```

Tres fingerprints independientes, todos apuntan al worker Hono:

1. **CMD** == el CMD de `Dockerfile.node:30` (y ≠ el CMD de dev-local: `node /app/edge/dev-local/dist/index.js`, ver §3).
2. **Línea de boot** `edge-worker.node.listen` existe en UN único lugar del repo — `edge/worker/src/node-server.ts:48` (grep sobre `edge/`, `backend/`, `shared-ts/`, excluidos `node_modules`/`dist`; único hit).
3. **Label** `com.docker.compose.project.config_files` = `compose.prod.yml` (el archivo que apunta a `Dockerfile.node`), servicio `edge`, `running` + `healthy`.

Coincide con lo medido por el auditor N2 (`02-edge-gateway.md:23-24, 86, 98`) en la generación 22:58Z; el contenedor fue recreado 23:45:32Z y la identidad persiste.

## §3 · Evidencia (3) — ¿brecha real o terminología? **Terminología (triple colisión de nombres)**

### 3.1 `Dockerfile.node → node-server.ts` SIRVE al worker Hono — sí, y es el diseño intencional

`edge/worker/src/node-server.ts` (post-edición WO-13):

- `node-server.ts:20-22` — `import { serve } from "@hono/node-server"` + `import app from "./index.js"`. **`index.ts` es la app Hono canónica** (`index.ts:41` — `const app = new Hono<{ Bindings: Env }>()`), declarada byte-idéntica al deploy de Cloudflare (mensaje de `60f3f702`: "The canonical edge/worker/src/index.ts is NOT modified — byte-identical to the Cloudflare deploy").
- `node-server.ts:44-45` — `serve({ fetch: (req) => app.fetch(req, env), port: PORT })` (línea 44) vía `@hono/node-server` (dep en `edge/worker/package.json` deps: `hono ^4.13.3`, `@hono/node-server ^1.13.7`, `ioredis ^5.4.6`).
- `node-server.ts:33-42` — el `Env` del worker se construye desde `process.env` + dos shims `RedisKV` (`ARBX_CACHE` línea 40, `RATE_LIMIT` línea 41) sobre ioredis. Consecuencia arquitectónica ya notada por N2 (`02-edge-gateway.md:24`): rate-limit y cache en prod son **Redis-backed**, no in-memory-per-proceso.

### 3.2 El "POC `poc/edge-worker-node`" NUNCA existió en git

- `ls poc/` → `No such file or directory` (el árbol ni siquiera tiene `poc/`).
- `git log --all --oneline -- 'poc*'` → **vacío** (cero commits en cualquier ref).
- Lo que existió: POC **in-repo** — commit `4dcd2617` *"feat(edge-worker): Node port POC — RedisKV shim + node-server + build config"* + los planes `docs/plans/2026-08-11-edge-worker-node-port-poc.md` y `...-poc-design.md` (añadidos por `60f3f702`). El POC corría en el VPS en `:8788` ("prod edge 8787 untouched. Not yet wired to compose" — mensaje de `4dcd2617`) y fue **promovido** a 8787 en el mismo PR #321. `4dcd2617` no es ancestro standalone de main porque el PR fue squash-mergeado en `60f3f702` (que contiene los 3 archivos: `node-server.ts`, `kv-redis.ts`, `Dockerfile.node` — verificados con `git show 60f3f702 --stat`).

### 3.3 "dev-local" es OTRO artefacto que sigue vivo, pero solo en dev

- `edge/dev-local/Dockerfile` (última línea): `CMD ["node","/app/edge/dev-local/dist/index.js"]` — shim Express DEV-ONLY ("**Do not deploy to production**", `edge/README.md:30`).
- `docker/compose.dev.yml:313` — `dockerfile: edge/dev-local/Dockerfile`. **Solo el stack dev** lo usa; `compose.prod.yml` no lo referencia desde 2026-08-11.

**Conclusión de §3:** no hay dos edges en producción ni un worker sin usar. Hay UNA app Hono canónica (`edge/worker/src/index.ts`) con DOS adaptadores de runtime declarados: `wrangler`/workerd (Cloudflare) y `@hono/node-server` (Docker/VPS, CANÓNICO hoy), más un shim Express legado restringido a dev. La "brecha" del board es un artefacto de tres colisiones: (a) la memory del 2026-08-12 (`arbx-b02-edge-worker-vs-dev-local.md`, encabezado "prod corre dev-local (Express) no worker (Hono)") capturó el estado PRE-swap y el informe externo §2.6 la repitió como presente; (b) `node-server.ts:2` se autodenominaba "POC for B-02" pese a ser el entrypoint prod (corregido en este WO); (c) el sufijo `.node` de `Dockerfile.node` fue leído como "dev-local" cuando nombra el RUNTIME (Node).

## §4 · Cadena de ancestría (todo lo desplegado incluye el fix)

```
git merge-base --is-ancestor 60f3f702 origin/main  → 60f3f702 IS ancestor of origin/main (9ac06d2d)
git merge-base --is-ancestor 60f3f702 main         → IS ancestor of local main (28d48cdd)
git merge-base --is-ancestor 60f3f702 HEAD         → IS ancestor of HEAD (a6-cbprom-01)
git merge-base --is-ancestor 0707db09 origin/main  → IS ancestor  (fix regresión B-02, PR #325
                                                      "exempt SSR-internal traffic from rate-limit")
```

El roadmap (`00-PREDATOR-ROADMAP.md` §1.2) verificó que el VPS `/opt/arbitragex-v2` corre `9ac06d2d` desde 23:45:32Z (deploy veraz). Por tanto el swap B-02 lleva ~26 días en main y en producción, sobrevivió a la cascada de merges #545→#543→#544 (el roadmap N2:17 ya lo certificó: "corre worker Hono canónico; B-02 resuelto").

## §5 · Reconciliación con las fuentes en conflicto

| Fuente | Afirmación | Adjudicación |
|---|---|---|
| Board `GOAL-WORKORDERS.md:21` (vía informe externo §2.6) | "Edge de producción = Dockerfile.node (dev-local), worker Hono es solo POC (B-02)" | **FALSA/STALE** — mezcla tres artefactos (§3.3); el contenedor prod corre el worker Hono (§1, §2) |
| `00-PREDATOR-ROADMAP.md:17` (N2, col. VPS) | "MATCH (corre worker Hono canónico; B-02 resuelto)" | **VERDADERA** — re-confirmada con inspect fresco (§2) |
| `08-monitoring-fleet.md:71` md5 config == contenedor | md5 de `alerts.rules.yml` `e804aa003696c637282ba3e6d04dcaf2` idéntico local↔main↔VPS↔contenedor Prometheus | Válida para Prometheus (coherencia repo↔VPS↔runtime); para edge la equivalencia la dan el CMD + label + boot-log (§2) |
| `WO-14-RUNBOOK.md` §0 | "prod sirve el worker vía `edge/worker/Dockerfile.node` → `node-server.ts` (env desde process.env)" | **VERDADERA** — corroborada (§3.1); su `docker inspect` de edge-1 ya mostraba el mismo contenedor |
| Memory operador 2026-08-12 | "prod corre dev-local (Express)... POC PASS poc/edge-worker-node" | **Raíz del error** — captura PRE-swap + shorthand de path inexistente (§3.2); su propio registro posterior "B-02 SWAP_DONE_DEGRADED" (R6, #325) ya contiene la corrección |

## §6 · Decisión emitida y apply ejecutado (opción (a): documentación del estándar)

Cambios solo-comentario, marcados `WO-13 (2026-09-06)`, en archivos bajo claim:

1. `edge/worker/src/node-server.ts:1-14` — header reescrito: "PRODUCTION edge entrypoint, not a POC", con la cadena de verificación viva (PR #321, CMD del contenedor, boot log) y la separación vs `edge/dev-local/`.
2. `edge/worker/Dockerfile.node:1-5` — header nuevo: "CANONICAL production edge entrypoint (compose.prod.yml `edge` service)... NOT 'dev-local'".
3. `docker/compose.prod.yml:406-409` — comentario sobre el servicio `edge` declarando el estándar (Hono worker under Node; dev-local = DEV-ONLY, compose.dev.yml only).

Cero cambios de comportamiento. `poc/edge-worker-node` (claim del charter): no existe — nada que crear ni tocar (RULE 00: no se fabrica).

## §7 · Residuales reales detectados (NO son B-02; fuera de mi claim → seguimiento)

| # | Residual | Evidencia | Dueño natural |
|---|---|---|---|
| R-1 | `compose.dev.yml:313` sigue construyendo dev-local para el stack dev → paridad dev/prod divergente (tests de integración ejercitan CORS/limiter distintos a prod) | `02-edge-gateway.md:163` | PR de paridad (operador decide unificar o mantener shim documentado) |
| R-2 | Divergencia CORS: worker no tiene la regex `*.ape-tv.net` que dev-local sí tiene | `02-edge-gateway.md:154`, `edge/dev-local/src/index.ts:159` | Idem R-1 (parity-test CI) |
| R-3 | `ALLOWED_ORIGINS` vacío + `EDGE_AUDIT_TOKEN` ausente en env del contenedor edge | `WO-14-RUNBOOK.md` §0, item 3 | **WO-14** (ya diseñado + runbook entregado) |
| R-4 | `wrangler.toml:34-36` sigue con sentinels `REPLACE_ME_*` → la vía Cloudflare-native NO está deployable sin render CI (hoy irrelevante: la vía canónica de deploy ES Docker/Node) | `edge/worker/wrangler.toml:30-36` | Documentado aquí; acción solo si el operador quiere dual-deploy |

## §8 · Límites y honestidad (RULE 00 / R8)

- El `docker inspect` no expone valores de env (solo pedí nombres estructurales CMD/Labels/StartedAt) — no se leyó ni imprimió ningún secreto.
- El hash `e804aa00...` citado en §5 es del auditor 08-monitoring (línea 71), no recalculado por mí (no es necesario para el veredicto del edge).
- `IMG=arbitragex-v2-edge` es imagen construida por compose (sin tag de versión); la identidad del BINARIO dentro se probó por CMD + boot-log fingerprint (§2), método ya validado por N2 (`02-edge-gateway.md:98`).
- No se modificó nada del VPS; los tres archivos editados viven solo en el árbol local compartido (pendiente de PR explícito del operador, protocolo NO-GIT 2026-08-23).

**WO-13: CERRADO — veredicto BRECHA CERRADA, estándar documentado, residuales derivados a sus dueños (WO-14 / PR de paridad).**
