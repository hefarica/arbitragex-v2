---
name: arbitragex-omniscience
description: ARBITRAGEX DAPP OMNISCIENCE — La super-skill que integra 264 estrategias, 31 operadores, 60 detectores, knowledge graph (2,511 edges), doctrina de rutas, estado del arte mundial DApp/DeFi/MEV, y el DRIVER del loop Holy Grail (workbook canónico 83 hojas, WIP=1, evidence-only). Úsala para CUALQUIER pregunta sobre estrategias, rutas, sizing, financiamiento, MEV, DEX, optimización, ejecución, o para operar el loop de remediación HG. INCLUYE §7 Crypto Deep Analyser — due diligence exhaustiva de tokens/proyectos cripto vía workflow de 8 analistas + verificación adversarial; úsala también para "analiza a fondo X", "investiga X", "due diligence de X", "compara A vs B".
---

# ARBITRAGEX DAPP OMNISCIENCE

## Cómo activar el conocimiento completo

Cuando el operador hace cualquier pregunta relacionada con estrategias, rutas, operadores,
financiamiento, sizing, MEV, DEX, arbitraje, o ejecución, cargar TODOS estos recursos:

### 1. Canon (el 5% — Excel + repo)
```
skills/arbitragex-ultra/SUPER_SKILL.md                     ← Arquitectura + reglas
skills/arbitragex-ultra/knowledge_graph.jsonl              ← 2,511 edges Strategy↔Operator↔Detector
skills/arbitragex-ultra/capability_matrix.json             ← 265 estrategias con estado
skills/arbitragex-ultra/operators/op_XX/SKILL.md           ← 31 operator skills
skills/arbitragex-ultra/operators/op_XX/OPERATOR.json       ← 31 operator data
skills/arbitragex-ultra/strategies/MEV-XX-XXX/SKILL.md     ← 264 strategy cartridges
skills/arbitragex-ultra/strategies/MEV-XX-XXX/STRATEGY.json ← 264 strategy data
```

### 2. Doctrina (investigación mundial)
```
docs/ROUTES_CROWN_JEWEL_DOCTRINE.md                        ← RICH, CFMM convex, fees on-chain
docs/superpowers/plans/2026-08-19-GPRICE-SPEED-LIGHT.md   ← Precios velocidad de la luz
docs/superpowers/plans/2026-08-19-GSIM1-THREE-BUGS-PLAN-SUPREMO.md ← Bugs G-SIM-1
```

### 3. Implementación (código real)
```
backend/math-engine/src/operators/                         ← 31 operadores Rust
backend/searcher-rs/cartridges/strategies/                 ← 264 cartridges Rhai
backend/searcher-rs/src/route_discovery/                   ← Discovery + financing
backend/searcher-rs/src/workers/chainlink_subscriber.rs    ← Event-driven prices
backend/simulator-v2/src/                                  ← LazyDb + REVM runner
```

### 4. Mundo (el 95% — research)
```
skills/arbitragex-ultra/world/graph-algorithms/            ← RICH, BF variants, convex routing
skills/arbitragex-ultra/world/mev-practice/                ← Searcher real, bundles, margins
skills/arbitragex-ultra/world/defi-protocols/              ← UniV4, Morpho, Hyperliquid, intents
skills/arbitragex-ultra/world/security-simulation/         ← REVM, formal verify, attack surface
skills/arbitragex-ultra/world/quant-math/                  ← Kyle, HJB, Kelly multi-armed, VPIN
```

### 5. Datos extraídos del Excel
```
docs/excel_ingestion_manifest.json                          ← 47 hojas, 534K celdas
docs/excel_strategies_extracted.json                        ← 267 estrategias
docs/excel_operators_extracted.json                         ← 33 operadores
docs/excel_matrix_extracted.json                            ← 1,716 asociaciones
docs/excel_detectors_extracted.json                         ← 60 detectores
docs/coverage_manifest.json                                 ← Coverage verificada
```

## Reglas de razonamiento

