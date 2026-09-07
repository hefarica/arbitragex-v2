// GANG OMNISCIENCE §9 — Mesa redonda PhD con loop de éxito total (v1.2, 2026-09-06/07)
// v1.1: RESPAWN-2 §9.1.8 (orden del operador) — agente muerto (429 o cualquier razón) →
//       2 reemplazos de doble conocimiento con la tarea SUBREPARTIDA; el gang sigue sin parar.
// v1.2: CIRCUIT-BREAKER ANTI-RATE-LIMIT (orden del operador 2026-09-07: "controlar para no
//       hacer rate limit"). Lección wf_29b60a15-af2: 63/71 agentes muertos por 429 sistémico —
//       RESPAWN-2 duplica CONOCIMIENTO, nunca presión sobre un proveedor saturado. Controles:
//       (a) oleadas: Cross/Browse/Loop en olas de args.concurrency (default 4) — sin bursts de 16;
//       (b) presupuesto GLOBAL de respawns args.respawnBudget (default 8) — techo duro anti-amplificación;
//       (c) tripwire sistémico: muertes ≥ args.tripDeathThreshold (default 6) Y > 2× éxitos ⇒
//           se SUSPENDE todo respawn y las olas restantes (reanudar con resumeFromRunId);
//       (d) honest_final DEGRADADO cuando una fase esperada entrega 0 resultados (R8, sin overclaims).
// Despacho: Workflow({ name: "omniscience-gang", args: { goal, boardPath, programDir, baseUrl, rounds, maxAgents, concurrency, respawnBudget, tripDeathThreshold } })
export const meta = {
  name: 'omniscience-gang',
  description: 'Gang Omniscience §9: 20-100 agentes PhD resuelven un /goal — mesa redonda, cross-examination, navegación web real como usuarios, loop éxito-o-éxito',
  phases: [
    { title: 'Compose', detail: 'componer el gang según BOARD + inventario de roles/skills' },
    { title: 'Round', detail: 'especialistas ejecutan WOs con claims de archivo (serie si comparten archivo/target)' },
    { title: 'Cross', detail: 'cross-examination par-a-par + adversarial verify de entregables' },
    { title: 'Browse', detail: 'browser-verifiers navegan la DApp como humanos y dan fe' },
    { title: 'Loop', detail: 'gaps → re-dispatch hasta criterios verificados o blockers gated' },
  ],
}

const args0 = args || {}
const GOAL = args0.goal || 'resolver el /goal del BOARD'
const BOARD = args0.boardPath
const DIR = args0.programDir
const BASE = args0.baseUrl || 'https://arbx.ape-tv.net'
const MAXR = args0.rounds || 2
const NOW = args0.now || '2026-09-06'

const RULES = `REGLAS DURAS (inviolables, CLAUDE.md):
- RULE 00 ZERO MOCKS: nada de datos fabricados/simulados/decorativos. Fail-honest R8: "no computado" se declara, jamás se inventa.
- §32/§33 PERMANENT audit/scaffold/shadow/read-only: CERO executor, wallets, capital, firma, broadcast. VPS solo lectura por ssh arbx (docker ps/logs/inspect, redis-cli read, psql SELECT). Prohibido mutar VPS.
- §34.3: flips LIVE_MAINNET = operador-only. default-deny y MainnetRefused INTOCABLES.
- NO-GIT: CERO commit/push/PR/deploy (protocolo operador 2026-08-23). Edición local + verificación (tests/tsc/cargo check) SOLAMENTE.
- Diffs propios marcados con el ID del WO: "// WO-XX (${NOW})".
- Presupuesto dominio público: máximo 5 requests HTTP manuales por agente (429 self-contamination). El navegador hace journeys, no barridos.
- Lexicon OMEGA: TLS/Holonomic Loop Resolution/Topological Yield/Decoherencia de Estado/Variedad de Liquidez.
- Board: ${BOARD}. Todo entregable escribe su reporte en ${DIR} y cita file:line como evidencia.`

