---
name: arbx-live-engineering
description: Ingeniería integral de ArbitrageX para testnet live y mainnet live, evolución de .claude, agregación de blockchains/DEX/pools, mapeo VPS read-only antes de cambios y pruebas reales mediante Desktop Commander y herramientas verificadas.
# disable-model-invocation retirado por orden explícita del operador (hefarica, 2026-09-17):
# "ese puto .claude restringido me tiene mamado, arregla eso". La máquina de estados §4 y
# todos los gates internos permanecen INTACTOS — esto solo habilita la invocación por el modelo.
argument-hint: "[mapear | implementar | probar | preparar-testnet | preparar-mainnet] [alcance]"
---

# ArbitrageX — Ingeniería live con evidencia

## 1. Mandato y objetivo de salida

Construye, integra y verifica la DApp existente para soportar **testnet live y mainnet live**.
No sustituyas ese objetivo por un producto permanentemente paper-only, una maqueta,
un segundo frontend ni un informe sin implementación cuando el trabajo está autorizado.

Cada obstáculo técnico se convierte en reproducción, causa, corrección y nueva prueba.
Persiste mientras exista un siguiente paso autorizado y útil; no repitas intentos idénticos
sin nueva evidencia. Un bloqueo de acceso o una condición insegura limita esa acción,
no elimina el objetivo de ingeniería ni impide avanzar en los trabajos independientes.

Esta skill define un encargo y un procedimiento. No crea credenciales, no concede
permisos del sistema operativo, no sustituye una conexión MCP/SSH y no prevalece sobre
instrucciones superiores ni permisos administrados. No simules facultades inexistentes.
La firma, el envío de transacciones con valor y la habilitación de un bot que opere
fondos reales quedan en manos del operador. Entrega el software y el flujo de activación
para que el operador pueda utilizarlos; no impongas una prohibición permanente a mainnet.

Argumentos del encargo actual: $ARGUMENTS

## 2. Contexto de partida que debes verificar

- Repositorio esperado: `hefarica/arbitragex-v2`.
- Alias SSH esperado: `arbx`; ruta remota esperada: `/opt/arbitragex-v2`.
- Integración previa: PR #560. Consulta su estado y SHA actuales; no fijes un HEAD histórico.
- Arquitectura de referencia: Rust, API TypeScript, Edge, Next.js, PostgreSQL y Redis.
- Los informes de septiembre de 2026 son antecedentes, nunca telemetría actual.

Lee [procedencia y fuentes](references/procedencia-y-fuentes.md) al usar estos antecedentes.
Valida el dispositivo, el repositorio y el destino antes de cualquier acción.

## 3. Facultades de ingeniería sobre el proyecto

Dentro del repositorio identificado y los permisos disponibles puedes leer, crear,
corregir y refactorizar código, contratos, pruebas, configuración de desarrollo,
migraciones, contenedores, CI/CD y documentación relacionados con el objetivo.
Puedes modificar `CLAUDE.md`, `.claude/CLAUDE.md`, `.claude/rules/`, `.claude/agents/`,
`.claude/commands/` y `.claude/skills/` para mantener instrucciones coherentes.

Para `.claude/settings.json`, hooks y configuración MCP del proyecto: inspecciona
primero, conserva las entradas ajenas y separa un cambio funcional de un cambio de
permisos. Obtener permisos adicionales requiere el mecanismo real del cliente y la
aprobación correspondiente; no edites las barreras para autoautorizarte.

La política funcional del producto puede evolucionar de “solo pruebas/paper” a
“testnet/mainnet soportadas con activación explícita”, con diff y pruebas de regresión.
Documenta cuál decisión del operador resuelve la contradicción; no borres la historia.

No vacíes `.claude`, no reemplaces instrucciones enteras por una declaración de poder,
no desactives confirmaciones globales ni controles administrados, no retires pruebas
para producir un verde y no alteres esta skill o su validador para fingir cumplimiento.
No toques el `.claude` global del usuario ni otros proyectos por extensión del encargo.

Preserva cambios locales, incluidos untracked. Antes de sobrescribir, crea una copia
local recuperable o commit acotado; evita incluir secretos en esa copia o en Git.

## 4. Máquina de estados obligatoria

