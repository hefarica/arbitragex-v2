# FASE 1 — GATE1: merges, deploy y PUERTA DE FASE

Fecha: 2026-10-22 · Autor: integrador-2 (t11, attempt 20325c32) · Repo: hefarica/arbitragex-v2 · Rol: git/deploy integrator

## 0. Resumen en una línea

Las cinco PRs P0 + el docs PR están **mergeadas y desplegadas** en `af331ce7` (deploy run 39783612719, sha verificado en el endpoint de status) y el control de PR1 **se manifiesta en producción** (3 filas con `dex_b` no nulo post-deploy vs 0 en la ventana previa de 2 h). **La puerta `status='executed'` NO pasa: 0 filas.** La causa es estructural, no de despliegue: el único escritor de `executed` es el relayer tras una inclusión on-chain (`backend/relays-client/src/persistence.rs:47-54`), y la producción corre en `paper_mode=true` (`ARBX_TRADE_MODE=paper`, `ARBX_LIVE_EXEC_ENABLED=False`, Redis `arbx:papermode:1`), que no difunde transacción. No se forzó la puerta ni se tocaron rutas protegidas (G5).

## 1. Merges secuenciales (squash) — cola verificada por el capitán

Base `f27bdb33de026c0f6fa5edcbc1b94438b20af663` → HEAD final de la fase `af331ce7fbeaee2e6c9cf25760d927d098d7b1d4`.

| # | PR | Rama | Head verificado | Merge commit en main | Estado |
|---|---|---|---|---|---|
| 1 | #1013 POBLACION-FILTER-01 | feat/poblacion-filter-01 | `78ab15ee` | `d2863d95` | MERGED (squash) |
| 2 | #1014 ATOMIC-CAUSE-01 | feat/atomic-cause-01 | `72aca3d5` | `9ecefc75` | MERGED (squash) |
| 3 | #1015 TWO-LEG-01 | feat/two-leg-01 | `d975eb30` | `b5809f08` | MERGED (squash) |
| 4 | #1016 REVERT-TRACE-01 | feat/revert-trace-01 | `098d1c3a` | `d71c9fa5` | MERGED (squash) |
| 5 | #1017 OPTIMAL-SIZE-01 | feat/optimal-size-01 | `e8990fd8` | `0a0b1388` | MERGED (squash) |
| 6 | #1018 docs/rea-base (docs-only, 10 archivos) | docs/rea-base | `5cb3ef2c` | `af331ce7` | MERGED (squash) |

Disciplina aplicada (skill `arbx-ci-cd-prolijo`):