const ROUND_SCHEMA = {
  type: 'object', properties: {
    wo: { type: 'string' }, status: { type: 'string', enum: ['DONE', 'BLOCKED', 'PARTIAL'] },
    deliverable: { type: 'string' }, files_touched: { type: 'array', items: { type: 'string' } },
    verification: { type: 'string' }, blocked_reason: { type: 'string' },
    report_path: { type: 'string' },
  }, required: ['wo', 'status', 'deliverable', 'verification', 'report_path'],
}
const CROSS_SCHEMA = {
  type: 'object', properties: {
    wo: { type: 'string' }, verdict: { type: 'string', enum: ['PASS', 'GAPS'] },
    gaps: { type: 'array', items: { type: 'object', properties: {
      desc: { type: 'string' }, blocked_by: { type: 'string', enum: ['agent-fixable', 'operator-gated'] },
      fix_hint: { type: 'string' } }, required: ['desc', 'blocked_by'] } },
    questions: { type: 'array', items: { type: 'string' } },
  }, required: ['wo', 'verdict', 'gaps', 'questions'],
}
const BROWSE_SCHEMA = {
  type: 'object', properties: {
    persona: { type: 'string' }, verdict: { type: 'string', enum: ['FE', 'FAIL', 'GAPS'] },
    evidence: { type: 'array', items: { type: 'string' } },
    gaps: { type: 'array', items: { type: 'object', properties: {
      desc: { type: 'string' }, blocked_by: { type: 'string', enum: ['agent-fixable', 'operator-gated'] } }, required: ['desc', 'blocked_by'] } },
    screenshots: { type: 'array', items: { type: 'string' } }, report_path: { type: 'string' },
  }, required: ['persona', 'verdict', 'evidence', 'gaps', 'report_path'],
}
const PLAN_SCHEMA = {
  type: 'object', properties: {
    specialists: { type: 'array', items: { type: 'object', properties: {
      wo: { type: 'string' }, role: { type: 'string' }, kind: { type: 'string', enum: ['design', 'apply', 'verify', 'redesign'] },
      charter: { type: 'string' }, files: { type: 'array', items: { type: 'string' } },
      serial_group: { type: 'string' } }, required: ['wo', 'role', 'kind', 'charter', 'files'] } },
    browsers: { type: 'array', items: { type: 'object', properties: {
      persona: { type: 'string' }, journey: { type: 'string' } }, required: ['persona', 'journey'] } },
    skills_to_create: { type: 'array', items: { type: 'string' } },
    notes: { type: 'string' },
  }, required: ['specialists', 'browsers', 'skills_to_create'],
}