| Estado | Trabajo permitido | Evidencia para avanzar |
|---|---|---|
| ACCESS_CHECK | Identificar herramientas, máquina, repo y destino | Acceso autenticado real, identidad y alcance |
| VPS_READ_ONLY | Inventariar y consultar estado sin mutaciones intencionales | Mapa completo del alcance operativo |
| VPS_MAPPED | Formular plan de estabilización, riesgos y reversión | Mapa revisado, acciones y autorización aplicable |
| CHANGE_AUTHORIZED | Implementar únicamente el plan acotado | Verificaciones por acción y rollback disponible |
| ENGINEERING_VERIFIED | Integración, builds, pruebas y despliegue técnico | Evidencia del SHA y entorno realmente probados |
| TESTNET_LIVE_VERIFIED | Pruebas reales en red de pruebas sin valor económico | Identidad de red, recibos y trazabilidad |
| MAINNET_RELEASE_READY | Paquete mainnet y flujo de activación para el operador | Capacidad técnica comprobada, límites y pendientes explícitos |

`MAINNET_RELEASE_READY` no equivale a `MAINNET_ACTIVE`. No presentes un estado futuro
como observado. La activación financiera del operador se registra solamente si existe
evidencia real, sin ejecutarla por delegación automática del agente.

El código local y las pruebas aisladas pueden avanzar mientras se completa el mapa.
Eso no autoriza escribir en el VPS ni tratar un entorno de producción como sandbox.

## 5. Acceso mediante Desktop Commander

Consulta [acceso y Desktop Commander](references/acceso-y-desktop-commander.md).
El operador inicia el dispositivo remoto en el equipo que ya tiene SSH autorizado:

```bash
npx @wonderwhy-er/desktop-commander@latest remote
```

Completa la autenticación interactiva del proveedor, conecta el cliente MCP y descubre
las herramientas realmente disponibles. Registra la versión resuelta; fija esa versión
para repetir pruebas. `latest` es bootstrap solicitado, no una versión reproducible.

No ejecutes este bootstrap en el VPS durante `VPS_READ_ONLY`. No inventes una sesión
SSH porque exista el nombre `arbx`. No solicites claves privadas en el chat.

Los permisos de carpetas de Desktop Commander no constituyen una jaula para su
terminal. Una cuenta SSH restringida o un mecanismo de lectura ya provisionado por
el administrador puede imponer la frontera real. La skill no lo instala durante el mapa.

## 6. VPS: lectura primero, sin excepciones tácitas

Hasta cerrar el mapa solo se permiten consultas identificadas y acotadas. Quedan fuera:
instalar software, escribir temporales remotos, `touch`, cambiar permisos, limpiar,
reiniciar, recargar servicios, alterar Redis/SQL, migrar, hacer pull/fetch remoto,
construir imágenes, desplegar o modificar cron, túneles, firewall, SSH y credenciales.
No ejecutes scripts desconocidos “para ver qué hacen”.

Los artefactos del diagnóstico se guardan en la estación de trabajo autorizada,
fuera de datos sensibles y sin escribir archivos en el VPS. Filtra campos en origen:
**un secreto no debe llegar a la salida de la herramienta para redactarlo después**.

Lectura significa ausencia de mutaciones intencionales de aplicación/configuración.
SSH, HTTP y el sistema pueden registrar accesos o actualizar contadores/atime;
no prometas “cero bits cambiados”. Un endpoint GET o un comando llamado `--check`
no es automáticamente inocuo: comprueba su implementación antes de usarlo.

Procedimiento y dominios: [VPS read-only y mapa](references/vps-readonly-y-mapa.md).

## 7. Qué significa mapear el 100%

Significa cubrir el **100% del inventario operativo definido**, con denominador,
activos enumerados, dependencias y evidencias actuales. No significa leer secretos,
cada byte del disco ni garantizar que no existe un activo desconocido.

Enumera los 12 dominios del manifiesto; concilia sus fuentes de inventario, termina
la paginación y amplía la lista cada vez que aparezca un activo nuevo. No marques
completa una salida truncada. Toda ausencia necesita evidencia, no una suposición.

Un activo averiado puede estar identificado, pero un catálogo inaccesible sigue
siendo un hueco. Si PostgreSQL no permite consultar metadatos requeridos, informa
`BLOCKED` y no certifiques el mapa completo. No lo conviertas en “no aplica”.

Antes del mapa completo informa hallazgos y evidencia faltante; **no propongas ni
implementes un plan de modificación del VPS**. No reduzcas unilateralmente el alcance
para habilitar escrituras. Si avanzar requiere una decisión del operador, delimita
esa decisión y continúa con el trabajo independiente.

Usa `templates/MAPA_VPS.template.json`. El script local `scripts/validate_map.py`
comprueba estructura, cobertura declarada, referencias, antigüedad y hashes de archivos.
Su resultado **no demuestra por sí solo exhaustividad ni autenticidad de los hechos,
no concede permisos y no habilita escrituras ni live**. Revisa la evidencia real.

## 8. Estabilización e implementación después del mapa

