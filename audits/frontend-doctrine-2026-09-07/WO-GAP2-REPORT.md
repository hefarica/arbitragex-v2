# WO-GAP2 — Espejo backend R8: roi_pct placeholder `Some(0.0)` en ramas de rechazo del orchestrator

> FIXER Gang Omniscience ronda 2 · 2026-09-08/09 · Diffs marcados `// WO-GAP2 (2026-09-07)`.
> Reglas respetadas: RULE 00/R8 · §32/§33 (CERO interacción con VPS esta ronda — el gap
> estaba plenamente adjudicado por fuentes vivas de los pares; 0 requests HTTP) ·
> §34.3 intacto (ni toqué relays-client) · NO-GIT (0 commit/push/PR/deploy — edición
> local + verificación SOLAMENTE).

## §0 Veredicto en una línea

**GAP-2 CERRADO local**: las 5 ramas de rechazo del spine-gate en `process_candidate`
ya NO escriben `roi_pct = Some(0.0)` — todas estampan `None` (R8: no computado) vía un
helper único `apply_gate_rejection_fields`, con test de regresión que ejecuta el código
REAL del helper (no una re-implementación). `cargo check` PASS + 38/38 tests del módulo
PASS (incluida la regresión nueva). Pendiente operador: PR propio + deploy (NO-GIT).

## §1 Contexto de mesa (a quién construyo encima — todo verificado, no asumido)

- **WO-G-6-REPORT.md §2** (ronda 1, FIXER): declaró la dependencia con evidencia de
  código — `orchestrator.rs:1242,1269,1297,1332,1377` escriben el placeholder;
  `:1358` es el único `Some(outcome.net_roi_pct)` computado. "La cura correcta es
  backend (`None` en esas ramas, o un flag `roi_computed`). Queda registrado para la
  mesa — es el espejo exacto de G-6 del lado emisor."
- **WO-G-6-REVERIFICATION-R2.md §1** (verificador adversarial): confirmó por TRES
  fuentes vivas que el placeholder NO llega al wire hoy (Redis 0/10,004 · PG 0/47,818
  en 15 min · endpoint exacto del ticker 0/20) → latente, no activo; y §5 tabla lo
  marcó **agent-fixable**. Mi WO ejecuta exactamente esa adjudicación.
- **WO-G-6-REVERIFICATION-R2.md §2.8**: re-leyó las 5 líneas y `:1358` — cifra que
  reproduje yo mismo (lectura directa, abajo §2). Cero contradicciones con mis pares.
- **Chain de código del wire** (re-verificada por mí, CANONICAL_REPO):
  `shared-rs/src/contracts.rs:76` `pub roi_pct: Option<f64>` sin
  `skip_serializing_if` → `None` serializa `null`; `opportunity_emitter.rs:438-447`
  `emit_rejected` clona el opp y NO toca `roi_pct`; `persistence.rs:151,175` bind
  directo (`roi_pct` PG es nullable — `api-server/test/migrations.test.ts:48` dropea
  NOT NULL). **`None` fluye intacto a PG + `arbx:opps:detected` + endpoint** — el
  contrato ya es null-ready end-to-end (el wire vivo lo prueba: 100% null hoy).

## §2 El defecto (leído por mí, no heredado)

`backend/searcher-rs/src/orchestrator.rs`, método `process_candidate`, match sobre
`ConfigGateOutcome` + bloque MacroMevGate — 5 ramas con el patrón idéntico:

| Línea pre-fix | Rama | Estado del evaluador al llegar ahí |
|---|---|---|
| :1242 | `TokenNotAllowed` | jamás corrió (allowlist gate) |
| :1269 | `StrategyDisabled` | jamás corrió |
| :1297 | `StrategyConfigGateBlocked` | jamás corrió |
| :1332 | `EvaluatedRejected` | corrió y rechazó (math gate) |
| :1377 | `MacroMevGate` reject | corrió, PASÓ el spine (`:1358` pre-fix ya escribió `Some(net_roi_pct)`), y el gate energético la RECHAZÓ por divergencia — el placeholder además DESTRUÍA un valor computado |