1. **El Excel es el 5%** — nunca el límite. Cuando encuentres algo mejor, regístralo.
2. **Dos capas** — DISCOVERY (enumerar topología) ≠ EVALUATION (gates + sizing + EV)
3. **Financing = dimensión de ruta** — cambia qué rutas son viables, no cuántas se descubren
4. **Nada muere en silencio** — cada rechazo: (hop_tier, gate, razón, financing_mode)
5. **Fees on-chain** — leer de la cadena, nunca hardcodear (Aave = 5bps HOY, gobernable)
6. **Fail-honest** — "—" = no computado, 0 = exactamente cero, nunca fabricar
7. **Anti-hallucination** — clasificar: CANONICAL_WORKBOOK / CANONICAL_REPO / PRIMARY_SOURCE / INFERRED / HYPOTHESIS / UNKNOWN

## Consultas que puedes responder

- "¿Qué operadores usa MEV-06-018?" → knowledge_graph.jsonl
- "¿Qué estrategias usan Kelly?" → reverse lookup en matriz
- "¿Qué rutas sobreviven sin flash loans?" → funnel born/died por mode
- "¿Sizing óptimo para 2-pool WETH/USDC?" → fórmula cuadrática cerrada
- "¿Qué detector descubre triangular?" → detector families
- "¿Qué estrategia NO está en el Excel?" → gap analysis vs estado del arte
- "¿Hay un algoritmo mejor para esta ruta?" → comparar con world/
- "Due diligence de <TOKEN> / analiza a fondo <proyecto> / compara A vs B" → §7 Crypto Deep Analyser (workflow 8 analistas)

## 6. Holy Grail Loop Driver (referente operativo SSOT — workbook canónico)

El workbook **Holy_Grail_Audit_*.xlsx (83 hojas)** ES simultáneamente: contrato de intención,
evidencia de auditoría, cola de remediación WIP=1, prompt polimórfico por fila, ledger de
testigos anti-regresión, histórico de score y baseline inmutable N para el ciclo N+1.
**Ningún agente avanza una fila por afirmación propia** — sólo un fresh audit cierra (P-02).

### Ubicaciones (verificadas 2026-09-05)
```
Harness:   C:\HolyGrailAuditorV131\hg_v131_src        (servicio uvicorn, puerto 8090)
Runner:    C:\HolyGrailAuditorV131\hg_v131_src\agent_loop\hg_loop_runner.py
Runs:      C:\HolyGrailAuditorV131\hg_v131_src\data\runs\<run_id>\{run.json, baseline.xlsx, screenshots\}
Workbooks: C:\Users\HFRC\Downloads\Holy_Grail_Audit_*.xlsx   (el runner toma el más reciente como baseline)
CDP Edge:  "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --remote-debugging-port=9222 --user-data-dir=C:\HolyGrailEdgeProfile
```

### Comandos del driver
```
py -3 agent_loop/hg_loop_runner.py status                     # cola abierta + tail del score ledger
py -3 agent_loop/hg_loop_runner.py next                       # fila ACTIVE + EXACT_AGENT_PROMPT
py -3 agent_loop/hg_loop_runner.py run --repo https://github.com/hefarica/arbitragex-v2.git \
       --dapp https://arbx.ape-tv.net --baseline <xlsx> --port 8090    # ciclo N+1 (~2.5h)
```

### Protocolo esencial (63_AGENT_LOOP_PROTOCOL, P-00..P-20)
- **P-01 WIP=1**: trabajar SOLO la primera fila ACTIVE de 61_REMEDIATION_LOOP; el resto LOCKED.
- **P-02**: cierre sólo con fresh audit que pruebe el criterio de aceptación exacto.
- **P-03/P-12/P-13**: testigo o PASS perdido → STOP → revertir antes de cualquier trabajo nuevo.
- **P-04/P-05**: el score nunca se pinta hacia arriba; una mejora no compensa un PASS perdido.
- **P-07 REBASE-FIRST**: reconciliar intent del Excel + HEAD + refs + runtime ANTES de generar código.
- **P-10**: fallos del harness se reparan en el harness, no maquillando el DApp.
- **P-11 SAFE-BOUNDARY**: sin live/mainnet, capital, signers ni broadcast (§34 del CLAUDE.md).

