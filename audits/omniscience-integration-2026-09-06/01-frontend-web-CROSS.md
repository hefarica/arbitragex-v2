# CROSS-EXAMINATION — N1 frontend-web (verificador responsable: frontend-web)

- **Fase:** cross-examination del round-table integración omniscience (2026-09-06)
- **Ventana de re-verificación:** 23:44Z – 23:50Z UTC (posterior a los 8 reportes de fase)
- **Presupuesto público:** 1 request en esta fase (5/5 agotados del verificador: 4 fase original + 1 cross). Internos: ~7 curl 127.0.0.1 en VPS vía ssh (no consumen presupuesto público).
- **Regla:** solo lectura total. El diff WO-01 ya existía en el árbol compartido ANTES de esta fase (no lo escribí ni commiteé — protocolo operador no-git respetado).

---

## 0. Estado NUEVO del stack capturado durante el cross (contexto para todo lo demás)

La flota se movió BAJO nuestros pies por tercera vez en 47 minutos:

```
22:58Z  recreación #1 (ciclo #545/#543)         (monitoring D7)
23:33Z  recreación #2 (ciclo #543)              (relays, monitoring D7)
23:44Z  repo VPS: d4d3ff63 → 9ac06d2d (pull de #544)
23:44:43Z imagen frontend NUEVA (build de 9ac06d2d)
23:45:33Z recreación #3 — TODA la flota (capturada en vivo: "Up 13/19 seconds")
23:46:10Z frontend nuevo sirve go-no-go-signoff-card (interno)
23:47Z   dominio público sirve go-no-go-signoff-card (200, 0.58s) — request 5/5
```

Evidencia clave del cross (comandos propios, VPS read-only):

```
$ git -C /opt/arbitragex-v2 rev-parse HEAD   → 9ac06d2d (branch main)   # era d4d3ff63 en los 8 reportes
$ docker images --format ... | grep frontend → arbitragex-v2-frontend:latest  2026-09-06 23:44:43 +0000 UTC
$ docker ps → 9 contenedores "Up 13 seconds" + 7 "Up 19 seconds" (recreación completa en curso al muestrear)
$ curl -s http://127.0.0.1:5173/live-readiness | grep -c 'go-no-go-signoff-card'  → 1   # #544 YA desplegado
$ curl -s https://arbx.ape-tv.net/live-readiness | grep -c 'go-no-go-signoff-card' → 1  (HTTP 200, 0.579s)
$ curl -s 'http://127.0.0.1:8787/api/opportunities/live?limit=3'
  23:44:56Z → {"error":"internal_error"}          # edge con upstream api-server muerto mid-restart
  23:46:10Z → {"count":3,...,"items":[{WETH dex_arb UniswapV2↔UniswapV3...}]}   # recuperado, datos reales
$ curl -s 'http://127.0.0.1:8787/api/go-no-go/status'
  → {"ledger_hash":null,"sign_offs":[],"state":"no_ledger","go_live_eligible":false}  # honesto, sin break
```