Elabora `templates/PLAN_CAMBIO.template.md`: causa comprobada o hipótesis, activo,
acción exacta, prerequisitos, alcance de escritura, impacto, evidencia de backup,
reversión, verificación, presupuesto y criterio de interrupción por acción.

Ejecuta de forma autónoma las acciones reversibles ya autorizadas dentro de ese plan,
sin pedir otra vez lo mismo. Para nuevo alcance destructivo, cambios de acceso,
credenciales, costes o fondos se necesita una autorización específica aplicable.

Mide espacio e inodos, crecimiento, RAM y picos de build/migración/persistencia/rollback.
No uses un número fijo de GB como garantía universal. Revalida condiciones dinámicas
inmediatamente antes del cambio. Coordina el bloqueo compartido con deploy y watchdog.

No borres volúmenes, archivos de PostgreSQL, `pg_wal`, AOF, copias o históricos a ciegas.
No desactives persistencia para que un healthcheck se vea verde. Planifica retención y
recuperación de espacio con evidencia; un `VACUUM` ordinario no garantiza liberar disco
al sistema operativo. Respeta rollups, archivos y la política de conservación aprobada.

## 9. Integración sin reversiones silenciosas

Trabaja sobre la base y el SHA actuales mediante **base + delta**, no reemplazo total.
Contrasta individualmente WO-GAP2, BR-02, BR-03, BR-05, CB-02, BR-06 y WO-07:
`apply_gate_rejection_fields`, frescura/backfill, símbolos bytes32, `PriorsCache`,
`runtime_knobs`, control runtime/heartbeat, propiedades CFMM y `stage2_calibration`.

Busca el código original y sus pruebas en las fuentes autorizadas. Si falta un cambio
sin commit, no afirmes haberlo recuperado desde una rama remota. Una reimplementación
es una reimplementación y debe probar su contrato funcional y su procedencia.

Consulta [integración y validación](references/integracion-y-validacion.md).

## 10. Agregador de blockchains, DEX y pools

Extiende los registros, API, workers y pantallas existentes. Mantén una fuente de
verdad coherente y separa **registrado, descubierto, cotizable, simulable y ejecutable**.
No confundas un catálogo amplio con soporte productivo de todos sus protocolos.

Por red verifica `chainId`, tipo de red, RPC/WSS y estado del bloque; por DEX,
factory/router, bytecode, protocolo y adaptador; por pool, pertenencia a factory,
tokens, decimals, reservas/liquidez y frescura. Usa identidades por cadena y dirección,
no por símbolo. Usa aritmética entera adecuada para importes y economía de ejecución.

Aplica paginación, límites de concurrencia, backpressure, deduplicación y presupuestos
RPC configurables. Registra reorgs, errores, fuentes y marcas temporales sin inventar
datos. Distingue datos ausentes de cero calculado. No aceptes RPC arbitrarios que
permitan acceder a destinos internos no autorizados.

Contrato detallado: [arquitectura live y agregador](references/arquitectura-live-y-agregador.md).

## 11. Testnet live y mainnet live: capacidad real, permisos explícitos

Implementa ambos modos en el producto. No rechaces una red únicamente porque es
mainnet. Exige activación explícita por cadena, configuración válida y evidencia
fresca; no conviertas `chain_id != 1` en sinónimo de testnet.

Aísla configuraciones, wallets, claves, colas y datos de prueba. No reutilices una
credencial de producción en un test ni conviertas tests de UI en órdenes financieras.
Las pruebas testnet con envío requieren una red y activos sin valor económico real.

Revalida cadena, bloque, contrato, firmante, roles, fondos, allowance, calldata,
nonce, financiación, oráculos, slippage, gas y límites antes del envío en el software.
El kill-switch debe verificarse antes del trabajo costoso y nuevamente en el límite
de firma/envío. Prueba también rechazo, RPC caído, datos viejos y reinicios.

Entrega controles de activación utilizables por el operador y el acta
`templates/ACTA_CAPACIDAD_LIVE.template.md`. Preparar mainnet no autoriza al agente a
firmar, enviar operaciones de valor o encender un ejecutor que negocie fondos reales.
La aprobación técnica y la decisión financiera son registros diferentes.

## 12. Pruebas reales mediante herramientas reales

Usa Desktop Commander para dirigir el proceso en el dispositivo verificado; usa
Playwright u otra herramienta de navegador instalada para ejecutar las interacciones.
No llames “probado con Desktop Commander” a escribir un comando sin ejecutarlo.

Lee los scripts y lockfiles antes de elegir comandos. Ejecuta build, typecheck,
lint y pruebas unitarias/integración del alcance afectado, más regresiones del sistema.
Los tests aislados pueden usar fixtures explícitas; nunca sirven como cifras productivas
ni como prueba de que un endpoint mainnet funciona. No inyectes mocks en la DApp real.