El sitio computado (`opp.roi_pct = Some(outcome.net_roi_pct)`, `:1358` pre-fix →
`:1354` post-fix) queda como único que publica un ROI medido. **Violación**: R8
fail-honest ("None = no computado,
Some(0.0) = computado y exactamente cero") + RULE 00 (placeholder no-decorativo a
medio camino: es un cero FABRICADO presentado como medido). Daño latente: el FE
post-G-6 renderiza `roi_pct` verbatim → "+0.00%" con flecha ▲ para una fila que
nunca midió nada (exactamente el bug del ticker que G-6 curó del lado cliente).

**Precedente canónico dentro del mismo crate** (mi cura lo sigue, no lo inventa):
`scanner.rs:2505-2521` — "Some(0.0) would mean 'we computed the profit and it is
exactly zero' — a distinct semantic" → `roi_pct = None // R8: not computed`.

## §3 El fix aplicado (diff propio, `// WO-GAP2 (2026-09-07)`)

Archivo ÚNICO: `backend/searcher-rs/src/orchestrator.rs`. Contención verificada:
`WO-GAP2` aparece solo ahí (grep backend+frontend); ningún archivo de otro WO tocado.

1. **Helper puro `apply_gate_rejection_fields(opp, reason)`** (sección Utilities,
   junto a `detection_source_as_str`): fija `rejection_reason = Some(reason)`,
   **`roi_pct = None`** (R8), `risk_score = Some(0.0)` (convención rejected-row
   pre-existente, ver §5). Doc-comment completo con la semántica GAP-2↔G-6.
2. **Las 5 ramas** delegan al helper (3 líneas cada una → 1 llamada + marker). La
   rama EvaluatedRejected CONSERVA su propagación R8 de `net_expected_profit_usd`
   (`expected_profit_usd.map(|g| g - outcome.gas_cost_usd)`) — fuera de scope, intacta.
3. **Rama MacroMevGate**: comentario explícito de la decisión semántica — el spine
   SÍ computó `net_roi_pct` (:1354 post-fix), pero la trayectoria divergió (E_state ≥ τ);
   su figura marginal NO se reporta. `None` mantiene la fila honesta en el wire
   (dirección del operador: "None en esas 5 ramas — jamás Some(0.0) no-computado").
4. **Test de regresión `r8_gate_rejection_roi_none`** (módulo `orchestrator::tests`,
   junto a `r8_none_gross_preserved` por convención temática): precondición
   `roi_pct = Some(1.23)` (el estado EXACTO que ve la rama MacroMevGate tras su
   asignación computada)
   → el helper DEBE sobreescribirla a `None`; pina reason verbatim, convención
   risk_score, e idempotencia (segundo stamp gate-after-spine). **Por qué helper y
   no 5 edits inline**: `process_candidate` requiere Redis/config vivos (inaccesible
   en unit tests del crate — el módulo de tests existente es estructural); con el
   helper la regresión ejecuta el código REAL que las 5 ramas llaman, y los 5 sitios
   no pueden derivar. 5 usos = no es abstracción single-use.

