# FASE 1 — GATE1: merges, deploy y PUERTA DE FASE

Fecha: 2026-10-22/23 · Autor: integrador-2 (t11; attempts 20325c32 → 6eed3575 tras fallo de transporte) · Repo: hefarica/arbitragex-v2

## 0. Resumen en una línea

Los cinco P0 + el docs PR están **mergeados y desplegados**: el deploy del SHA de código `af331ce7` (run 39783612719) terminó `success` y el endpoint de status del VPS devolvió `deploy.sha=af331ce7…` + `deploy.id=39783612719`; el control de PR1 **se manifiesta en producción** (93 filas con `dex_b` no nulo sobre 1514 en la ventana post-deploy, contra 0/62 en la ventana previa de 2 h); **la puerta `status='executed'` NO pasa: 0 filas tras 15.5 h de ventana**. La causa es estructural: el único escritor de `executed` es el relayer tras una inclusión on-chain (`backend/relays-client/src/persistence.rs:47-54`) y producción corre en paper (`ARBX_TRADE_MODE=paper`, `ARBX_LIVE_EXEC_ENABLED=False`, Redis `arbx:papermode:1`), que no difunde transacción. No se forzó la puerta ni se tocaron rutas protegidas (G5).

## 1. Merges secuenciales (squash) — cola verificada por el capitán