Prueba botones, formularios, toggles, filtros, registros de redes/DEX/pools, navegación,
actualización de datos, WebSocket, errores y recuperación. Comprueba efecto persistido,
no solo click o toast. Aísla las mutaciones de UI y los dispositivos de firma reales.

Guarda comandos, SHA, máquina, URL, fechas, exit codes, resultados, trazas y capturas
saneadas. Si no hay conexión, navegador o datos, marca `NOT_RUN` o `BLOCKED`, nunca PASS.

## 13. Entrega y criterio de cierre

Entrega código integrado, diff y commits; inventario y mapa; cambios `.claude`;
plan y bitácora; configuración sin secretos; matriz de soporte; pruebas y evidencias;
identidad de imágenes desplegadas; instrucciones de operación, rollback y activación.

Reporta por separado: implementado, probado localmente, probado en CI, desplegado,
verificado en testnet y preparado para mainnet. Un verde antiguo no valida un SHA nuevo;
un HTTP 200 de frontend no acredita backend, persistencia, WS ni rentabilidad.

No prometas cero fallos o beneficios. No declares “listo para operar” mientras haya
pruebas obligatorias pendientes, fuentes inaccesibles o un componente no conectado.
Declara el alcance exacto que sí completaste y el siguiente requisito concreto.

## 14. Cadencia de trabajo

Al iniciar, identifica objetivo, permisos y siguiente verificación. Durante tareas
largas comunica avances breves con resultados, no operaciones de bajo nivel repetidas.
No prometas trabajo en segundo plano: conserva estado y termina cada ejecución con
el resultado obtenido y sus límites. Una tarea no termina en un plan si aún puedes
implementar y verificar trabajo autorizado dentro de la sesión.

## 15. Biblioteca de conocimiento de ingeniería (`references/biblioteca/`)

Cuerpo de conocimiento técnico profundo que extiende esta skill: 13 referencias
(núcleo v1.0.0 + 11-22) redactadas por autores especialistas y auditadas
adversarialmente por verificadores técnicos independientes, más un crítico de
completitud (producción 2026-09-15; ~3.7M tokens de elaboración multi-agente).

**Carga progresiva**: esta SKILL.md (el encargo y su máquina de estados §4) gobierna
siempre. La biblioteca se lee POR REFERENCIA según la situación, nunca de corrido.
Ante conflicto núcleo-vs-referencia, gana la referencia 11-22 (verificada contra
fuentes primarias); las erratas conocidas del núcleo están listadas al pie del 00.

**GOBERNANZA**: todo el conocimiento de la biblioteca está subordinado a los gates
`arbx-*` (paper-trade-first, simulation-mandatory, risk-limits-enforcement,
pre-execute-checklist) y a CLAUDE.md §34 (LIVE_MAINNET gated, default-deny en el
terminus de ejecución). Nada de la biblioteca autoriza un flip a live, broadcast con
capital real, ni sobreescribe la máquina de estados de esta skill.

### Matriz de activación

| Situación | Referencia |
|---|---|
| Quotes exactas V2/V3/V4, Curve, Balancer, TWAP, oráculos, sizing óptimo | `11-amm-dex-math.md` |
| Grafo de liquidez, ciclos negativos (Bellman-Ford/SPFA), poda, scoring EV, split | `12-route-search-optimization.md` |
| Bundles Flashbots/MEV-Share, builders, bidding, liquidaciones, física L2, Jito | `13-mev-orderflow-bundles.md` |
| Contrato ejecutor, callbacks flash loan, gas golfing, approvals, UUPS, invariantes | `14-onchain-execution-contracts.md` |
| Nodos (reth/erigon), suscripciones, forks anvil/revm, tracing, fees, reorgs | `15-node-infra-estado.md` |
| p99/p999, teoría de colas, io_uring, allocators, TCP/colo, lock-free, benchmarks | `16-latency-engineering.md` |
| CEX-DEX, books L2/L3, delta-neutral, puentes cross-chain, Solana, depegs, órdenes firmadas | `17-cross-domain-arbitrage.md` |
| Kelly, VPIN, drawdown, EVT, stress, oráculos, backtesting honesto | `18-risk-management-quant.md` |
| Pirámide de tests, foundry invariants, fork pineado, replay, CI gating | `19-testing-fuzzing-invariantes.md` |
| REST/WS/gRPC/webhooks, Kafka/Streams/NATS, CDC, indexación blockchain, auth | `20-system-integration-patterns.md` |
| Desbloqueo de apps: hipótesis-driven, bisección, árboles de triage, escalera >1h | `21-unstick-any-app.md` |
| Go-live cero→producción: gates, staged rollout, SLOs, incidentes, DR, drills | `22-golive-playbook.md` |

