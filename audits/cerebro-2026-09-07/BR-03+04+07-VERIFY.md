# BR-03+04+07 — VERIFY: auditoría adversarial RULE 00 / R8 / §34.3 de los tres diffs

- **WO**: BR-03+04+07-VERIFY · kind: **verify** (READ-ONLY — 0 git write, 0 mutación VPS,
  0 compilación, 0/5 requests HTTP dominio público; 1 SSH read-only al VPS).
- **Agente**: ecc:security-reviewer (Gang Omniscience, IA OMEGA) · 2026-09-07.
- **Charter**: verify adversarial de BR-03 (cascada de oráculos) + BR-04 (anti-spam tiering,
  apply del diseño WO-06 con sus 3 MEDIUMs corregidos) + BR-07 (SIM_BACKEND, decisión WO-12).
  Corre DESPUÉS de que los tres applies existan. Rubric ecc:security-reviewer.
- **Archivos bajo claim**: `backend/token-enricher/src/` · `backend/searcher-rs/src/flood_gate.rs`
  · `backend/searcher-rs/src/opportunity_emitter.rs` · `docker/compose.prod.yml`.
- **Lexicon**: Topological Yield · Variedad de Liquidez · TLS · Decoherencia de Estado.

## 0. VEREDICTO: **BLOCKED ×3 — los applies NO existen** (baseline pre-verificado como referencia)

| WO | Veredicto | Razón en una línea |
|---|---|---|
| **BR-03** (oráculos) | **BLOCK — apply AUSENTE** | `backend/token-enricher/src/` byte-idéntico a HEAD (0 entradas en `git status`, 0 marcadores `BR-03` en el árbol). La cascada de oráculos NO fue aplicada en ningún lugar verificable (worktree, 49 worktrees, ramas, stash). |
| **BR-04** (anti-spam) | **BLOCK — apply AUSENTE** | `backend/searcher-rs/src/flood_gate.rs` NO existe; cero símbolos `FloodGate|self_pair_noop|arbx_flood_suppressed` en `backend/searcher-rs/src`; `git log --all -S self_pair_noop` → único hit es docs (`0fc286f0`, tocó SOLO `.claude/`+`audits/`). El diseño WO-06 sigue sin aterrizar. |
| **BR-07** (SIM_BACKEND) | **BLOCK — apply AUSENTE, flip operador-only PRESERVADO** | `docker/compose.prod.yml` diff vacío (0 líneas `SIM_BACKEND`/`REVM_RPC_URL`); VPS verificado read-only: `SIM_BACKEND=anvil`, `REVM_RPC_URL` ausente — NINGÚN builder ejecutó el flip. La decisión WO-12 sigue sin aterrizar como diff. |

Veredicto **BLOCKED**, no PENDIENTE: este verify corrió con presupuesto completo sobre el
árbol real; lo que faltaba exigía un builder que no entregó. Mismo patrón que
`BR-00-VERIFY.md` (BLOCKED 14:48Z) y `BR-05+06-VERIFY.md` (BLOCK por apply ausente).

**Lo que este reporte SÍ entrega** (para que el re-despacho no arranque de cero):
§3 = baseline auditado de los tres territorios con anclas file:line, §6 = checklist exacto
del re-verify post-apply (incluye el comando del gate 3 capturado textual de WO-06-DESIGN §7.3).

## 1. Sincronía de mesa redonda

- **Board leído completo**: `audits/cerebro-2026-09-07/GOAL-WORKORDERS.md` — BR-03/BR-04/BR-07
  figuran PENDIENTE en el kanban al momento de este verify.
- **Pares citados** (construyo sobre ellos):
  - `BR-00-VERIFY.md` — precedente BLOCK-by-absence; su §1.4.4 ordena que **BR-04 NO corra
    antes de que BR-00 mida post-fix** (las supresiones de flood recortarían el denominador
    y fabricarían mejora del 98.2%→<50% por mix-shift). Este verify lo re-confirma aplicable.
  - `BR-05+06-VERIFY.md` — patrón "apply ausente → BLOCK + pre-verificación como referencia".
    Mi §3 replica ese patrón para BR-03/04/07.
  - `BR-01-FORENSE-EMBUDO.md` / `BR-01-DESIGN.md` — contexto del embudo (leídos para alinear
    el mapa de compuertas; no los contradigo).
- **Upstreams de diseño** (oleada 2026-09-06): `WO-06-DESIGN.md` (anti-flood estructural,
  §7.3 = comando RULE 00 del gate 3) · `WO-12-DESIGN.md` (decisión SIM_BACKEND: revm
  in-process Tier 1/2 declarado + anvil retenido Tier 3; el flip es de configuración,
  propiedad del operador).

## 2. Evidencia de ausencia (6 vías independientes)

1. **Worktree** (`git status --porcelain`): 0 entradas bajo `backend/token-enricher/`,
   0 para `flood_gate.rs` (el archivo NO existe), 0 para `opportunity_emitter.rs`,
   0 para `docker/compose.prod.yml` (`git diff` vacío). Único diff Rust vivo =
   `lib.rs`(+4)/`main.rs`(+3)/`runtime_knobs.rs`(untracked) marcados **CB-02** — claim del
   orquestador control-board (`audits/control-board-2026-09-07/`), ajeno a BR-03/04/07,
   y sin contenido BR (leído el diff completo).