Base `f27bdb33de026c0f6fa5edcbc1b94438b20af663` → HEAD del deploy de código `af331ce7fbeaee2e6c9cf25760d927d098d7b1d4`; main al cierre de este informe: `80acfc9d` (REA_MASTER v2, #1026, mergeado por el capitán después).

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
- Checks requeridos verdes en las 6 PRs antes de cada merge (`gh pr checks <n>`: 0 fail; `ci-gate` es el job agregador de `ci.yml` que `needs:` lint-and-test-{node,rust,frontend,contracts}, fixture-metadata-gate, retention-vacuum-regression, liquidity-catalog-contract — se esperó su veredicto, ~14 min en #1013).
- `gh pr update-branch` solo cuando hizo falta: #1013 (único actualizado a mano en la secuencia de código; head pasó a `c2e8c390`, espera de 245 s ≥ 4 min antes de pollear, regla 8). Las otras 5 ya estaban `CLEAN/MERGEABLE` con checks válidos sobre su propio head; no se refrescó nada que no lo necesitara.
- Rama remota borrada al confirmar MERGED en los 6 casos (`gh api -X DELETE …/git/refs/heads/<branch>`, exit 0).
- Worktree local limpiado al cerrar cada ciclo (`git worktree remove --force` + `git worktree prune`): poblacion-filter-01, atomic-cause-01, two-leg-01, revert-trace-01, optimal-size-01, docs-rea-base.
- Rutas protegidas intactas en TODO el rango:
  `git diff --name-only f27bdb33…af331ce7 -- configs/app.toml backend/relays-client/src/live_exec_policy.rs backend/relays-client/src/bundle_builder.rs` → **salida vacía**. `paper_mode = true` sigue en `configs/app.toml` (main).

## 2. Deploy y verificación de identidad (un merge que no deploya no es cierre)

- Cada push a main dispara `auto-deploy-vps.yml` (concurrency `production-vps`, `cancel-in-progress: false`). Los deploys de los SHAs intermedios quedan **superseded por diseño**: `scripts/ci/wait_deploy_gates.py` aborta con "Target superseded by main; deploy the newer validated commit" apenas main avanza. Esa es la razón de los runs `failure`/`cancelled` intermedios, no un fallo de código.
- **Deploy válido del SHA de código**: run **39783612719** — `Wait for all deployment gates` success + `Deploy to VPS` success (completed 2026-10-22T13:59:50Z).
- Gates del SHA `af331ce7`: los workflows push/main del manifest `scripts/ci/deploy-gates.json` + path-filtered, todos `completed`, **0 failure** (17/17).
- Verificación en el endpoint de status del VPS (`http://127.0.0.1:8080/status`, leído por ssh):
  `{"sha": "af331ce7fbeaee2e6c9cf25760d927d098d7b1d4", "id": "39783612719", "at": "2026-10-22T13:32:02Z"}`
  → **sha del deploy == sha mergeado**, e id == run id del deploy.
- Sellado en contenedor: `ARBX_DEPLOY_SHA=af331ce7fbeaee2e6c9cf25760d927d098d7b1d4` (api-server). Imágenes recreadas 13:58:40Z; searcher-rs / relays-client / api-server / sim-ctl `healthy`.
- Al cierre de esta ventana, main avanzó a `80acfc9d` (REA_MASTER v2, #1026, docs-only) con su propio deploy run **39905803540** en curso; el sha servido seguía siendo `af331ce7` en la última lectura del endpoint (el nuevo deploy lo reemplaza al terminar). El presente informe se entrega en un PR docs-only posterior, sujeto a su propio ciclo de deploy.

## 3. PUERTA DE FASE — SQL real sobre la PG del VPS

Acceso: ssh al VPS → `docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -F '|'`.

**Nota fail-honest sobre la herramienta**: `sql_query` del harness devuelve `{"ok":true,"data":"","rows_affected":0}` incluso para SQL inválido (`SELECT * FROM tabla_que_no_existe_xyz;`) — **no ejecuta**. Todas las cifras de esta sección salen de psql real sobre la PG viva.

**Nota de transporte (para reproducir)**: en este host `ssh` de Windows falla (`ssh -o BatchMode=yes arbx 'echo SSH_OK'` → exit 255, sin salida; el binario no llega a escribir ni el log con `-E`). La vía que funciona es WSL: copiar la clave a `~/.ssh` con `chmod 600` (sobre `/mnt/c` da 0777 y ssh rechaza la clave) y ejecutar desde `bash.exe`:
`ssh -i ~/.ssh/arbx_hetzner -o BatchMode=yes root@195.201.235.70 'docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -F "|"' < query.sql`.

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

### 3.2 Post-deploy — ventana completa (deploy 2026-10-22T13:59:50Z → medición 2026-10-23 05:28:13Z = 15.5 h)

```
ahora|2026-10-23 05:28:13.740074+00
executed_total|0
post_deploy_total|1514|93
status_post|rejected|1514
pre_deploy_2h|62|0
rango_retenido|2026-10-11 04:27:10.210808+00|2026-10-23 05:27:42.631332+00|515188
paper_trade_runs_total|0
```

(columnas de `post_deploy_total`/`pre_deploy_2h`: filas | filas con `dex_b` no nulo)

`dex_b` no nulo por hora dentro de la ventana post-deploy (continuo, no un pico aislado):

```
2026-10-22 14:00|5    2026-10-22 15:00|15   2026-10-22 16:00|6    2026-10-22 17:00|1
2026-10-22 18:00|10   2026-10-22 19:00|37   2026-10-22 20:00|6    2026-10-22 21:00|2
2026-10-23 00:00|2    2026-10-23 01:00|1    2026-10-23 02:00|4    2026-10-23 03:00|3
2026-10-23 04:00|1
```

Razones de rechazo de las filas post-deploy (muestra de la ventana temprana; el patrón se mantuvo — 0 filas llegan a simulación):

```
TokenNotAllowed:0x7adbfd873084db714a57ed648ee69aa4627cb998|7
TokenNotAllowed:0x9ac9468e7e3e1d194080827226b45d0b892c77fd|2
TokenNotAllowed:0x95af4af910c28e8ece4512bfe46f1f33687424ce|2
TokenNotAllowed:0x54ce4b27caabba24926865f035d432dbde2901f7|1
TokenNotAllowed:0x69af81e73a73b40adf4f3d4223cd9b1ece623074|1
```

Completitud de la ventana: `rango_retenido` arranca 2026-10-11 (la retención no truncó la ventana); `paper_trade_runs` = **0 filas en toda la tabla** (el paper executor nunca ejecutó).

### 3.3 Veredicto de la puerta

| Criterio | Resultado | Evidencia |
|---|---|---|
| (a) `SELECT COUNT(*) FROM opportunities WHERE status='executed'` ≥ 1 | **NO PASA — 0 filas** (15.5 h de ventana, 1514 filas nuevas, todas `rejected`) | §3.2; 0 también en baseline §3.1 |
| (b) control PR1: `dex_b` no nulo post-deploy > 0; ventana previa 2 h = 0 | **PASA — 93 vs 0** | §3.2 y §4 |

**Por qué (a) no puede pasar hoy (causa estructural, con artefacto)**:

1. El único escritor de `status='executed'` es el relayer, y solo ante inclusión on-chain: `backend/relays-client/src/persistence.rs:47-54` (`ExecutionStatus::Included => "executed"`; `NotSubmitted|Submitted|NotImplemented => "simulated"`). El UPDATE además exige `WHERE status IN ('simulated','executing')` (línea 59).
2. `ExecutionStatus::Included` se construye en un solo lugar y solo después de difundir y esperar inclusión: `backend/relays-client/src/submit_engine.rs:854-883` (`wait_for_inclusion` post-broadcast).
3. La producción corre en paper: `ARBX_TRADE_MODE=paper`, `ARBX_LIVE_EXEC_ENABLED=False`, Redis `arbx:papermode:1`; sin difusión no hay inclusión y por lo tanto no hay `executed`. El camino paper no escribe `opportunities.status`: persiste en `paper_trade_runs` (`backend/api-server/src/paper/executor.ts:351-371`), tabla hoy vacía.
4. Aun si hubiera difusión, hoy **ninguna** oportunidad supera los gates previos: 100 % de las filas post-deploy mueren en `TokenNotAllowed:<token>` (allowlist de tokens); heartbeat del searcher: `passed_all_gates: 0`.

**Consecuencia honesta**: la puerta definida (`opportunities.status='executed'` ≥ 1) y la restricción dura `paper_mode=true` son incompatibles: cumplirla exige o bien una inclusión on-chain real (modo live + capital, decisión del operador) o bien un cambio de semántica que marque `executed` en paper (tocaría el camino de ejecución y contradeciría el significado del estado). Ambas cosas están **fuera del alcance de t11** y la primera cae en las rutas protegidas (G5). **No se forzó**: no se tocó `configs/app.toml`, `live_exec_policy.rs` ni `bundle_builder.rs`.

## 4. PR1 (TWO-LEG-01) — manifestación medida en producción

**(a) SQL** — 93 filas con `dex_b` no nulo en la ventana post-deploy (6.1 % de 1514) contra 0 en la ventana previa exacta de 2 h (62 filas) y 0 en todo el historial previo:

```
dex_b_rows|uniswap-v2|uniswap-v3|2026-10-22 14:08:53.110057+00
dex_b_rows|uniswap-v2|uniswap-v3|2026-10-22 14:03:01.704582+00
```

**(b) Log del productor vivo** (searcher-rs desplegado, `docker logs arbitragex-v2-searcher-rs-1`):

```
{"timestamp":"2026-10-22T14:03:01.839094Z", … ,"message":"crossed pair persisted as both legs (TWO-LEG-01)","event":"scanner.crossed_pair_identified","pair":"WETH-Yee","buy_pool":"0xbc7ce7…","sell_pool":"0xb21462…","dex_a":"uniswap-v2","dex_b":"Some(\"uniswap-v3\")"}
{"timestamp":"2026-10-22T14:08:53.326264Z", … ,"pair":"MASK-USDC","dex_a":"uniswap-v2","dex_b":"Some(\"uniswap-v3\")"}
```

Es decir: el emisor persiste la segunda pata (venue del pool de venta) sobre el productor vivo, con el código desplegado de `af331ce7`, de forma sostenida (13 horas con actividad). Las filas quedan `rejected` por el gate de allowlist de tokens (hecho separado, preexistente), pero la columna que la PR1 debía poblar **ya no es NULL**.

## 5. Huecos declarados (no computado ≠ cero)

1. **La puerta (a) no se cumple**: 0 filas `executed` en 15.5 h. No es un hueco de medición — es el resultado medido. La causa está en §3.3 con artefacto de código y estado vivo.
2. **`sql_query` del harness no ejecuta** (devuelve ok con data vacía incluso para SQL inválido): toda la evidencia SQL se tomó con psql real vía ssh.
3. **ssh de Windows no funciona en este host** (exit 255 sin salida); la vía reproducible es WSL (`bash.exe`) con la clave en `~/.ssh` y `chmod 600` (§3, nota de transporte).
4. **Deploys intermedios superseded**: los runs de `d2863d95`, `9ecefc75` fallan y los de `b5809f08`, `d71c9fa5` quedan cancelados por el guard de supersesión; no indican regresión (main siguió avanzando hasta el SHA final, que sí deployó y se verificó).
5. **Costo autoinfligido**: en #1013 ejecuté `update-branch` antes de notar que el PR ya estaba `CLEAN`; reinició los required checks y agregó ~14 min de espera. Sin impacto en el resultado.
6. La fila `status='executed'` no existe en `executions` (CHECK sin ese valor): la puerta se midió donde corresponde, `opportunities`.
7. El informe se entrega en un PR docs-only posterior a la ventana; su propio merge genera un deploy docs-only nuevo (verificado en su ciclo), sin efecto en las mediciones.

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

# puerta (vía WSL; ver §3 nota de transporte)
ssh arbx "docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -F '|'" <<'SQL'
SELECT 'executed_total', count(*) FROM opportunities WHERE status='executed';
SELECT 'post_deploy_total', count(*), count(*) FILTER (WHERE dex_b IS NOT NULL)
  FROM opportunities WHERE detected_at >= '2026-10-22T13:59:50Z';
SELECT 'pre_deploy_2h', count(*), count(*) FILTER (WHERE dex_b IS NOT NULL)
  FROM opportunities WHERE detected_at >= '2026-10-22T11:59:50Z' AND detected_at < '2026-10-22T13:59:50Z';
SQL
```