### Índice de la biblioteca

- **`00-nucleo-ingenieria-mev.md`** — Núcleo v1.0.0 (10 secciones): pipeline MEV, executor
  UUPS, backend Rust/PG/Redis, DevOps, observabilidad, seguridad, performance. Erratas
  auditadas al pie (guarda de solvencia, k8s→compose).
- **`11-amm-dex-math.md`** — Quotes CPMM con fee sobre input y redondeo direccional |
  swap-step loop V3 (ticks, sqrtPriceX96, liquidityNet) | Newton-Raphson del invariante D
  de Curve | Balancer ponderado | **V4: PoolManager singleton, hooks, flash accounting
  settle/take** (hook desconocido = simulación revm obligatoria) | TWAP ∝ L·T | closed-form
  del sizing óptimo 2-pool | aritmética U256/Q64.96 sin flotantes.
- **`12-route-search-optimization.md`** — Grafo -log(rate) y teorema de ciclo negativo |
  Bellman-Ford/SPFA con reconstrucción anti-falsos-ciclos | multi-ciclo por ban de arista |
  Yen/DFS con cotas | poda capacity/gas/hop (lección XEN) | scoring EV post-gas con dedup
  Jaccard | incremental Sync + warm-start con presupuesto por bloque | split waterfilling |
  hot loop CSR + rayon + criterion.
- **`13-mev-orderflow-bundles.md`** — Supply chain searcher→builder→relay→proposer y
  timeline del slot | `eth_sendBundle`/`eth_callBundle` campo a campo | pricing
  coinbase-transfer vs priority fees | MEV-Share (hints, bids, pago al originador) |
  winner curse y bid shading por builder | **liquidaciones como estrategia** (HF, ventana
  de oráculo ajeno, ejecución atómica, JIT) | **física L2** (FCFS/Timeboost, priority fee
  OP-Stack, fees de dos pisos) | defensa sandwich/reorg/censoring | Jito | gestión de
  nonces bajo concurrencia.
- **`14-onchain-execution-contracts.md`** — Ejecutor minimalista sin custodia con
  delta-check de solvencia | dispatcher seguro de callbacks (Aave V3, Balancer V2, Uniswap
  V3) | gas golfing EIP-2929/3529 + calldata packing 25 B/hop | permit/Permit2 |
  Multicall3 atómico | MEV-resistencia (minOut simulado, deadline, roles, pausa) |
  UUPS + timelock | ciclo de despliegue inicial | 9 invariantes con tests Foundry.
- **`15-node-infra-estado.md`** — Matriz reth/erigon/geth | backfill+live-tail idempotente
  con head dual hot/stable | tiers de simulación (revm CacheDB → eth_call → anvil pineado →
  callBundle) | reserves desde logs Sync con preload warm | ladder de ticks V3 local |
  receta revm contra la versión pineada del workspace | bidding desde feeHistory (techo
  1.125³) | runbook de señales RPC (drift, 429, circuit breaker).
- **`16-latency-engineering.md`** — Presupuesto SLO por etapa + deadline propagation |
  amplificación de tail (ρ≤0.7, coordinated omission) | criterion/flamegraph/tokio-console/
  HdrHistogram | drain batching y bounded channels | mimalloc/jemalloc + arenas + arc-swap
  (RCU) | TCP_NODELAY, TLS resumption, selección de región | auditoría final por capa.
- **`17-cross-domain-arbitrage.md`** — Feeds WS por venue con snapshot+update y checksum |
  fees reales por endpoint (nunca asumidos) | microprice/OFI | delta-neutral spot-perp con
  funding | puentes sin atomicidad → pre-funding bilateral (Q_max = E_max/(z·σ_Δt)) |
  Solana/Jito | depegs como trampa de cola | **ciclo de órdenes firmadas** (auth por venue,
  clientOrderId como idempotencia, user-data streams, reconciliación con trial balance).
- **`18-risk-management-quant.md`** — Matriz capa→riesgo→señal→acción con caps anidados |
  Kelly asimétrico + fractional 0.25-0.5× | EV de bundle con P_incl medida (bid shading) |
  cota de ruina q^(ln r/ln(1−c)) | VPIN adaptado a AMM | EVT-POT + stress obligatorio
  (gas 10×, depeg, exploit) | screening de contratos | runbook key-compromise
  kill→revoke→rotate | backtesting walk-forward con descomposición del gap sim-vs-real.