### Hojas clave
`61_REMEDIATION_LOOP` (cola 46 cols, ACTIVE=NR-0000 barrier) · `62_CLOSURE_RATCHET_LEDGER`
(testigos) · `63_AGENT_LOOP_PROTOCOL` · `64_SCORE_LEDGER` (score floor + MONOTONIC_VERDICT) ·
`48_SURFACE_CERT` (cert por superficie, latency_ms + failed_requests + root cause) ·
`50_CURRENT_DEFECTS` · `52_FINAL_GATE_2026` (F-01..F-24) · `40_GATES_A1_A9` · `24_MASTER_CHECKLIST`.

### Arquitectura de tráfico del dominio público (VERIFICADA — cambia el diagnóstico)
```
browser → Cloudflare (104.21.x/172.67.x) → cloudflared.service (tunnel, VPS) → http://localhost:5173 (frontend)
```
- **nginx :80 está FUERA del camino del dominio público** — su access.log NO refleja tráfico del
  dominio. La verdad per-request del dominio está en `journalctl -u cloudflared` (VPS).
- `Incoming request ended abruptly: context canceled` en cloudflared = el NAVEGADOR abortó
  (firma de NETWORK_FAILURE en el audit; ≠ 429 ≠ 502 de origin).
- Latencia ~28.6s en surface cert = timeout del navegador bajo carga del barrido.

### Gotchas operativos (aprendidos con sangre 2026-09-05)
1. **NUNCA redeployar/recrear la flota del VPS con un audit en curso** — #532 deployó 22:45Z y
   mató el run ce5067dd (zombie en "Organism · genome").
2. Edge CDP muerto → el run falla con `Browser.new_context: Connection closed` (caso a9b1917c).
   Relanzar Edge con el comando de arriba y verificar `curl 127.0.0.1:9222/json/version`.
3. VPS corre **UTC**; `docker logs --since/--until` interpreta en UTC (local del operador = -05:00).
4. El rate-limit #531 vive en el **edge worker** (KV bucket per-IP, devuelve 429 HTTP), no en nginx.
5. `docker exec edge-1` → el container se llama `arbitragex-v2-edge-1` (prefijo de compose).
6. run.json de un run completo pesa ~92MB — leer con streaming/targeted JSON, jamás entero.

## 7. Crypto Deep Analyser (port del Venice Mind `crypto-analyser-1372`)

> **Origen**: mind del operador en Venice.ai ("Crypto Deep Analyser", publicado 2026-09-06,
> 8 sub-agentes + 5 tools, razonamiento enabled). Portado a Claude Code el mismo día con un
> upgrade: aquí los analistas corren con WebSearch/WebFetch reales y se añadió una capa de
> **verificación adversarial** que el mind original no tiene. El mind NO es invocable vía
> API pública v1 (verificado 2026-09-06: `api.venice.ai/api/v1/minds` y `/api/v1/agents` =
> 404 con key válida; los minds viven solo en la web app detrás de client-attestation en
> `outerface.venice.ai/api/v2/agents/*`) — **el port local ES el pipeline**.
> **v2 (mismo día, orden del operador)**: auto-dispatch obligatorio + canon omniscience
> inyectado en los 8 analistas ("la skill suprema al servicio de los 8").

### Disparadores
"Due diligence de \<TOKEN\>" · "analiza a fondo \<TOKEN/proyecto\>" · "investiga \<proyecto\>" ·
"compara \<TOKEN A\> vs \<TOKEN B\>" · tokenomics de X · risk assessment de X · research cripto.

### Ejecución (AUTO-DISPATCH OBLIGATORIO — la invocación ES el gatillo)
**Regla dura**: si esta skill se invoca (slash `/arbitragex-omniscience` o auto-trigger) y el
mensaje trae un target de research, DESPACHA el Workflow EN EL MISMO TURNO — sin preguntar,
sin resumir antes, sin responder de memoria:
```
Workflow({ name: "crypto-deep-analyser", args: { target: "<TOKEN o PROYECTO>", lang: "español" } })
```
- Con target → Workflow **SIEMPRE, inmediato**.
- Sin target (consulta de estrategias/rutas/HG) → secciones 1-6; §7 no consume nada.
- Target ambiguo → pregunta SOLO el target y despacha.
Script: `.claude/workflows/crypto-deep-analyser.js` (si el `name` no resuelve, pasar `scriptPath`).
Args: `target` (obligatorio, admite "A vs B"), `lang` (default "español"), `asOf` (default hoy).
Prioriza completitud sobre velocidad — igual que el mind original, tarda minutos.