2. **Símbolos**: `grep -rn "FloodGate|flood_gate|self_pair_noop|arbx_flood_suppressed"
   backend/searcher-rs/src/` → **0 hits**.
3. **Commits**: `git log --all --grep "BR-03|BR-04|BR-07"` → **0 commits** en cualquier rama.
4. **Pickaxe**: `git log --all -S self_pair_noop` / `-S arbx_flood_suppressed` → único commit
   `0fc286f0` "chore(orchestration): gang omniscience 9.5…" cuyo diff tocó **SOLO**
   `.claude/` y `audits/` (texto del diseño WO-06 embebido en reportes — cero src/).
   `-S REVM_RPC_URL -- docker/` → 0 commits: el wiring compose de WO-12 §5.1 nunca aterrizó.
5. **Worktrees/stash**: 49 worktrees listados (nombres/ramas: gsimg/wf_*/docs/* — ninguno BR);
   5 stashes (retention, omega-money-sprint, main-WIPs viejos — ninguno BR).
6. **Marcadores**: `grep -rn "BR-03|BR-04|BR-07" backend/ docker/` → **0 hits** (la regla dura
   "diffs `// BR-XX (2026-09-07)`" no tiene nada que marcar porque no hay diffs).

## 3. Baseline pre-apply auditado (referencia obligada cuando el apply aterrice)

### 3.1 BR-03 — cascada de oráculos (estado actual del territorio)

- **Tiers existentes en `token-enricher`**: `geckoterminal_tier.rs:1-31` — tier peer de
  `dexscreener.rs`, AMBOS writers independientes y env-gated al hash `arbx:token_prices:<chain>`
  (last-writer-wins por símbolo, precios reales); **NO son reader-cascade** (el header lo
  declara explícitamente, `:21-22`). `multicall.rs:26-66` = symbol/decimals on-chain vía
  IMulticall3 (la lectura "de la cadena lo que la cadena dice"). `trustwallet.rs` = logos.
- **Dónde muere hoy el no-precio**: `opportunity_emitter.rs:380-381` (counter
  `gate_unknown_token_price` sobre label `unknown_token_price|unknown_price`) ·
  `scanner.rs:2511` (`RejectReason::UnknownTokenPrice`) · `counters.rs:64/83`.
- **R8 exigido al apply**: precio ausente = **None / no computado**, JAMÁS `0.0`
  (RULE 00 + R8). El pass-through `tier_unknown` ya existe honesto en
  `signal_tier.rs:34/177/201/336/373` (motivo `tier_unknown_execution_class`) — el apply de
  BR-03 NO debe aplastarlo ni convertir tiers desconocidos en precio cero.

### 3.2 BR-04 — anti-spam (gate RULE 00 capturado + baseline limpio)

- **Comando EXACTO del gate 3** (WO-06-DESIGN §7.3, textual):
  `git diff | grep -iE "0x3235|0x0645|0x6982|agld|xen\b|pepe"` → hits SOLO en `#[cfg(test)]`.
  Un solo hit en src/ de producción = rechazo del PR.
- **Corrido hoy** sobre el diff completo del worktree → **0 hits** (trivial: no hay diff BR).
- **Baseline de los archivos bajo claim** (mismo grep sobre el árbol):
  - `token-enricher/src/*` → 0 hits · `compose.prod.yml` → 0 hits.
  - `opportunity_emitter.rs` → 3 hits, clasificados: **L356 y L694 = doc-comments**
    (`///` ejemplos del FORMATO de razón de rechazo `"TokenNotAllowed:PEPE"` — no literales
    de filtro ni datos) · **L789 dentro de `mod tests`** (`#[cfg(test)]` empieza en L730).
    **0 literales en código ejecutable.**
  - *Advertencia al builder BR-04*: si el diff roza L356/L694, el gate-3 los mostrará fuera
    de `cfg(test)` — la interpretación correcta del gate es literal-en-código-ejecutable;
    los comentarios-doc pre-existentes no son hardcode (y no deben editarse: cambio quirúrgico).
- **Sentinels**: `0xdead` aparece SOLO en `dexscreener.rs:754`, DENTRO de
  `#[cfg(test)] mod tests` (L610) = fixture de test permitido. 0 sentinels en producción.

### 3.3 BR-07 — SIM_BACKEND (todo el arsenal ya presente en baseline)

- **Selección de backend**: `sim-ctl/src/main.rs:653-675` — default `anvil`; `revm` estricto
  opt-in con **fail-loud YA presente**: `SIM_BACKEND=revm requires REDIS_URL — RevmBackend
  reads live gas_price_wei from Redis` (anyhow hard, sin fall-through silencioso).
- **Comentario RPC-dedicado**: `sim-ctl/src/consumer.rs:5-8` — "when the full B2c env is
  present at boot (`SIM_BACKEND=revm` + `REVM_RPC_URL` + `ARBITRAGE_EXECUTOR` + `REDIS_URL`),
  the route-aware REAL pipeline runs … `execute_multistep_revm` (paper_mode=true,
  observer-only)" — PRESENTE en baseline e intacto.
- **Drain-guard fail-loud per-backend**: `sim-ctl/src/lib.rs:13-41` (SIMWIRE-02c P1-2:
  autorización PER-BACKEND, nunca OR común que drenaría el stream validado a rechazos),
  con tests `drain_guard_authorizes_per_backend_never_a_common_or` (`:67`) y
  `missing_executor_composes_with_drain_guard_refusal` (`:116`); `consumer.rs:603`.
  **INTACTO** — sim-ctl sin diff en el worktree.
- **Compose**: `docker/compose.prod.yml:157` `ANVIL_URL: http://anvil:8545`; SIN `SIM_BACKEND`
  en compose (la var vive en `.env` del VPS — coherente con WO-12 §1.2).
- **VPS (1 SSH read-only, §32/§33)**: `docker inspect arbitragex-v2-sim-ctl-1` →
  `SIM_BACKEND=anvil` explícito, `REVM_RPC_URL` AUSENTE, boot `2026-09-07T13:20:06Z`;
  `/opt/arbitragex-v2` en `e65040f1` = **merge del PR #555 (HOPS-LIVE-01)** — el deploy que el
  propio board anunciaba ("PR #555 en CI"), NO un artefacto de builders BR.
  ⇒ **El flip SIM_BACKEND=revm NO fue ejecutado por nadie: disciplina operador-only PRESERVADA.**

## 4. §34.3 INTACTO (charter §3)

- `backend/relays-client/` byte-idéntico a HEAD (`git status` vacío para el directorio).
- `MainnetRefused` ×6 verificados: `live_exec_policy.rs:37/:85/:124/:141/:154` +
  `bundle_builder.rs:475` (call-site del path de bundle). Nota de mesa: `BR-00-VERIFY.md:201`
  citó 5 — los dos reportes son consistentes (el 6º es el call-site, no el policy).
- default-deny (`ARBX_LIVE_EXEC_ENABLED != "true"`) sin cambios.
- Grep del charter sobre "los tres diffs": los diffs no existen → 0 cambios al terminus por
  construcción, verificado además por el estado limpio de relays-client.
- El flip SIM_BACKEND queda **documentado como operador-only** (WO-12-DESIGN §0: "hay
  configuración por flappear", propiedad del operador; confirmado empíricamente en §3.3).

## 5. §32/§33 — cero artefactos de deploy/mutación generados por builders

- **0 commits / 0 scripts de deploy nuevos**: los untracked del worktree son pngs de evidencia,
  reportes de audits/, y los archivos CB-02 (claim ajeno del orizonte control-board) —
  ninguno script de deploy ni mutación.
- **0 PRs BR** (sin commits fuente que los generen).
- **VPS NO mutado** por este verify: única acción = 1 SSH read-only (inspect + rev-parse).
- Límite declarado (fail-honest): no existe `gh` CLI local para enumerar PRs abiertos en
  GitHub (memoria del proyecto); la cobertura es árbol local + refs de ramas + VPS. El
  orquestador mantiene la vista de PRs (regla "PRs del orquestador al final").

## 6. Checklist de re-verify post-apply (lo que este verify exigirá al builder)

1. **Gate 3 textual** sobre el diff: `git diff | grep -iE "0x3235|0x0645|0x6982|agld|xen\b|pepe"`
   → hits solo en `#[cfg(test)]` (ver §3.2 para la clasificación doc-comment).
2. **R8**: cada camino None≠0 preservado — `tier_unknown` pass-through vivo, oráculo sin
   precio = no computado, jamás `0.0`.
3. **§34.3 grep del diff**: `relays-client|live_exec_policy|default-deny|MainnetRefused`
   → **0 líneas** cambiadas.
4. **Marcadores** `// BR-XX (2026-09-07)` en cada hunk de los tres diffs.
5. **BR-04**: NO deployar antes de que BR-00 mida post-fix (BR-00-VERIFY §1.4.4) ·
   invariante XLEN delta=0 en 10 min post-deploy (WO-06 §7.4) · counter
   `arbx_flood_suppressed_total` como única huella audible.
6. **BR-07**: tests del drain-guard verdes (`cargo test -p sim-ctl`) · comentario
   RPC-dedicado presente · flip `SIM_BACKEND=revm` SOLO documentado, JAMÁS en el diff
   del builder (operador-only).
7. **Mode-invariante §34.1** + `cargo check/clippy/fmt` del workspace afectado
   (searcher-rs, token-enricher, sim-ctl según toque).

## 7. Presupuesto y reglas

- HTTP dominio público: **0/5** usados (toda la evidencia es árbol local + 1 SSH read-only).
- 0 git write · 0 mutación VPS · 0 compilación (no hay diff que compilar; el mismo árbol pasó
  `cargo test` 58/0/2 hoy en BR-00-VERIFY §4).
- Este reporte es público para la mesa: los re-despachos de BR-03/BR-04/BR-07 deben recibir
  §3 (baseline) + §6 (checklist) como parte de su charter.