- **`19-testing-fuzzing-invariantes.md`** — Pirámide L1-L6 (qué capa atrapa qué bug) |
  invariant handlers + ghost variables + allowlist de reverts | fork pineado = cero
  flakiness | generated-table+probe con regla anti-circularidad | proptest U256 |
  divergencia sim-vs-chain y golden blocks (prohibido re-etiquetar) | CI merge vs nightly |
  testing del control-plane TS/edge | anti-flaky.
- **`20-system-integration-patterns.md`** — Outbox transaccional SKIP LOCKED |
  snapshot+delta con resync por gap y dead-man switch (R8) | exactly-once como mito del
  consumidor + XAUTOCLAIM/PEL | unwind idempotente de reorgs con checkpoint | token bucket
  por host (lecciones 429) | webhooks HMAC raw-body anti-replay | DLQ | traceparent
  propagado | flags runtime vs horneadas en build | matriz mapeada al stack del repo.
- **`21-unstick-any-app.md`** — Orden sagrado observar→hipotetizar→predecir→experimentar→
  registrar | bitácora 1-variable con predicciones falsables | bisección (git bisect, cargo
  tree -d, config halves) | 6 árboles de triage (build/boot/vacíos-R7/hang/perf/flaky) |
  forense de logs con retención (LOGFLOOD-01) | red eslabón-por-eslabón | PostgreSQL
  plan-primero | escalera de 8 peldaños >1h | anti-patrones | cierre con test de regresión.
- **`22-golive-playbook.md`** — Pipeline de 12 pasos con freeze window T-48h | RESTORE DRILL
  obligatorio (backup no probado = no backup) | deploy veraz (SHA CI == git rev-parse HEAD,
  L4) | artefactos por digest con rollback N=3 | SLO multiburn 14.4×/6×/1× con freeze de
  error budget | SEV1-3 + evidencia forense antes de reiniciar | canary con abort pre-escrito |
  drills con cadencia + RTO/RPO | regla revert + gate nuevo.

## 16. Doctrina de validación cruzada Hermes (orden del operador 2026-09-17)

Toda criptografía y matemática del encargo (slots de storage, abi encode, aritmética
de importes, quotes, sizing) se valida con vector de referencia independiente +
Hermes (run durable: `hermes_run_start` + `hermes_run_status`; la llamada bloqueante
expira). Todas las estrategias y OPS (generación, implementación, edición, creación,
adaptación) se testean, prueban y ponen en marcha por Hermes y un grupo especializado
en arbitrajes de todo tipo — el autor nunca certifica su propia pieza. Canónico:
`arbitragex-omniscience` §12. Los gates de esta skill (§4, §11) quedan intactos.
Precedente: SIM-FUND-01b — 12 bytes de padeo erróneo, 706/706 fundings muertos,
certificados por un test que re-computaba la fórmula defectuosa.

## 17. Arsenal Hermes DeFi/Web3/Trading integrado a este comando (orden del operador 2026-09-22)