### Arquitectura: 8 analistas paralelos → verificación adversarial → síntesis
| # | Analista | Charter |
|---|----------|---------|
| 1 | Tokenomics | Supply (max/total/circulating, emisión), distribución, vesting/unlocks, inflación, utilidad |
| 2 | Market & Price | Price action multi-horizonte, volumen, liquidez (CEX+DEX), FDV/MCAP, correlaciones BTC/ETH |
| 3 | Community & Sentiment | Social (X/Reddit/Discord), actividad GitHub (commits/contributors), gobernanza, salud |
| 4 | News & Developments | Últimos 90 días: upgrades, partnerships, listings, funding, regulatory, roadmap vs promesas |
| 5 | Risk & Security | Auditorías (auditor, fecha, severidad, remediación), exploits, regulatory, team doxx, admin keys |
| 6 | Technical Architecture | Diseño de protocolo, innovaciones vs prior art, tradeoffs debatidos, whitepaper/specs |
| 7 | Use-case & Competition | Adopción real (usuarios/tx/TVL/revenue), TAM, 3-5 competidores directos, moat |
| 8 | Scaling | Throughput medido vs claims, L2/rollup strategy (L2Beat stage), stress tests, fees bajo carga |

Cada analista devuelve `findings[{claim, evidence, source_url, confidence}]` + `red_flags[]` +
`data_gaps[]`. Luego los red flags + top findings high-confidence pasan por verificadores
adversariales (`confirmed` / `refuted` / `unverifiable`) antes de la síntesis — upgrade local
que el mind Venice no tiene.

**Canon omniscience al servicio de los 8** (v2): cada analista, verificador y la síntesis
llevan inyectado un preamble con el canon — rutas **verificadas en disco**
(`skills/arbitragex-ultra/**`: SUPER_SKILL, knowledge_graph 2,511 edges, capability_matrix,
operators/, strategies/, world/{graph-algorithms, mev-practice, defi-protocols,
security-simulation, quant-math}), `docs/excel_*_extracted.json` y
`docs/ROUTES_CROWN_JEWEL_DOCTRINE.md` — más las reglas de razonamiento heredadas
(fail-honest, fees on-chain, clasificación anti-hallucination). Cada charter tiene un
"canon angle" específico (Risk → world/security-simulation; Architecture → world/defi-protocols;
Market → world/quant-math; etc.). Evidencia del canon = etiqueta `CANON_INTERNAL` + path;
evidencia web = URL.

### Formato de salida (contrato del mind original + honestidad R8)
1. **Executive Summary** — hallazgos clave ponderados por confianza
2. **Detailed Findings** — por categoría (las 8)
3. **Confidence Assessment** — High/Medium/Low + por qué, por sección
4. **Source Citations** — todo claim trazable a URL pública
5. **Risk-Adjusted Conclusion** — balance explícito incorporando incertidumbre
6. **Data Gaps** (extensión local) — lo NO verificado, dicho en claro

### Reglas doctrinales
- **Zero fabricación** (RULE 00): sin fuente real consultada = data gap, jamás finding.
- **Fail-honest** (R8): "no verificado" ≠ "no existe"; jamás rellenar huecos.
- Es **research, NO asesoría financiera** (§1 identidad): sin recomendaciones buy/sell.
- Herramienta **externa al pipeline ARBX**: no toca detección, cartuchos, sizing ni gates.

### Persona Venice (GLM 5.2) — para `chatWithVenice` single-shot
El API v1 no puede correr los 8 sub-agentes; esta persona destilada sirve para consultas
rápidas vía el módulo global:
```ts
import { chatWithVenice } from "C:/Users/HFRC/.claude/venice-global";
const CRYPTO_ANALYSER_PERSONA = "You are Crypto Deep Analyser, a research-intensive cryptocurrency analysis system that prioritizes accuracy and completeness over speed. Before answering, reason through eight specialist perspectives in order: tokenomics, market and price, community and sentiment, news and developments, risk and security, technical architecture, use-case and competition, and scaling. Every claim must be traceable to a public source when possible; mark anything you cannot source as a data gap instead of guessing. Structure every response as: Executive Summary; Detailed Findings by category; Confidence Assessment (high/medium/low per section, with reason); Source Citations; Risk-Adjusted Conclusion. You provide research, not financial advice.";
const reply = await chatWithVenice(
  [
    { role: "system", content: CRYPTO_ANALYSER_PERSONA },
    { role: "user", content: "Deep analyse <TOKEN>" },
  ],
  { temperature: 0.4 }
);
```

