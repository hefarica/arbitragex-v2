# SÍNTESIS — Auditoría repo + ciclos reales (2026-09-16)

Programa: real-cycles-audit-20260916 · Orquestador: Hermes (ccr-glm53)
Evidencia: BOARD WO-01..07 + PG (psql RO vía ssh arbx) + browser CDP + git.

## 1. Estado del repo (LOCAL vs MAIN vs VPS)

| Capa | SHA | Estado |
|---|---|---|
| Local (working tree) | `0e72a7cc` branch `fix/567-canonical-plan-consumer` | +5 commits propios / -17 detrás de main. Contiene fix GATE-2 (canonical_plan_consumer.rs, +952 líneas) SIN merge. Uncommitted: settings.json (intocable por directiva CCR), skill arbx-live-engineering + biblioteca (6.5K líneas), .mcp.json, rust-toolchain.toml, submodules OZ. |
| origin/main | `9abfab41` | PR #573 (data-integrity v3 fee manifest, 15 commits) mergeado hoy. |
| VPS /opt/arbitragex-v2 | `9abfab41` main | Deployado hoy 11:40Z, 6 servicios core healthy, imagen searcher 1d8ce6d2 consistente. |

Hallazgos VPS que persisten del mapa 02:25Z: deploy-lock VIVO (21:11Z), disk_guard
ROTO (0644), .env 0644 world-readable, sin backups automatizados, FLASHBOTS_SIGNER_KEY
ausente.

## 2. La cadena de verdad de un "ciclo completo con ganancia real"

Verificado capa por capa contra PostgreSQL del VPS (lectura):

| Capa | Estado | Evidencia |
|---|---|---|
| Detección | VIVA (76 opps/9h post-deploy) pero 100% rejected | v3_quote_unavailable 60/76 |
| Simulación S4 | Consume (74/9h) pero **passed=true = 0 EN TODA LA HISTORIA** | count(*) where passed |
| Paper ledger | 598,878 runs PERO 0 asociados a sim passed — todas REJECTED (R-0001) | join p↔s passed=0 |
| Ejecución real | **executions = 0 filas EN TODA LA HISTORIA** | count(*)=0 |
| Gates G1-G8 | 0/8 con evidencia reproducible; dashboard: Readiness 0/4, NO-GO | artifacts/ sin attestation; A.9 pending |

Los "P&L" de agosto del paper history (+9,669 / +13,798 USD diarios) provienen del
defecto R-0001 (paper executor tradeando oportunidades REJECTED). No son ejecutables
ni representan ciclos completos: son predicciones de rutas que el propio sistema
rechazó. La única corrida post-01-sep predice $459.88 sin observación (actual=NULL).

## 3. Entrega de la dapp (verificación browser 2026-09-16)

https://arbx.ape-tv.net viva y R8-honesta:
- Home: "0 ASIMETRÍAS ACTIVAS", "sin ciclos completos (§44)", capital $0.00,
  GO live NO-GO, 2 GATES RED, Live/Submit/Broadcast OFF, Paper ON.
- /executions: "0 ROWS — No executions yet".
- /paper/history: Runs 0 (24h); filas visibles con reason=non_positive_profit.

La dapp YA muestra el estado real. Lo que NO puede mostrar son ciclos con ganancias
reales porque ESOS DATOS NO EXISTEN en ninguna tabla del sistema. Fabricarlos
violaría RULE 00 (Zero Mocks), R8 (Fail-Honest) y el estándar de evidencia §34.5.3
(precedente GATE-2: evidencia fabricada detectada y rechazada).

## 4. Camino para que existan ciclos reales (orden, sin saltos)

1. **Merge branch 567** (canonical_plan_consumer) — el fix GATE-2 está escrito,
   testeado en la branch, sin integrar. Requiere: rebase sobre 9abfab41 + CI + PR.
2. **Cerrar v3_quote_unavailable** (60/76 rechazos actuales) — sin quote V3 no hay
   candidato evaluable. Investigar por qué el quote V3 falla para los pares activos
   (a0b869…/c02aaa…, c02aaa…/dac17f…).
3. **Primera simulación passed=true de la historia** → paper run legítimo (no-REJECTED)
   → el ledger vuelve a significar algo.
4. **G1**: manifest V3 firmado por 2 humanos + backup con restore-test (hoy: 0 firmas,
   0 restore-test; disk_guard roto no bloquea esto pero es el mismo paquete de higiene).
5. **G4→G6**: submit engine exercised (idempotency), fork-replay 10 blocks <1%,
   Sepolia live detect→reconcile.
6. **G7 (A.9 2 firmas) → G8 canary ≤$350 / 5 WETH** — bajo §34.5, si TODO lo anterior
   pasa con artefactos reproducibles, el flip procede sin nueva ceremonia.

## 5. Veredicto final

- Auditoría: COMPLETA (7/7 WOs, evidencia reproducible en este directorio y queries PG).
- "Dapp mostrando ciclos completos con ganancias reales": **NO ENTREGABLE HOY** —
  no por límite del agente sino porque los datos no existen y fabricarlos está
  prohibido por doctrina del propio repo. La dapp entregada muestra la verdad: vacío
  honesto + NO-GO + el camino de gates.
- Escalado operador: pasos 1-2 son trabajo de ingeniería autorizado (merge + fix quote)
  que puedo ejecutar a pedido; los pasos 4-7 tienen componentes operador-only
  (firmas físicas A.9/manifest, flip de red) ya cubiertos por §34.5.