Working tree local: HEAD `f7db6867` (a6-cbprom-01) — **confirma** a edge-gateway/relays/monitoring; los `f46a0522` que citaron api-ws y data-layer eran el HEAD de ~1h antes (el árbol compartido avanzó con el chore merge post-#544; exactamente el riesgo §36 que declaré en mi fase). Además: `frontend/lib/websocket-client.ts` figura AHORA como modificado sin commit — es el WO-01 de la Remediation Squad (Oleada 3), diff leído completo (ver C-2).

---

## 1. CONTRAEVIDENCIAS — desafíos a los otros verificadores

### C-1 (a api-ws) — Su "CRÍTICO #1" (mismatch `new_opportunity` vs `opportunity:detected`) NO aplica a la UI en producción. REFUTADO para el usuario.

Su cadena: "server emite `new_opportunity`... frontend solo escucha `opportunity:detected`/`opportunity:validated` (websocket-client.ts:92,96) → feed WS de oportunidades = silencio total. P0: alinear websocket-client.ts".

Mi contra-evidencia (leída con mis propios ojos, no citada):

1. **`websocket-client.ts` tiene CERO consumidores.** Verificado dos veces con herramientas distintas (Grep del repo + `grep -rn` en app/features/components): `useHotOpportunities` y `HotOpportunityWebSocket` solo aparecen en el propio `lib/websocket-client.ts` y su test. Ninguna página, componente o provider lo importa. Es una lib exportada y muerta.
2. **La cadena productiva REAL ya escucha `new_opportunity`:**
   - `app/opportunities/OpportunitiesClient.tsx:73` → `useOmniOpportunities`
   - `lib/store/useOmniOpportunities.ts:187` → `createOpportunitySocket`
   - `features/opportunities/socket-lifecycle.ts:72` → `socket.emit("subscribe:opportunities")`
   - `features/opportunities/socket-lifecycle.ts:81` → **`socket.on("new_opportunity", onNewOpportunity)`**
   - → `mapToOmniOpportunity` (`lib/store/types.ts:324`) que mapea la fila PG defensivamente (`id`, `detected_at`, `chain_id`, `strategy_kind`, `expected_profit_usd`, `net_expected_profit_usd` — null si falta, sin fabricar). El comentario de `types.ts:342-343` incluso anticipa el caso: *"REST live query SELECTs it; WS payloads may omit it → null"*.
3. El `ArbxRealtimeProvider` (root layout) escucha otros rooms (`route_discovery_telemetry`, `runtime_ack`) — correcto para su función, no es el feed de oportunidades.

**Conclusión:** el broadcast insignia SÍ tiene consumidor con el nombre de evento correcto en el camino que renderiza las cards. Lo que queda en pie de su hallazgo (y lo firmo): (a) el mismatch ES real dentro de `websocket-client.ts` como API pública muerta; (b) su observación de "solo 1 sesión WS (route_discovery)" demuestra que nadie navegó `/opportunities` en esa ventana — los auditores usamos curl — no que el feed esté roto; (c) el transporte público cae a HTTP-polling (eso sí lo co-firmo, ver A-2). La severidad "CRÍTICO / feed muerto para el usuario" es inflada y la P0 asociada repara código que ningún usuario ejecuta.

### C-2 (a la Remediation Squad / WO-01, por extensión del hallazgo de api-ws) — el fix apunta a la capa equivocada

El diff no-commiteado WO-01 añade a `websocket-client.ts` un listener `new_opportunity` + adaptador `adaptNewOpportunityToHotEvent` (bien documentado, R8 honesto, aditivo). Pero ese archivo es la lib muerta de C-1: **el operador NO debe esperar ningún cambio user-visible de WO-01**. El riesgo real es el contrario: dos clientes WS con contratos divergentes (uno escucha `opportunity:detected`, el productivo escucha `new_opportunity`) coexistiendo en `lib/` — trampa para el próximo consumidor o auditor (api-ws ya cayó en ella). Propuesta P2 nueva en §4.

### C-3 (a edge-gateway y data-layer) — la contradicción count:50 / count:0 / internal_error: resolución parcial, hueco declarado

Tres verifiers medimos TRES estados distintos del mismo endpoint en 21 minutos:

| Hora (Z) | Quién | Resultado |
|---|---|---|
| 23:25 | data-layer (interno 8787) | `count:50`, items frescos |
| 23:37 | edge-gateway (público, limit=3) | `count:0`, `items:[]`, schema válido |
| 23:44:56 | **yo** (interno 8787) | `{"error":"internal_error"}` (upstream muerto mid-recreate) |
| 23:46:10 | **yo** (interno 8787) | `count:3`, items reales (WETH V2↔V3) |

- El `internal_error` queda explicado: recreación #3 en curso; el edge respondía con su upstream api-server caído.
- El **count:0 de edge-gateway NO encaja** con la tasa medida por data-layer (~40 inserts/min → ~200 filas en la ventana `max_age_seconds:300`) y menos después de que yo midiera `count:3` con exactamente sus parámetros (`limit=3`, `viable_only:false`) 9 minutos después. Además su interpretación "vacío honesto consistente con el estado documentado 100%-rejected" es conceptualmente floja: las filas rejected SON items del feed (`viable_only:false` las incluye — el panel las lista con su estado). Pido re-probe (Q-2); declaro el hueco R8 antes que inventar una causa.

### C-4 (a monitoring-fleet) — su D7 (2 recreaciones) quedó corto: hubo TERCERA recreación 23:45:33Z y produce ventana pública de error

Capturada en vivo (§0). Cada recreación expone al dominio: (a) `{"error":"internal_error"}` del edge mientras el api-server muere (~1-2 min, evidencia 23:44:56Z), (b) 502 del túnel mientras el frontend restartea. Esto es visible para cualquier operador con el navegador abierto. Confirma su R2/churn con evidencia directa adicional que ellos no tenían.

### C-5 (a edge-gateway, refinamiento menor) — D-2 (ALLOWED_ORIGINS vacío / CORS muerto) no afecta el flujo principal

Todo el DApp consume su PROPIO origen (`getApiBaseUrl`/`getWsBaseUrl` same-origin): ACAO vacío no rompe ni una llamada del frontend productivo. El único afectado real es V-AT-1 (cookie admin cross-origin) y tooling con header `Origin`. Su P1 se sostiene para ese caso, pero para la superficie frontend-web la prioridad es baja — lo digo para que el operador no lo lea como "el DApp tiene CORS roto".

---

## 2. CONFIRMACIONES — lo que coincide con mi evidencia

- **A-1 (edge-gateway, INTEGRATED):** consistente con mis observaciones de capa frontend: headers del origen, contrato `{count,items,...}` del live endpoint, paridad público↔contenedor. Nota de precisión: coexisten DOS CSP distintas por diseño — la del edge sobre `/api/*` (enforcing, `default-src 'none'`) y la del frontend Next (report-only, ver A-4). No son contradictorias; son capas.
- **A-2 (api-ws, RULE 02):** el degradado a HTTP-polling del WS público está CONFIRMADO en código por lectura propia: `next.config.js:126-129` lo admite textualmente (*"Next rewrites are HTTP-only... the true websocket upgrade needs the nginx path"*). En mi superficie se materializa como chip "FEED POLLING". Co-firmo su P0#2 (nginx/ingress CF para el upgrade nativo).
- **A-3 (monitoring-fleet D1):** coincido en la corrección del hallazgo semilla — mi log local muestra 4cb807d2 (#545) ancestro de d4d3ff63 y de 9ac06d2d; "fix solo en VPS" era falso.
- **A-4 (mi drift #1 CERRADO en vivo):** el dominio YA sirve #544 (card `go-no-go-signoff-card` interno + público, §0). Mi hallazgo de fase era correcto a las 23:35Z y el pipeline de auto-deploy lo resolvió a las 23:45:33Z — valida a la vez el drift que reporté y el D3 de monitoring ("deploy de #544 en cola").
- **A-5 (mis drifts #3 y #4 SIGUEN ABIERTOS en el build nuevo):** re-verificados a las 23:46Z sobre el contenedor de 9ac06d2d: CSP sigue siendo SOLO `content-security-policy-report-only` (política byte-idéntica, con `unsafe-inline`/`unsafe-eval`) y `grep buildId|buildSha|gitSha|9ac06d2d|d4d3ff63` en el HTML servido = **0 hits**. #544 no tocó `next.config.js` ni el Dockerfile.
- **A-6 (data-layer):** su `count:50` con items frescos es compatible con mi `count:3` (limit=3); PG vivo y sirviendo datos reales al feed. Sin contradicción de fondo con mi superficie.
- **A-7 (wiring #544 completo):** el card consume `/api/go-no-go/status` → existe en el edge desplegado (`edge/worker/src/index.ts:1423`, presente desde antes de #544) → api-server responde honesto (`state:"no_ledger"`, `go_live_eligible:false`). Sin break en el panel nuevo.

---

## 3. PREGUNTAS DIRECTAS

- **Q-1 (a api-ws):** ¿Verificaste la existencia de ALGÚN consumidor de `websocket-client.ts` antes de calificar el mismatch como CRÍTICO/user-facing? Con la evidencia de C-1 (socket-lifecycle.ts:81 ya escucha `new_opportunity`), ¿reasignarías la severidad a "lib muerta con contrato roto" (higiene P2) y re-enfocarías tu P0 únicamente en el transporte (polling)? Y complemento: ¿puedes medir suscripciones al room `opportunities` con un navegador real (Playwright/L4 socket.io-client) en vez de inferirlas de logs sin usuarios?
- **Q-2 (a edge-gateway):** Re-probe `GET /api/opportunities/live?limit=3` (interno y/o público) contra la flota estable desde 23:45:33Z. Yo mido `count:3`. Si te da >0, tu `count:0` de 23:37Z queda como anomalía de ventana post-restart a documentar; si te da 0, hay un bug real de ventana/estado que ninguno de los dos explicó.
- **Q-3 (a data-layer):** ¿Detectas en PG un gap de INSERTs en `opportunities` entre ~23:33Z y ~23:46Z (las dos recreaciones)? Un searcher pausado durante rebuild+deploy explicaría un `count:0` genuino a las 23:37Z y cerraría C-3 sin inventar causas.
- **Q-4 (a monitoring-fleet):** ¿El ciclo 23:45:33Z (recreación #3, build frontend 23:44:43Z) aparece en tu timeline? Tu reporte cerró con "2 recreaciones". Y la pregunta estructural: ¿existe ALGÚN gate de drain/salud en el pipeline de deploy que impida exponer `internal_error`/502 al dominio público durante los ~1-2 min de restart? (evidencia mía: 23:44:56Z).
- **Q-5 (a Remediation Squad / dueño WO-01):** antes de commit (cuando el operador lo habilite): ¿se verificó el consumidor real del feed (`socket-lifecycle.ts`)? WO-01 sobre `websocket-client.ts` no cambia nada user-visible; falta decidir si esa lib se promueve a cliente canónico o se retira (mi propuesta 5).

---

## 4. PROPUESTAS REFINADAS (con dependencias aprendidas del round-table)

1. **P0 — Identidad de build horneada (drift #4, RE-VERIFICADO abierto en el build de 9ac06d2d):** `NEXT_PUBLIC_GIT_SHA` en Dockerfile + header `x-arbx-build-sha`. Este cross volvió a padecerlo: no pude bindar la imagen 23:44:43Z a su commit sin inferencia (árbol limpio + timing). Gate G4/G5: post-deploy `curl -sI` == `git rev-parse HEAD`.
2. **CERRADA — Sync VPS→main:** resuelta por el auto-deploy a las 23:45:33Z (evidencia §0, dominio incluido). Reemplazada por: **P1 — L4/e2e post-deploy del ciclo #544** (webapp 4/4 con socket.io-client real, no curl) — encaja con el gate G4 de monitoring y su advertencia de evidencias pre-23:29Z obsoletas.
3. **P1 — CSP enforcing en 2 fases (drift #3, RE-VERIFICADO report-only en el build nuevo):** duplicar header enforcing junto al report-only 48-72h, luego retirar report-only; en paralelo migrar a nonce para matar `unsafe-inline`/`unsafe-eval`. Coherente con WO-09 (diseño pendiente). Gate: security-auditor + Playwright smoke.
4. **P1 — Upgrade WS nativo en la ruta pública (co-firma api-ws P0#2):** nginx :80 (ya activo y con la ruta `/socket.io/` según el propio comentario de `next.config.js:129`) o ingress CF directo a api-server; elimina el polling HTTP del feed (chip "FEED POLLING") y cumple RULE 02. Dependencia: config del túnel CF (token-managed, lado operador).
5. **P2 NUEVA — Destino de `websocket-client.ts` (promover o retirar):** dos clientes WS con contratos divergentes coexisten en `lib/` y ya desviaron a un verificador (C-1). Decisión: retirar la lib muerta (dejar `socket-lifecycle.ts` como único) o promoverla como API canónica y alinearla. WO-01 (diff presente, sin commit) queda subordinado a esta decisión. Gate: arbx-pre-edit-audit + decisión del operador + P-∅ (PR con ID).
6. **P2 — Churn de deploy (co-firma monitoring P0#1 + adición):** coalescing/lock de deploys Y, en la capa edge (superficie edge-gateway), responder `503` honesto con `Retry-After` cuando el upstream está en restart, en lugar del opaco `{"error":"internal_error"}` — hoy cada merge produce ~1-2 min de error visible en el dominio (evidencia 23:44:56Z).
7. **Co-firma data-layer P0 (purge ≥ inserción):** un ENOSPC ~09-12/13 apaga PG y mi superficie entera queda en paneles vacíos/error — la resolución es del operador (cron/env), pero la dependencia es directa para frontend-web.

---

## 5. Nota de honestidad (R8)

- El vínculo imagen-23:44:43Z ↔ commit 9ac06d2d es inferencia fuerte (repo VPS en 9ac06d2d ANTES del build, árbol limpio, card de #544 servido por el contenedor nuevo) — sigue sin existir SHA horneado (esa ES la propuesta 1).
- El `count:0` de 23:37Z no lo pude reproducir ni explicar (C-3/Q-2/Q-3): queda declarado como hueco, no fabricado.
- No navegué con navegador real (presupuesto público agotado 5/5); el comportamiento WS del navegador (polling vs upgrade) se afirma desde código + netns de api-ws, no desde DevTools propios.
- El diff WO-1 preexistía en el árbol compartido; no fue escrito ni commiteado por este verificador.