## 8. Convocatoria Universal de Agentes (modo CCR — todos con el LLM de sesión)

> **Orden del operador 2026-09-06**: "todos los que no salen, configúralos para que salgan en
> sus mismos roles con el LLM seleccionado (zai-coding-plan/GLM-5.3)". Causa raíz probada con
> drill empírico: un agente con `model:` pinneado en frontmatter fuerza routing Anthropic →
> **401 "Not logged in"** (pin sonnet) o **400 "All target providers failed"** (pin opus/haiku)
> en sesiones CCR sin login Anthropic. **Cura**: SIN `model:` en frontmatter el agente hereda
> el LLM de sesión y sale en SU MISMO ROL.

### Qué se aplicó (respaldado, reversible)
| Pieza | Acción | Estado |
|---|---|---|
| 67 agentes `ecc:*` (`~/.claude/skills/ecc/agents/`) | línea `model:` eliminada del frontmatter (58 sonnet / 8 opus / 1 haiku); roles y `tools:` intactos | ✅ disco |
| Mirror `.kimi/agents/` (mismos 67) | mismo strip | ✅ disco |
| Built-ins pineados: `Explore` (opus) · `claude-code-guide` (haiku) · `statusline-setup` (sonnet) | shadows sin pin en `~/.claude/agents/{Explore,claude-code-guide,statusline-setup}.md` | ✅ disco |
| 10 OMEGA nativos (`.claude/agents/`) | ya estaban SIN pin | sin cambio |
| Backups | `~/.claude/backups/ecc-agents-pinned-2026-09-06/` + `ecc-kimi-agents-pinned-2026-09-06/` | ✅ |

### ⚠️ Gotcha crítico: el registry de agentes es snapshot de session-start
Probado post-strip: re-spawns de `ecc:code-explorer`, `ecc:architect`, `Explore` y un ecc NUNCA
usado en la sesión (`ecc:vue-reviewer`) SIGUEN muriendo con el MISMO pin (sonnet-5/opus-5) —
el runtime cachea las definiciones al arrancar y no relee el disco. **El fix es efectivo desde
la PRÓXIMA sesión**. En la sesión en curso solo convocan los que nacieron sin pin (4 core) y
los subagentes de workflow (12 del pipeline §7).

### Doctrina de despacho (permanente en esta skill)
1. Despachar roles con `agentType: 'ecc:<rol>'` o shadows **SIN** parámetro `model` — el
   override (`sonnet|opus|haiku|fable`) SIEMPRE fuerza Anthropic y muere en CCR.
2. Agente muere 401/400 → NO reintentar con otro modelo. Verificar pin residual
   (`grep -l "^model:" ~/.claude/skills/ecc/agents/*.md`) o sesión stale; fallback = agente
   por defecto + rubric del rol copiado al prompt (rubrics: `.claude/agents/skills/<rol>/agent-prompt.md`).
3. Conteo honesto post-fix (desde próxima sesión): 67 ecc:* + 4 core (`general-purpose`,
   `claude`, `Plan`, `adidas-mode`) + 3 shadows + 12 pipeline §7 = **86 roles convocables**
   con LLM de sesión. Los ~675 SKILL.md de `.agents/skills/` son CONOCIMIENTO/rubrics que
   alimentan prompts — NO agentes spawnables.

## 9. GANG OMNISCIENCE — Mesa Redonda PhD con Loop de Éxito Total (v1, 2026-09-06)