- Branch protection de `main`: required contexts = `ci-gate`, `Verifier policy tests`; `strict=false`; `enforce_admins=true`.
- Checks requeridos verdes en las 6 PRs antes de cada merge (`gh pr checks <n>`: 0 fail; `ci-gate` = job agregador de `ci.yml` que `needs:` lint-and-test-{node,rust,frontend,contracts}, fixture-metadata-gate, retention-vacuum-regression, liquidity-catalog-contract — se esperó su veredicto, ~14 min en #1013).
- `gh pr update-branch` solo cuando hizo falta: #1013 (único actualizado a mano, head pasó a `c2e8c390`; espera de 245 s ≥ 4 min antes de pollear, regla 8). Las otras 5 ya estaban `CLEAN/MERGEABLE` con checks válidos sobre su propio head; no se refrescó nada que no lo necesitara.
- Rama remota borrada al confirmar MERGED en los 6 casos (`gh api -X DELETE …/git/refs/heads/<branch>`, exit 0).
- Worktree local limpiado al cerrar cada ciclo (`git worktree remove --force` + `git worktree prune`): poblacion-filter-01, atomic-cause-01, two-leg-01, revert-trace-01, optimal-size-01, docs-rea-base. `git worktree list` final no muestra residuos de la fase.
- Rutas protegidas intactas en TODO el rango:
  `git diff --name-only f27bdb33…af331ce7 -- configs/app.toml backend/relays-client/src/live_exec_policy.rs backend/relays-client/src/bundle_builder.rs` → **salida vacía**. `paper_mode = true` sigue en `configs/app.toml` (main).

## 2. Deploy y verificación de identidad (un merge que no deploya no es cierre)

- Cada push a main dispara `auto-deploy-vps.yml` (concurrency `production-vps`, `cancel-in-progress: false`). Los deploys de los SHAs intermedios quedan **superseded por diseño**: `scripts/ci/wait_deploy_gates.py` aborta con "Target superseded by main; deploy the newer validated commit" apenas main avanza. Esa es la razón de los runs `failure`/`cancelled` intermedios, no un fallo de código.
- **Deploy válido del SHA final**: run **39783612719** — `Wait for all deployment gates` success + `Deploy to VPS` success (completed 2026-10-22T13:59:50Z).
- Gates del SHA final `af331ce7`: los 17 workflows push/main del manifest `scripts/ci/deploy-gates.json` + path-filtered, todos `completed`, **0 failure**.
- Verificación en el endpoint de status del VPS (`http://127.0.0.1:8080/status`, leído por ssh):
  `{"sha": "af331ce7fbeaee2e6c9cf25760d927d098d7b1d4", "id": "39783612719", "at": "2026-10-22T13:32:02Z"}`
  → **sha del deploy == sha mergeado**, e id == run id del deploy.
- Sellado en contenedor: `ARBX_DEPLOY_SHA=af331ce7fbeaee2e6c9cf25760d927d098d7b1d4` (api-server). Imágenes recreadas 13:58:40Z; searcher-rs / relays-client / api-server / sim-ctl `healthy`.

## 3. PUERTA DE FASE — SQL real sobre la PG del VPS

Acceso: `ssh arbx` → `docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -F '|'`.

**Nota fail-honest sobre la herramienta**: `sql_query` del harness devuelve `{"ok":true,"data":"","rows_affected":0}` incluso para SQL inválido (`SELECT * FROM tabla_que_no_existe_xyz;`) — **no ejecuta**. Todas las cifras de esta sección salen de psql real sobre la PG viva del VPS.

### 3.1 Baseline PRE-cambio (2026-10-22 12:40:22Z, antes del primer merge)

```
medido_en|2026-10-22 12:40:22.630508+00
executed_total|0
total_rows|1531507
rows_2h|187
dex_b_notnull_2h|0
rows_6h|437
dex_b_notnull_6h|0
status|rejected|1531507
```

### 3.2 Post-deploy (deploy completado 13:59:50Z; medición final 14:43:29Z = 43.6 min de ventana)

```
ahora|2026-10-22 14:43:27.115354+00
deploy_ref|2026-10-22T13:59:50Z
executed_total|0
post_deploy_total|13|3
pre_deploy_2h|62|0
status_3h|rejected|134
paper_runs_2h|0
total_rows|1531534
```

(columnas de `post_deploy_total`/`pre_deploy_2h`: filas | filas con `dex_b` no nulo)

Razones de rechazo de las 13 filas post-deploy (todas del gate de allowlist de tokens, ninguna llega a simulación):

```
TokenNotAllowed:0x7adbfd873084db714a57ed648ee69aa4627cb998|7
TokenNotAllowed:0x9ac9468e7e3e1d194080827226b45d0b892c77fd|2
TokenNotAllowed:0x95af4af910c28e8ece4512bfe46f1f33687424ce|2
TokenNotAllowed:0x54ce4b27caabba24926865f035d432dbde2901f7|1
TokenNotAllowed:0x69af81e73a73b40adf4f3d4223cd9b1ece623074|1
```

Contexto 24 h: `status` único = `rejected` (1845 filas); `dex_b` no nulo en 24 h = **3** (exactamente las 3 post-deploy; antes del deploy no había ninguna). `paper_trade_runs` = **0 filas en toda la tabla** (el paper executor nunca ejecutó).

### 3.3 Veredicto de la puerta

| Criterio | Resultado | Evidencia |
|---|---|---|
| (a) `SELECT COUNT(*) FROM opportunities WHERE status='executed'` ≥ 1 | **NO PASA — 0 filas** | psql real §3.2; 0 también en baseline §3.1 |
| (b) control PR1: `dex_b` no nulo en ventana post-deploy > 0, ventana previa 2 h = 0 | **PASA — 3 vs 0** | §3.2 y §4 |

**Por qué (a) no puede pasar hoy (causa estructural, con artefacto)**:

1. El único escritor de `status='executed'` es el relayer, y solo ante inclusión on-chain: `backend/relays-client/src/persistence.rs:47-54` (`ExecutionStatus::Included => "executed"`; `NotSubmitted|Submitted|NotImplemented => "simulated"`). El UPDATE además exige `WHERE status IN ('simulated','executing')` (línea 59).
2. `ExecutionStatus::Included` se construye en un solo lugar y solo después de difundir y esperar inclusión: `backend/relays-client/src/submit_engine.rs:854-883` (`wait_for_inclusion` post-broadcast).
3. La producción corre en paper: `ARBX_TRADE_MODE=paper`, `ARBX_LIVE_EXEC_ENABLED=False`, Redis `arbx:papermode:1`; sin difusión no hay inclusión y por lo tanto no hay `executed`. El camino paper no escribe `opportunities.status`: persiste en `paper_trade_runs` (`backend/api-server/src/paper/executor.ts:351-371`), tabla hoy vacía.
4. Aun si hubiera difusión, hoy **ninguna** oportunidad supera los gates previos: 100 % de las 13 filas post-deploy mueren en `TokenNotAllowed:<token>` (allowlist de tokens); heartbeat del searcher: `passed_all_gates: 0`.

**Consecuencia honesta**: la puerta definida (`opportunities.status='executed'` ≥ 1) y la restricción dura `paper_mode=true` son incompatibles: cumplirla exige o bien una inclusión on-chain real (modo live + capital, decisión del operador) o bien un cambio de semántica que marque `executed` en paper (tocaría el camino de ejecución y contradeciría el significado del estado). Ambas cosas están **fuera del alcance de t11** y la primera cae en las rutas protegidas (G5). **No se forzó**: no se tocó `configs/app.toml`, `live_exec_policy.rs` ni `bundle_builder.rs`.

## 4. PR1 (TWO-LEG-01) — manifestación medida en producción

El control de la PR1 se cumple y se ve en dos planos independientes:

**(a) SQL** — 3 filas con `dex_b` no nulo en la ventana post-deploy, contra 0 en la ventana previa exacta de 2 h (62 filas) y 0 en todo el historial previo:

```
dex_b_rows|uniswap-v2|uniswap-v3|2026-10-22 14:08:53.110057+00
dex_b_rows|uniswap-v2|uniswap-v3|2026-10-22 14:03:01.704582+00
```

**(b) Log del productor vivo** (searcher-rs desplegado, `docker logs arbitragex-v2-searcher-rs-1`):

```
{"timestamp":"2026-10-22T14:03:01.839094Z", … ,"message":"crossed pair persisted as both legs (TWO-LEG-01)","event":"scanner.crossed_pair_identified","pair":"WETH-Yee","buy_pool":"0xbc7ce7…","sell_pool":"0xb21462…","dex_a":"uniswap-v2","dex_b":"Some(\"uniswap-v3\")"}
{"timestamp":"2026-10-22T14:08:53.326264Z", … ,"pair":"MASK-USDC","dex_a":"uniswap-v2","dex_b":"Some(\"uniswap-v3\")"}
```

Es decir: el emisor persiste la segunda pata (venue del pool de venta) y `pool_addresses[1]`, sobre el productor vivo, con el código desplegado de `af331ce7`. Las filas quedan `rejected` por el gate de allowlist de tokens (hecho separado, preexistente), pero la columna que la PR1 debía poblar **ya no es NULL**.

## 5. Huecos declarados (no computado ≠ cero)

1. **Ventana post-deploy de 2 h incompleta**: al cierre de esta medición pasaron 43.6 min desde el deploy. Los números reportados son de esa ventana; completar las 2 h solo puede sumar filas al control (b), no cambiar (a), que es estructuralmente 0. Comando para re-medir: `arbx_sql.sh` con `SELECT count(*) FILTER (WHERE dex_b IS NOT NULL) FROM opportunities WHERE detected_at >= '2026-10-22T13:59:50Z';`.
2. **`sql_query` del harness no ejecuta** (devuelve ok con data vacía incluso para SQL inválido): toda la evidencia SQL se tomó con psql real vía ssh.
3. **Deploys intermedios superseded**: los runs de `d2863d95`, `9ecefc75` fallan y los de `b5809f08`, `d71c9fa5` quedan cancelados por el propio guard de supersesión; no indican regresión (main siguió avanzando hasta el SHA final, que sí deployó y se verificó).
4. **Costo autoinfligido**: en #1013 ejecuté `update-branch` antes de notar que el PR ya estaba `CLEAN`; reinició los required checks y agregó ~14 min de espera. Sin impacto en el resultado; las otras 5 PRs se mergearon sin refresco innecesario.
5. La fila `status='executed'` no existe en `executions` (CHECK sin ese valor): la puerta se midió donde corresponde, `opportunities`.

## 6. Reproducibilidad (comandos literales)

```bash
# merges
gh pr checks <n> -R hefarica/arbitragex-v2
gh pr merge <n> -R hefarica/arbitragex-v2 --squash
gh api -X DELETE repos/hefarica/arbitragex-v2/git/refs/heads/<branch>
git worktree remove --force <path> && git worktree prune

# deploy
gh api repos/hefarica/arbitragex-v2/actions/runs/39783612719
ssh arbx 'curl -s http://127.0.0.1:8080/status'   # marker deploy.sha / deploy.id

# puerta
ssh arbx "docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -F '|'" <<'SQL'
SELECT 'executed_total', count(*) FROM opportunities WHERE status='executed';
SELECT 'post_deploy_total', count(*), count(*) FILTER (WHERE dex_b IS NOT NULL)
  FROM opportunities WHERE detected_at >= '2026-10-22T13:59:50Z';
SELECT 'pre_deploy_2h', count(*), count(*) FILTER (WHERE dex_b IS NOT NULL)
  FROM opportunities WHERE detected_at >= '2026-10-22T11:59:50Z' AND detected_at < '2026-10-22T13:59:50Z';
SQL
```