// §9.1.8 RESPAWN-2 (orden del operador 2026-09-06): agente muerto (429 o CUALQUIER razón) →
// el gang SIGUE; se crean 2 reemplazos con el DOBLE de conocimiento (effort alto) y la tarea
// SUBREPARTIDA entre ambos. Jamás reintento 1:1. Si un reemplazo también muere, su mitad
// vuelve a dividirse (el helper se aplica recursivamente vía re-despacho de la oleada).
// v1.2: PERO bajo un límite SISTÉMICO del proveedor duplicar spawns amplifica el 429 —
// tripwire + presupuesto cortan la amplificación y el gang se reanuda con resumeFromRunId.
let respawnBudget = args0.respawnBudget ?? 8
let agentDeaths = 0, agentWins = 0, systemicDeclared = false
const TRIP = args0.tripDeathThreshold ?? 6
const WAVE = args0.concurrency ?? 4
const mergeHalves = (a, b) => {
  if (!a) return b || null
  if (!b) return a
  const out = {}
  for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) {
    const va = a[k], vb = b[k]
    if (Array.isArray(va) || Array.isArray(vb)) {
      out[k] = [...new Set([...(Array.isArray(va) ? va : []), ...(Array.isArray(vb) ? vb : [])].map(x => JSON.stringify(x)))].map(x => JSON.parse(x))
    } else if (va && vb && typeof va === 'object' && typeof vb === 'object') {
      out[k] = { ...vb, ...va }
    } else {
      out[k] = (va !== undefined && va !== null && va !== '') ? va : vb
    }
  }
  // Estado/veredicto: gana el PEOR (R8 — jamás maquillar hacia verde).
  const rank = { PASS: 0, DONE: 0, FE: 0, PARTIAL: 1, GAPS: 1, BLOCKED: 2, FAIL: 2 }
  if (out.status && (rank[b.status] || 0) > (rank[a.status] || 0)) out.status = b.status
  if (out.verdict && (rank[b.verdict] || 0) > (rank[a.verdict] || 0)) out.verdict = b.verdict
  out.respawn2 = true
  return out
}
const runAgent = async (prompt, opts) => {
  const first = await agent(prompt, opts)
  if (first) { agentWins++; return first }
  agentDeaths++
  if (systemicDeclared) return null
  if (agentDeaths >= TRIP && agentDeaths > 2 * Math.max(agentWins, 1)) {
    systemicDeclared = true
    log(`⛔ LÍMITE SISTÉMICO DEL PROVEEDOR: ${agentDeaths} muertes vs ${agentWins} éxitos — RESPAWN-2 y olas restantes SUSPENDIDOS (golpear un proveedor saturado solo genera más 429). Reanudar con resumeFromRunId cuando la cuota revierta.`)
    return null
  }
  if (respawnBudget <= 0) { log(`⚠ presupuesto RESPAWN-2 agotado ("${opts.label || 'agent'}" → null sin respawn — circuit-breaker v1.2)`); return null }
  respawnBudget--
  log(`⚠ RESPAWN-2: "${opts.label || 'agent'}" murió → 2 reemplazos de doble conocimiento, tarea subrepartida (el gang sigue; presupuesto restante: ${respawnBudget})`)
  const halves = (await parallel([0, 1].map(h => () =>
    agent(`${prompt}

RESPAWN-2 (orden del operador): el agente original de esta tarea murió (429 u otra razón).
Eres el REEMPLAZO ${h === 0 ? 'A' : 'B'} con el DOBLE de conocimiento del caído. Tu mitad del charter: ${h === 0 ? 'la PRIMERA mitad (ítems/fases/archivos 1..⌈n/2⌉)' : 'la SEGUNDA mitad (ítems/fases/archivos ⌈n/2⌉+1..n)'} — tu par cubre la otra mitad EN PARALELO. Ejecuta SOLO tu mitad con rigor PhD completo y el mismo schema de salida (reporta lo de tu mitad; el orquestador fusiona ambas).`,
      { ...opts, label: (opts.label || 'agent') + `-respawn-${h === 0 ? 'A' : 'B'}`, effort: 'high' })
      .then(r => { if (r) agentWins++; else agentDeaths++; return r })
  ))).filter(Boolean)
  if (!halves.length) return null
  return mergeHalves(halves[0], halves[1])
}
// v1.2 oleadas: tasa de requests acotada ESTRUCTURALMENTE (sin Date.now) — Cross/Browse/Loop
// despachan en olas de WAVE agentes; la siguiente ola sólo arranca cuando la anterior termina.
const waved = async (items, make) => {
  const out = []
  for (let i = 0; i < items.length; i += WAVE) {
    if (systemicDeclared && i > 0) { log(`⛔ sistémico: ${items.length - i} ítems de esta fase omitidos (circuit-breaker)`); break }
    out.push(...await parallel(items.slice(i, i + WAVE).map(it => () => make(it))))
  }
  return out
}

phase('Compose')
log('Componiendo el gang: leer BOARD + inventariar WOs pendientes')
const plan = await runAgent(`Eres el COMPOSER del Gang Omniscience (§9 de la skill arbitragex-omniscience). Fecha ${NOW}.
GOAL del operador: ${GOAL}
Lee ${BOARD} con Read. Lee también los reportes ya existentes en ${DIR} (WO-*-*.md, 00-PREDATOR-ROADMAP.md si existe) para NO re-despachar lo hecho.
Compon el gang más pequeño que CUBRA todo lo pendiente (min 8, máx ${args0.maxAgents || 40}):
1. specialists: un agente PhD por WO pendiente (design/apply/verify/redesign). Asigna serial_group a los que comparten archivos o el target/ de Rust: applies Rust = serial_group "rust" (serie TOTAL, §36.4); applies TS con archivos distintos = paralelos.
2. browsers: 2-4 personas-usuario que den fe en la DApp viva (${BASE}): operador que revisa opportunities/live-readiness/operations, auditor que busca datos vacíos/honestidad R8, QA que verifica WS en vivo.
3. skills_to_create: gaps de skill que el orquestador debe crear ANTES (si ninguno: []).
Roles PhD disponibles (rubrics): ecc:rust-reviewer, ecc:typescript-reviewer, ecc:security-reviewer, ecc:database-reviewer, ecc:performance-optimizer, ecc:tdd-guide, ecc:code-architect, ecc:react-reviewer + roles OMEGA (rust-topology-engineer, strategy-architect, data-analytics, frontend-architect, solidity-engineer, devops-platform, security-auditor, math-validator, economics-validator, cs-validator). En workflow-subagents la rubric va en el charter del prompt.
${RULES}`, { schema: PLAN_SCHEMA, label: 'gang:compose' })
if (!plan) { return { error: 'compose failed' } }
log(`Gang compuesto: ${plan.specialists.length} especialistas · ${plan.browsers.length} browsers · ${plan.skills_to_create.length} skills a crear`)