> **Orden del operador (2026-09-06, nocturno)**: al invocar `/arbitragex-omniscience` con un
> /goal, OMEGA es EL ORQUESTADOR de un **gang de agentes PhD especialistas** (mínimo 20, máximo
> 100 si el /goal lo exige). Si faltan skills, están desactualizadas o parcialmente
> implementadas en el workspace → **se CREAN**, se les asigna su agente PhD especialista y se
> integran al gang. Mesa redonda con eje central: todos al tanto del trabajo de todos, cada uno
> en su espacio, sin pisarse ni dañarse; se cuestionan entre sí para encontrar las mejores
> maneras. Los agentes actúan como **USUARIOS y navegadores web de la DApp** y dan fe de que lo
> hecho funciona como lo haría quien lo solicita (humano). Loop éxito-o-éxito: detectar gaps →
> confirmar operativas → encontrar errores → corregir o confirmar, una y otra vez hasta el
> /goal. Todo queda embebido aquí — esta sección ES la doctrina permanente.

### 9.1 El contrato del gang
1. **/goal primero**: el orquestador PIDE o ENCUENTRA el /goal y lo CONFIRMA con el operador
   antes de despachar. El /goal se desglosa en un **BOARD** kanban (`GOAL-WORKORDERS.md` en un
   directorio `audits/<programa>-<fecha>/`): un work-order por ítem, con dueño, gate y estado.
   El BOARD es el **eje central** — todo agente lo lee ANTES de trabajar y lo actualiza al
   terminar su WO. Nadie reporta solo al orquestador: reporta AL BOARD.
2. **Composición dinámica**: el orquestador selecciona especialistas del registro (§8: 86 roles
   convocables + 675 rubrics `.agents/skills/` + 50 skills elite `.claude/skills/SKILL_*`).
   **Gap de skill** (inexistente/desactualizada/parcial) → el orquestador la CREA (SKILL.md con
   charter + protocolo + evidencia) y le asigna su agente PhD (definición `.claude/agents/` o
   rubric inyectada al subagent). El gap se documenta en el BOARD como una fila más.
3. **Anti-colisión (nadie pisa a nadie)**: cada WO declara **claims de archivo** explícitos.
   Dos agentes que tocan el mismo archivo van en SERIE (§36.4); archivos distintos van en
   PARALELO. En el árbol compartido: trabajar con WO-IDs en comentarios de diff (`// WO-XX
   (fecha)`) y CERO commit/push/PR/deploy sin el gate final del operador (protocolo
   no-git-until-final-gate 2026-08-23).
4. **Mesa redonda (cross-examination)**: cada entregable pasa por (a) un **cross-examiner** par
   que lo desafía con evidencia propia (preguntas cruzadas registradas en el BOARD), (b)
   **verificación adversarial multi-lente** para hallazgos (correctness / doctrina / evidencia /
   materialidad; CONFIRMADO exige mayoría), y (c) síntesis que adjudica discrepancias R8 (cifra
   vs cifra, sin promediar). Los desacuerdos NO se ocultan: se registran y se escalan.
5. **Navegación web real (los agentes son el humano)**: browser-verifiers abren la DApp
   (dominio público `https://arbx.ape-tv.net` o build local), navegan como usuarios, ven lo que
   ve un humano, capturan screenshots/console/network como evidencia, y **dan fe** (PASS/FAIL
   con evidencia). Herramientas: Playwright MCP de sesión, skill `webapp-testing`
   (`~/.claude/skills/webapp-testing`), doctrinas Playwright `.agents/skills/arbx-g-g013..021`,
   y socket.io-client REAL para WS (curl muere en ping/pong). **Presupuesto 429**: máximo 5
   requests HTTP manuales por agente al dominio público (memoria: sweep 429 self-contamination);
   el navegador no hace barridos — hace journeys.
6. **Loop éxito-o-éxito**: `while (gaps > 0 && rounds < max) { despachar gaps → verificar →
   actualizar BOARD }`. El loop termina SOLO en: (a) criterios de aceptación del /goal
   VERIFICADOS con evidencia (browser/tests/build), o (b) **blockers gated** documentados
   (acciones operador-only: flips §34.3, VPS, commits, capital) — que se escalan al operador con
   el paquete listo para su decisión. **"Éxito" jamás = datos fabricados** (RULE 00/R8): si el
   gate dice NO_GO, el éxito del gang es haberlo PROBADO y documentado con el remedio diseñado.
7. **Perímetro invariante** (hereda §32/§33/§34): audit/scaffold/shadow/read-only. Sin executor,
   wallets, capital, firma ni broadcast. VPS solo lectura por ssh (`arbx`) con comandos
   read-only. Flips = operador. La pregunta canónica §34.4 gobierna cada diseño.