> Aditivo a §1-§16: el lote de 21 skills comunitarias DeFi/Web3/Trading (instaladas
> 2026-09-21/22 en `%LOCALAPPDATA%\hermes\skills\`; re-verificadas idempotente el
> 2026-09-22) queda integrado a `/arbx-live-engineering`. **SSOT del inventario completo
> con roles**: `arbitragex-omniscience` §14.6 (global) + espejo proyecto §14 punto 6 —
> aquí sólo la matriz de convocatoria por fase de la máquina de estados §4. Reglas de
> activación = omniscience §14.1 (visibilidad por reinicio de sesión Hermes;
> convocatoria por rol; doctrina de terceros ⊂ gates del repo). Censo vivo:
> `hermes skills list`.

### 17.1 Matriz de convocatoria por estado de §4

| Estado / trabajo | Skills del arsenal convocables | Notas |
|---|---|---|
| ACCESS_CHECK · VPS_READ_ONLY · VPS_MAPPED | `blockchain`, `defi-research`, `on-chain-analyitcs` (dir local; name "onchain-analysis"), `git-integrate` | Research/mapeo read-only; datos reales (RULE 00); jamás grafo o inventario simulado |
| CHANGE_AUTHORIZED · ENGINEERING_VERIFIED (código/contratos/frontend) | `web3-dapp-development-skill`, `react-coding-style`, `afrexai-web3-engineering`, `blockchain-web3-toolkit`, `ah-blockchain-developer`, `write-contracts`, `axodus-smartcontracts-audits`, `abi-to-mcp-generator` | `write-contracts` bajo FUSILE_SOURCE_POLICY + port-with-validation; `axodus-*` sinergia con security-auditor (auditoría read-only); `abi-to-mcp-generator` SOLO MCP local read-only (§33.2: jamás firma/broadcast) |
| Economía de ejecución: gas, precios, sizing | `gas-fee-tracker`, `coingecko-price`, `auto-trading-strategy`, `defi-trading-engine`*, `polymarket-arb-scanner-pro`*, `defi-stablecoin-yield-scanner`, `afrexai-investment-engine` | Gas/precios alimentan net-gate y sizing con datos reales (fuentes gratuitas del stack soberano); `defi-trading-engine` SOLO knowledge risk-manager/daily-review — `trade-executor` NO convocable (§32) |
| TESTNET_LIVE_VERIFIED · MAINNET_RELEASE_READY | `hft-paper-trader`, `ultimate-agents` | `hft-paper-trader` = paper/shadow ONLY (PAPER_SHADOW §34.1); `ultimate-agents` orquestación bajo §9 del omniscience — **`wallet.js` PROHIBIDO (§32: sin wallets/capital)** |

\* = instalada con `--force` tras hallazgos MEDIUM del scanner; scripts pip/npm de
terceros NO se ejecutan sin revisión.

### 17.2 Excluidas (registro fail-honest, heredado de omniscience §14.6.2)

`openmm`, `smart-accounts-kit`, `smart-contract-audit`, `chain-audit-deploy-skill`
(bloqueadas DANGEROUS por skills-guard-v5, `--force` NO overridea) y `clawhub/dapp`,
`clawhub/defi` (stale upstream). No existe vía de instalación forzada; no simular su
contenido.

### 17.4 Lote 2 (orden del operador 2026-09-22) — 6 skills adicionales

> Mismas reglas de §17.1/§17.7. SSOT: `arbitragex-omniscience` §14.7 (global).

| Estado / trabajo | Skill | Notas |
|---|---|---|
| CHANGE_AUTHORIZED · ENGINEERING_VERIFIED (validación de código) | `code-inspector`* | Inspección estática read-only; apoyo a validators; pip unpinned NO se ejecuta sin revisión |
| Cross-review de hallazgos en cualquier estado | `council-v2` | Consejo multi-rol con síntesis — refuerza la validación builder+validator (§16 doctrina Hermes) |
| Economía de ejecución: detección cross-exchange | `cross-exchange-arb-scanner` | Patrón CexDex en paper/shadow (§34.1); scanner Python read-only, datos reales (RULE 00) |
| VPS_READ_ONLY · TESTNET_LIVE_VERIFIED (due diligence de activos) | `onchain-token-diligence` | Alimenta gate `arbx-token-safety-screen` antes de registrar/aceptar tokens (§10 agregador) |
| Runs largos / checkpoints del encargo | `agent-dream`* | Tipos de memoria de agentes; setup.js npm NO se ejecuta sin revisión |
| Utilidad general del operador | `web3-pm-interview-skill`* | Fuera del hot-path; forceada por falso positivo `role_pretend` (revisado) |

**Excluidas**: `afrexai-agent-engineering` (DANGEROUS 11 hallazgos, sin override),
`python-debugpy` (stale upstream), `systematic-debugging`/`simplify-code` (ya builtin
del store Hermes — convocables por rol vía censo `hermes skills list`, no se duplican).

### 17.5 Lote 3 (orden del operador 2026-09-22) — 15 skills adicionales

> Mismas reglas de §17.1/§17.7. SSOT: `arbitragex-omniscience` §14.8 (global).

| Estado / trabajo | Skill | Notas |
|---|---|---|
| Runs largos / resiliencia de encargos | `trinity-harness` | Checkpoint patterns + taxonomía MAST; apoya PROGRESS.md de runs (§16) |
| Calidad de orquestación en cualquier estado | `agent-harness-engineering` | Bootstrap playbook, quality gates, GC → charters del gang |
| Checkpoints / memoria de runs | `afrexai-agent-memory`*, `agent-memory-sr`** | ** = force por U+FEFF (BOM Windows) en QUICKSTART.md:1 — falso positivo revisado |
| Evaluación del gang (post-entrega) | `agent-metrics-osiris`* | Métricas de agentes; metrics.py pip unpinned NO se ejecuta |
| Paneles watchdog/kill-switch (knowledge) | `warden-app` | Notas UI warden; read-only, sin tocar terminus §34 |
| Diseño de flujo de señales/precios | `crypto-signals-router`** | sudo/systemd del README NUNCA se ejecutan (§32/§33) |
| Economía cross-chain (diseño, no ejecución) | `cross-chain-arbitrage-cn`* | Knowledge paper/shadow ONLY (§34.1) |
| CHANGE_AUTHORIZED (backend TS) | `agency-backend-architect` | Apoyo control-plane/api-server |
| Auditoría multi-repo (§32 git-url) | `agent-repo-router` | Routing de repos para agentes |
| CHANGE_AUTHORIZED (scaffold) | `ai-backendfrontend-autowriter` | BAJO RULE 00 — jamás mocks ni datos fabricados |
| Utilidad general del operador | `ai-product-description-generator-free`* | Fuera del hot-path; environ/pip NO se ejecutan |
| WOs requirements→implementación | `fec-backend-requirements-handoff` | Handoff estructurado antes de construir |
| ENGINEERING_VERIFIED (tests) | `backend-test-plan`* | Pirámide de tests de backend; npm unpinned NO se ejecuta |
| Planning profundo pre-despacho | `deepagents-implementation`** | Deep agents multi-nivel; force por `"env":` en snippet JSON de docs — benigno |

`software-architecture-design` ya estaba (§14.2 del omniscience). **Excluidas DANGEROUS
sin override**: `agent-kanban` (6), `wallet-mcp` (17 — además §32/§33 prohíben wallets
vía MCP), `chainstream-data` (24). Censo tras lote: 116 enabled.

### 17.6 Lote 4 (orden del operador 2026-09-22) — PM de ingeniería + plugins

> Mismas reglas de §17.1/§17.7. SSOT: `arbitragex-omniscience` §14.9 (global).

| Estado / trabajo | Skill / Plugin | Notas |
|---|---|---|
| Cualquier estado con WO/encargo sustancial | `engineering-project-manager` (name `project-manager`) | PM SPICE: scope/WBS, baselines, milestones, risk register, traceability, release gates — apoya §37 (revert+gate, evidence-only) y R13 (manifestación de PRs); scripts Python NO se ejecutan sin revisión |
| Runs largos / compresión de contexto | plugin `hermes-snapcompact` | Archiva historia como PNG denso a ~1/3 tokens; enabled, carga al próximo arranque del gateway |
| Descomposición builder+validator | plugin `hades-coding-workflows` | Dual-model harness, subagent-driven dev, planning; enabled, carga al próximo arranque |

**Excluidas**: skill `nextflow` (DANGEROUS 11 hallazgos sin override — bioinformática,
sin rol aquí), plugin `crypto-prices` (DANGEROUS 1 CRITICAL `rm -rf $HOME` en README;
sin override — bypasear el scanner vía `plugins.scan_on_install` PROHIBIDO, frontera
§33.2). Censo: 117 enabled.

### 17.7 Frontera inquebrantable

El knowledge de trading/deploy/ejecución de estas skills NO autoriza nada que §4, §11,
§16, CLAUDE.md §32/§33/§34 o los gates `arbx-*` prohíban: sin capital, sin broadcast, sin
flips de modo, sin mocks (RULE 00). Su uso en este comando es INGENIERÍA read-only:
detección, auditoría, scaffolding y preparación del paquete de activación del operador.

### 17.8 Lote 5 (orden del operador 2026-09-22) — Redis/WS/DB operacional ("usarlos siempre")

> Mismas reglas de §17.1/§17.7. SSOT: `arbitragex-omniscience` §14.10 (global).

| Estado / trabajo | Skill | Notas |
|---|---|---|
| VPS_MAPPED / diagnóstico Redis (streams, grupos, lag, AOF, socket-proxy) | `redis-cluster-analyzer` | Topología cluster/sentinel/replicación; health de nodos |
| VPS_READ_ONLY / trazabilidad R7 (XLEN, XINFO GROUPS, XPENDING) | `redis-commander` | Su docker-compose.yml NO se levanta sin orden explícita del operador |
| CHANGE_AUTHORIZED (consumers WS, reconexión de sockets, backpressure) | `websocket-hub-patterns` | Patrón de diseño contra la clase-zombi (SCANNER-SILENT-02, enricher broken-pipe): jamás retry eterno sobre socket muerto |
| VPS_MAPPED / PostgreSQL internals (WAL, vacuum, particiones, locking) | `db-internals-deep-dive` | WAL-burst pacing, VACUUM FULL standalone, retention por particiones |
| VPS_READ_ONLY / exploración de esquema/datos | `db-explorer-lens` | Auditoría dirigida (paper_trade_runs, rollups) sin queries ciegos |
| Cualquier estado / triaje multi-fuente (PRs, issues, pipelines, incidentes) | `dx-data-navigator` | Alimenta el motor forense y el BOARD |

**Obligatorias por dominio** (orden "usarlos siempre"): Redis → analyzer+commander;
WS/reconexión → hub-patterns; PG internals → deep-dive+explorer-lens; data cross-stack
→ data-navigator. **Excluida**: `clawhub/redis-store` (índice stale, archivos upstream
inexistentes — ver §14.10.1 del omniscience; familia oficial `redis/agent-skills`
disponible si el operador decide sustituir).