const roundPrompt = (s) => `Eres un agente PhD del Gang Omniscience (${s.role}). Fecha ${NOW}. WO: ${s.wo} · kind: ${s.kind}.
CHARTER: ${s.charter}
Archivos bajo TU claim (nadie más los toca): ${s.files.join(', ')}
${s.kind === 'apply' ? `APLICA el diseño ya existente en ${DIR} (lee el WO-*-DESIGN.md o WO-*-APPLY.md correspondiente ANTES de editar). Edita SOLO tus archivos claimados. Verifica: cargo check (desde backend/, target caliente) o vitest/tsc según capa. NO compiles si tu WO es design-only.` : s.kind === 'verify' ? `Verifica con evidencia: tests + lectura de código + (si aplica) navegación del dominio ${BASE} con presupuesto 5 requests.` : `Produce el diseño con diffs exactos + invariante + gate. NO edites código de producción.`}
Escribe tu reporte en ${DIR}/${s.wo}-${s.kind === 'apply' ? 'APPLY' : s.kind === 'verify' ? 'VERIFY' : 'DESIGN'}.md (o actualiza el existente) y retorna el schema.
${RULES}`

phase('Round')
const groups = {}
for (const s of plan.specialists) { const g = s.serial_group || s.wo; (groups[g] = groups[g] || []).push(s) }
log(`Round: ${plan.specialists.length} especialistas en ${Object.keys(groups).length} grupos (${Object.keys(groups).filter(k => groups[k].length > 1).join(', ')} en serie)`)
const roundResults = (await parallel(Object.entries(groups).map(([g, members]) => () =>
  (async () => { const out = []; for (const m of members) { out.push(await runAgent(roundPrompt(m), { schema: ROUND_SCHEMA, label: 'round:' + m.wo, phase: 'Round' })) } return out })()
))).flat().filter(Boolean)
log(`Round terminado: ${roundResults.filter(r => r.status === 'DONE').length}/${roundResults.length} DONE`)

phase('Cross')
const doneWos = roundResults.map(r => r.wo)
const crossResults = (await waved(plan.specialists.filter(s => doneWos.includes(s.wo)), s =>
  runAgent(`Eres el CROSS-EXAMINER par del agente que ejecutó ${s.wo} (Gang Omniscience, ${NOW}). Tu trabajo: REFUTAR su entregable.
Lee el reporte (${DIR}) y los archivos que tocó (${s.files.join(', ')}). Desafía con evidencia propia: ¿cumple el charter? ¿la verificación es real o de humo? ¿introdujo regresiones fuera de su claim? ¿RULE 00/R8 violados? Los gaps que encuentras: agent-fixable (el gang corrige) vs operator-gated (exige decisión del operador: flips/commits/VPS/capital).
${RULES}`, { schema: CROSS_SCHEMA, label: 'cross:' + s.wo, phase: 'Cross' })
)).filter(Boolean)
log(`Cross: ${crossResults.filter(c => c.verdict === 'PASS').length} PASS · ${crossResults.flatMap(c => c.gaps).length} gaps`)

phase('Browse')
const browseResults = (await waved(plan.browsers, b =>
  runAgent(`Eres ${b.persona} — un HUMANO usuario de la DApp ArbitrageX (Gang Omniscience, ${NOW}). NO eres un script: navegas con criterio propio.
JOURNEY: ${b.journey}
Herramientas: usa ToolSearch para encontrar las herramientas Playwright MCP (browser_navigate, browser_snapshot, browser_take_screenshot, browser_console_messages, browser_network_requests) y navega ${BASE}. Para WebSocket en vivo: el feed es socket.io — verifica señales de conexión en el DOM (badges/postura) y console messages, NO curl. Presupuesto HTTP: journeys, no barridos (máx ~15 navegaciones).
DAN FE como lo haría el operador: ¿la página muestra lo que dice mostrar? ¿los datos son reales (RULE 00)? ¿los estados vacíos son honestos (R8)? ¿algo está roto, desalineado o mintiendo? Captura screenshots con nombre descriptivo y lista evidencia (URL + qué se ve + por qué es correcto/incorrecto).
Escribe tu reporte en ${DIR}/BROWSE-${b.persona.replace(/[^a-z0-9]+/gi, '-')}.md y retorna el schema.
${RULES}`, { schema: BROWSE_SCHEMA, label: 'browse:' + b.persona.slice(0, 18), phase: 'Browse' })
)).filter(Boolean)
log(`Browse: ${browseResults.map(b => b.persona + '=' + b.verdict).join(' · ')}`)