8. **Escalado de fallas de provider — RESPAWN-2 (429 o CUALQUIER razón)** (orden del operador
   2026-09-06): si un agente aborta, **el resto del gang SIGUE ejecutando sin interrupción**
   (nadie espera al caído). El orquestador NO reintenta 1:1: **crea 2 agentes adicionales con
   el DOBLE de conocimiento del que se cayó y SUBREPARTE la tarea entre ambos** (mitad A +
   mitad B, cada mitad con rigor PhD completo y effort elevado — pandilla sobre UNA tarea:
   no de velocidad, sino de conocimiento en menos tiempo). Si un reemplazo también cae, se
   aplica RESPAWN-2 de nuevo sobre SU mitad. La baja y el re-despacho se registran en el BOARD
   con la causa exacta (R8). Implementado en `omniscience-gang.js` (helper `runAgent`) y
   aplicable a TODO despacho de agentes en CUALQUIER workspace CCR (orden global del operador).
   **Límite del circuit-breaker (v1.2, orden del operador 2026-09-07 "controlar para no hacer
   rate limit")**: RESPAWN-2 duplica CONOCIMIENTO, nunca presión. Bajo un 429 **sistémico**
   (límite del proveedor, no muerte aislada) duplicar spawns AMPLIFICA el problema — lección
   wf_29b60a15-af2 (2026-09-06/07): 63/71 agentes muertos, fases Cross y Browse aniquiladas.
   Controles obligatorios en toda orquestación multi-agente: (a) **oleadas** — despacho en
   olas de ~4 agentes concurrentes, jamás bursts de 16; (b) **presupuesto global de respawns**
   (techo duro anti-amplificación, default 8); (c) **tripwire sistémico** — muertes ≥ 6 y > 2×
   éxitos ⇒ se suspenden TODOS los respawns y las olas restantes (respirar, no golpear); se
   reanuda con `resumeFromRunId` cuando la cuota revierte (lo completado sale del cache, 0
   requests); (d) **reporte DEGRADADO honesto** — una fase esperada con 0 resultados NUNCA se
   declara éxito (R8). Todo agente muerto por 429 se registra con causa exacta en el BOARD.

### 9.2 Plantillas ejecutables (guardadas en el repo)
- **Workflow**: `.claude/workflows/omniscience-gang.js` — parametrizado por
  `{ goal, boardPath, baseUrl, maxAgents, rounds }`. Fases: Compose → Round → Cross → Browse →
  Loop → Synthesize. Despachar con `Workflow({ name: "omniscience-gang", args: {...} })`.
- **Agente navegador**: `.claude/agents/dapp-browser-verifier.md` — el rol PhD "usuario de la
  DApp" (convocable como `agentType` desde la sesión siguiente a su creación; mientras tanto su
  rubric vive inyectada en los subagents del workflow).
- **Skills de soporte ya integradas**: `webapp-testing` (Playwright local),
  `superpowers:systematic-debugging` (debug con método),
  `superpowers:subagent-driven-development` (despacho guiado por subagents),
  `.agents/skills/arbx-g-g013..021` (doctrinas Playwright ARBX), familia `arbx-*` runtime-status.

### 9.3 Protocolo de despacho (lo que hace el orquestador, paso a paso)
1. Confirmar /goal con el operador (si no vino dado).
2. `mkdir audits/<programa>-<fecha>/` + desglosar /goal → BOARD + anunciar composición del gang
   (roles, skills a crear, claims de archivo).
3. Despachar Oleada "Round" (especialistas en PARALELO por claims; SERIE por archivo).
4. Despachar Oleada "Cross" (pares se examinan; adversarial verify de hallazgos).
5. Despachar Oleada "Browse" (browser-verifiers dan fe en la DApp viva).
6. Loop: gaps → re-despacho; blockers gated → paquete de escalada.
7. Síntesis final al operador: qué quedó VERIFICADO, qué quedó diseñado, qué exige SU decisión.
8. Todo reporte intermedio visible para el operador (transparencia total: el operador lee TODO
   lo que dicen y proponen los agentes — BOARD + archivos + monitor del directorio).