**Qué NO toqué deliberadamente** (disciplina quirúrgica + P-∅ "un PR = UN ID"):
- `risk_score = Some(0.0)` en las mismas ramas: MISMO patrón R8 en principio, pero es
  convención cross-crate coherente (scanner.rs:2222/2269/2319/2531, orchestrator,
  emitter tests) con una doctrina documentada detrás (scanner.rs:2199-2204 "rejection
  volume" transparency). Su adjudicación merece anomalía propia — la registro en §5.
- Los sitios HERMANOS `scanner.rs:2221,2268,2318` (path legacy, mismo patrón, mismo
  comentario-doctrina): fuera del scope adjudicado (GAP-2 = orchestrator.rs).
  Revertirlos es revertir una decisión documentada — le pertenece a la mesa, no a
  este ride-along. Registrados en §5.
- `cartridge_boot.rs:1644` es un `Some(outcome.net_roi_pct)` computado (legítimo,
  espejo de `:1358`) — sin cambio.

## §4 Verificación (ejecutada, no declarada)

- `cargo check -p searcher-rs` (workspace `backend/`, árbol principal con target/
  caliente) → **PASS** (`Finished dev profile`, exit 0; primera corrida 7m38s y
  re-check 6m58s, ambos incluyendo espera de file-lock con un peer compilando en
  paralelo — el re-check cubre el estado final tras el retoque cosmético de
  comentarios).
- `cargo clippy -p searcher-rs --lib --tests` → **PASS** (exit 0, `Finished dev
  profile in 40m 40s`; salida filtrada por `warning|error|orchestrator` — solo
  imprimió `Finished`, i.e. **0 warnings, 0 errors** en el crate completo incl.
  tests). Corre sobre el estado FINAL del archivo (cubre también el último cambio
  de comentario — clippy recompila todo, así que el estado final tiene
  verificación de compilación + lint propia).
- `cargo test -p searcher-rs --lib orchestrator::` → **38 passed / 0 failed**
  (incluye `r8_gate_rejection_roi_none ... ok`; el filtro atrapó también
  `sim_orchestrator` y `thermodynamics::carnot_orchestrator` — bonus, todo verde).
- `cargo fmt -- src/orchestrator.rs` → aplicado SOLO a mi archivo (ejecutar
  `cargo fmt -p` habría reformateado los 7 archivos modificados por peers en el
  árbol compartido — prohibido pisar). Post-fix: markers movidos a línea propia
  arriba de la llamada (rustfmt alienaba los comentarios `// TASK 3` pre-existentes
  a la columna del trailing comment).
- Contención: `git diff --name-only` → mi WO modifica EXCLUSIVAMENTE
  `backend/searcher-rs/src/orchestrator.rs` (79→~85 líneas netas). Peers activos
  en el árbol durante mi ronda: `lib.rs`, `main.rs`, `opportunity_emitter.rs`,
  `canonical_knobs.rs`, `pool_discovery.rs`, `reserves.rs`,
  `workers/route_scanner_worker.rs` (M) + `priors_cache.rs`, `runtime_knobs.rs`
  (untracked) — NINGUNO tocado por mí; `cargo check` verde implica que el árbol
  combinado compila.
- SIN VPS esta ronda: las tres fuentes vivas del gap ya estaban adjudicadas por el
  verificador ronda 2 (§1); re-query sería gasto de presupuesto sin información
  nueva. 0 requests HTTP dominio público.

## §5 Registro para la mesa (anomalías hermanas — NO cerradas por este WO)

| # | Hallazgo | Clasificación | Dueño sugerido |
|---|---|---|---|
| GAP2-a | `risk_score = Some(0.0)` placeholder en las MISMAS 5 ramas (y scanner.rs legacy): mismo argumento R8 aplica ("riesgo computado y exactamente cero" para filas jamás scoreadas). Convención coherente hoy — cambiarla es decisión de producto con blast-radius FE/PG. | CANONICAL_REPO | mesa ronda 3 |
| GAP2-b | `scanner.rs:2221,2268,2318` (path legacy) escriben el mismo `roi_pct = Some(0.0)` con doctrina explícita de "rejection volume" (:2199-2204). Hoy 0 filas con 0.0 en el wire viven (verificador ronda 2), pero si ese path revive, el placeholder revive. Requiere REVERTIR una decisión documentada — no un ride-along. | CANONICAL_REPO | mesa ronda 3 |
| GAP2-c | Fix NO deployed (NO-GIT): igual que GAP-1 de G-6, ambas curas (FE fiel + emisor honesto) aterrizan juntas cuando el operador reanude GIT→VPS. Hasta entonces el sistema deployed sigue siendo el viejo. | operator-gated | operador |

## §6 Composición con los pares (cero conflicto)

- **G-6** (ticker FE): composición exacta espejo — ellos volvieron el FE fiel
  (`roi_pct ?? null`), yo vuelvo el emisor honesto (`None`). Juntos cierran el
  circuito R8 end-to-end: `None` emisor → `null` wire → "—" / "roi_pct no computado
  (R8)" en pantalla (`OpportunitySummaryGrid.tsx:102` ya renderiza ese string para
  null — el FE estaba esperando este contrato).
- **WO-H4** (`schemas.ts` + `window_total`): aditivo/ortogonal, sin intersección.
- **HP-05 / CB-03** (untracked, reportados por verificador ronda 2 §3): sin
  intersección con orchestrator.rs; no evaluados aquí.

— FIXER GAP-2 · Gang Omniscience ronda 2 · 2026-09-08