phase('Loop')
let fixables = [...crossResults.flatMap(c => c.gaps), ...browseResults.flatMap(b => b.gaps)].filter(g => g.blocked_by === 'agent-fixable')
const gated = [...crossResults.flatMap(c => c.gaps), ...browseResults.flatMap(b => b.gaps)].filter(g => g.blocked_by === 'operator-gated')
let round = 0
const fixes = []
while (fixables.length > 0 && round < MAXR) {
  round++
  log(`Loop ronda ${round}/${MAXR}: ${fixables.length} gaps agent-fixables`)
  const tagged = fixables.map((g, i) => ({ ...g, _fixId: i }))
  const patched = (await waved(tagged, g =>
    runAgent(`Eres el FIXER del Gang Omniscience (${NOW}), ronda ${round}. Corrige este gap y verifica la corrección:
${g.desc}${g.fix_hint ? '\nHint del cross-examiner: ' + g.fix_hint : ''}
Lee el BOARD ${BOARD} y el reporte cross correspondiente en ${DIR} para contexto. Respeta claims de archivo: si el fix toca archivos de otro WO, documenta el conflicto en tu reporte en vez de pisarlo.
${RULES}`, { schema: ROUND_SCHEMA, label: `fix:r${round}:${g._fixId}`, phase: 'Loop' })
  )).filter(Boolean)
  fixes.push(...patched)
  const recheck = (await waved(patched.filter(p => p.status === 'DONE'), p =>
    runAgent(`Re-verifica adversarialmente el fix de ${p.wo} (Gang Omniscience ${NOW}, ronda ${round}): ${p.deliverable}. ¿El gap original quedó cerrado? ¿Sin regresiones? Reporta gaps RESTANTES (solo los que persisten). ${RULES}`, { schema: CROSS_SCHEMA, label: `recheck:r${round}:${p.wo}`, phase: 'Loop' })
  )).filter(Boolean)
  fixables = recheck.flatMap(c => c.gaps).filter(g => g.blocked_by === 'agent-fixable')
}
log(`Loop terminado: ${fixables.length} gaps sin cerrar (de haber ${MAXR} rondas máx) · ${gated.length} operator-gated`)

const expectedCross = plan.specialists.filter(s => doneWos.includes(s.wo)).length
const degradedPhases = []
if (expectedCross > 0 && crossResults.length === 0) degradedPhases.push(`cross 0/${expectedCross}`)
if (plan.browsers.length > 0 && browseResults.length === 0) degradedPhases.push(`browse 0/${plan.browsers.length}`)
return {
  goal: GOAL, board: BOARD, now: NOW,
  compose: { specialists: plan.specialists.length, browsers: plan.browsers.length, skills_to_create: plan.skills_to_create },
  round: roundResults.map(r => ({ wo: r.wo, status: r.status, verification: r.verification })),
  cross: crossResults.map(c => ({ wo: c.wo, verdict: c.verdict, gaps: c.gaps.length })),
  browse: browseResults.map(b => ({ persona: b.persona, verdict: b.verdict, evidence: b.evidence.length, report: b.report_path })),
  loop: { rounds_used: round, fixes: fixes.map(f => ({ wo: f.wo, status: f.status })), unresolved_fixables: fixables },
  operator_escalation: gated,
  circuit_breaker: { deaths: agentDeaths, wins: agentWins, respawns_left: respawnBudget, systemic_429: systemicDeclared, wave_size: WAVE },
  honest_final: degradedPhases.length > 0
    ? `DEGRADADO (R8): fases con 0 resultados (${degradedPhases.join(', ')}) — muertes masivas de agentes (429 sistémico u otra causa); NO es éxito. Reanudar con Workflow({scriptPath, resumeFromRunId}) — lo completado sale del cache`
    : fixables.length === 0
      ? 'ÉXITO: criterios verificados o todo lo restante es operator-gated con paquete listo'
      : `PARCIAL: ${fixables.length} gaps persisten tras ${MAXR} rondas (declarado R8, no maquillado)`,
}