### 9.4 Patrón probado (evidencia 2026-09-06, 44+ agentes, 3 workflows)
Ground-tright único recolectado por el orquestador (evita N×429 al dominio) · seed findings
refutables (el cross LOS REFUTA si están mal — 4 de 12 refutadas esa noche) · snapshot git+VPS
inyectado en todos los prompts · reportes por superficie `NN-*.md` + `NN-*-CROSS.md` · preguntas
cruzadas `41-ROUNDTABLE-QUESTIONS.md` · síntesis `00-PREDATOR-ROADMAP.md` con veredictos por
superficie × capa (LOCAL/REMOTE_MAIN/VPS/DOMINIO) · informe final `40-STATIC-AUDIT-FINAL.md`.
**Este patrón es el defaults del gang.**

### 9.5 GANG INTEGRADO — auto-dispatch + LOOP + consumo inteligente de tokens (orden del operador 2026-09-07)

`/omniscience-gang` **NO es un comando aparte: ES el modo de ejecución de esta skill.** Cualquier
invocación de `/arbitragex-omniscience` cuya tarea exceda lo que un contexto resuelve solo
(auditoría 0-100%, programa de WOs, remediación multi-superficie, verificación de release) despacha
el gang EN EL MISMO TURNO — sin preguntar, sin resumir antes (mismo contrato que §7):

```
Workflow({ scriptPath: ".claude/workflows/omniscience-gang.js", args: {
  goal, boardPath: "audits/<programa>-<fecha>/GOAL-WORKORDERS.md",
  programDir: "audits/<programa>-<fecha>", baseUrl: "https://arbx.ape-tv.net",
  rounds: 2, maxAgents: 40, concurrency: 4, respawnBudget: 8, now: "<fecha>" } })
```

**LOOP éxito-o-éxito (no one-shot):** al completar el workflow, el orquestador evalúa
`honest_final`: `DEGRADADO`/`PARCIAL` o gaps agent-fixables ⇒ **re-invocar con
`resumeFromRunId`** (lo hecho sale del cache = 0 tokens repetidos); muro 429 sistémico ⇒
circuit-breaker (§9.1.8 v1.2) ⇒ esperar reversión de cuota y reanudar con resume; sólo dos
salidas terminales: criterios VERIFICADOS, o todo lo restante operator-gated (paquete de
escalación). El BOARD es el SSOT del loop: cada reanudación relee el board, no la memoria.

**Consumo inteligente de tokens = máximo CONOCIMIENTO por token, no velocidad:**
1. **Cache-resume primero**: jamás re-correr lo completado (prompt+args byte-idénticos; cambiar
   `now`/`goal` bustea todo el cache).
2. **Oleadas 4 + presupuesto de respawns** (v1.2): cada token gastado contra un proveedor
   saturado es token quemado — el breaker PROTEGE el presupuesto.
3. **Adversarial-verify temprano**: un hallazgo falso refutado en Cross cuesta 1/10 de lo que
   costaría implementarlo — matar temprano es ahorro.
4. **Respawn-2 con subreparto**: 2 agentes de doble conocimiento sobre MITADES hacen más
   conocimiento/token que 1 reintentando todo (y fusionan con worst-wins R8).
5. **Ground-truth único del orquestador** (snapshot git+VPS+dominio inyectado en todos los
   prompts): N agentes no re-descubren lo que 1 sondeo compartió.
6. **Claims de archivo + serial-groups**: evita tokens quemados en conflictos de merge.

**Team agents / agent teams:** para misiones con mutación concurrente de archivos, combinar el
gang (workflow determinista) con **Agent teams** (teammates en worktrees, builders+validators en
paralelo §16.2): el workflow orquesta fases y verificación; los teammates ejecutan builds/fixes
aislados. Rust SIEMPRE serie total (§36.4 — target/ compartido).

## Invocación

El operador simplemente pregunta. Esta skill se activa automáticamente para cualquier
consulta relacionada con estrategias, rutas, operators, financiamiento, sizing, MEV,
DEX, arbitraje, optimización, ejecución, cualquier aspecto de la dapp ArbitrageX, o
due diligence / investigación profunda de tokens (Crypto Deep Analyser §7). Toda tarea
que exceda un contexto solo **despacha el GANG (§9.5) en el mismo turno** — loop
éxito-o-éxito con consumo inteligente de tokens.

No necesitas un comando especial — solo pregunta.
