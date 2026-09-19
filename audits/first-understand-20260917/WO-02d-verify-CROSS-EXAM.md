# WO-02d-verify · CROSS-EXAMINATION — veredicto: SOSTENIDO con 2 defectos factuales menores

> kind: cross-exam · par refutador del agente WO-02d-verify · 2026-09-17.
> Objetivo: REFUTAR `02d-RELAYS-CLIENT-SHARED.md` + `WO-02d-DESIGN.md` +
> `WO-02d-verify-VERIFY.md`. Método: re-apertura INDEPENDIENTE de ~25 citas
> file:line, greps propios, conteos propios. Cero cargo/git/VPS (restricción board).

## 0. Sincronía de mesa (estado al inicio de este cross-exam)

- Board `GOAL-WORKORDERS.md` leído. Pares publicados: 02a, 02b, 02c, 02-VPS-REMAP,
  WO-02a-verify-VERIFY, WO-02a-verify-CROSS-EXAM, WO-02b/02c-verify.
- Leídos para contraste: `WO-02a-DESIGN.md` (§6.1/§6.3), `02c-SIM-STACK-SELECTOR.md`
  (§carrier #567, execution_admission), `02b-OPERADORES-MATH-ENGINE.md` (grep terminus).

## 1. Intentos de refutación que FALLARON (el entregable resiste)

Todas las afirmaciones de seguridad del terminus fueron re-abiertas y son EXACTAS:

| Claim bajo ataque | Mi evidencia independiente | Resultado |
|---|---|---|
| Default-deny exact-"true" + allowlist Sepolia-default | `live_exec_policy.rs` leído completo: :3, :37, :41-52, tests :59-96 — todos exactos (yo conté 97 líneas, coincide) | ✅ |
| `MainnetRefused` NO existe en Rust | grep `MainnetRefused --include=*.rs` en backend/ = CERO hits; solo TS (control-board.ts:94,173,742; control-board-drift.ts:90) + CLAUDE.md + docs/audits | ✅ CONFIRMADO |
| Enforcement dual boot+pre-firma | `main.rs:172-188` (bail exacto en rango), `bundle_builder.rs:57-59` primera statement de `build_and_sign` | ✅ |
| Tests policy bundle_builder :414-436 | leído: default-deny, ChainNotAllowed para 137 y 1, mainnet explícito `Some("1")` OK — antes de resolver FLE | ✅ |
| op_origen del terminus = MEV-01(8)+3 base | `plan_validation.rs:17-32` leído: matches! exacto con los 11 ids citados | ✅ |
| OMEGA SEAL searcher | `searcher-rs/src/main.rs:303-343`: 8 keys de capital, panic en boot | ✅ |
| Grafo inverso validated_plan (3 productores) | `scanner.rs:2449` (SET EX 300), `candidate_simulation.rs:582` (ver §3 abajo), `canonical_plan_consumer.rs:41` (CARRIER_KEY_PREFIX, comentario documenta el triángulo scanner→carrier→relays); consumidor `submit_engine.rs:445-450` fail-closed | ✅ con MATIZ (§3) |
| R-0001 primera statement | `submit_engine.rs:92-112` exacto; espejo `searcher-rs/src/persistence.rs:53` exacto | ✅ |
| Caps económicos | `execution_admission.rs:185-201`: three-gas (`gas*3`) y `capital_usd/50` exactos; `config.rs:147-168` defaults paper=true, 1.0 ETH, 2.0 gwei (marcador WO-04 presente) | ✅ |
| `resolve_flashloan_executor_address` fail-closed | `chains.rs:699-714` Missing/Invalid/Zero exactos | ✅ |
| `docs/EXECUTION_MODES_DOCTRINE.md` ausente | `ls`/`find docs -iname *EXECUTION_MODES*` = negativo | ✅ GAP correcto |
| objetivo_usd RULE 00 | canary $350/5 WETH no existe en runtime (grep); distinción doctrina↔runtime correcta | ✅ |
| Corrección 1 del verify (02a cita) | 02a:237 dice `active_evaluate_and_emit (:964)`; 02a:360 lista `cartridge_boot.rs:...964; runner.rs:290` — la corrección es justa | ✅ |

**La verificación del WO-02d-verify NO es de humo**: re-abrió citas reales y sus
2 correcciones son legítimas. 02c corrobora INDEPENDIENTEMENTE el mismo carrier
(`02c-SIM-STACK-SELECTOR.md:127`: "Mismo carrier que relays-client lee en admisión
live (submit_engine.rs:161,442-445 — coherencia producer/consumer/terminus)").

## 2. DEFECTO 1 — conteo "33 archivos shared-rs" es FALSO (24 reales)

- Ficha §3 titular: "FICHAS shared-rs (módulo-tipo, **33 archivos**)".
- Board (GOAL-WORKORDERS.md:29-30) propagó: "**52 fichas: 19 relays-client + 33
  shared-rs**".
- Mi conteo reproducible: `find shared-rs/src -name "*.rs" | wc -l` = **24**
  (incluye `oracle_snapshot/configured_rpc.rs`); +1 test fuera de src = 25. No hay
  lectura posible que dé 33.
- reglas-client "19 archivos" = correcto (verificado).
- Impacto: menor (no afecta ninguna conclusión de seguridad), pero es un número
  presentado como "reproducible con wc -l" que NO reproduce — defecto de honestidad
  del ENTREGABLE (espíritu RULE 00/R8 aplicado al propio reporte), y ya contaminó
  el board. El verify (PASS) no lo detectó.

## 3. DEFECTO 2 — TTL del carrier validated_plan NO es uniforme 300 s

- Ficha §4: "Key `arbx:validated_plan:<id>` (**TTL 300 s**)".
- Realidad: `scanner.rs:2449+` escribe `EX 300`, pero `candidate_simulation.rs:582-586`
  escribe el MISMO key con **`EX 60`** (leído: `.arg("EX").arg(60)`).
- Consecuencia material para el análisis fail-closed del terminus: la ventana
  detect→broadcast para planes escritos por la ruta candidate_simulation es 60 s,
  no 300 — si el terminus cae detrás >60 s sobre ese productor, el plan expira y
  el fail-closed dropea (comportamiento SEGURO, pero la ficha describe mal la
  ventana). 02c solo cita el TTL 300 del carrier sim-ctl, así que tampoco lo pescó.
- Impacto: precisión documental; la conclusión de seguridad (fail-closed) queda
  INTACTA — el error subestima la tasa de drop esperable de esa ruta.

## 4. Observaciones menores (no bloqueantes)

1. **Granularidad de prueba desigual en §3 shared-rs**: varias filas citan solo
   ":1-8" (headers) — p.ej. `metrics.rs` (481 LOC) fichado con prueba ":1-8 c/u".
   El gate del board exige "línea exacta"; para módulos-transversales grandes la
   prueba es de header, no de misión. rpc_failover/chains/flashloan_math sí tienen
   citas sustanciales. Mitigado porque son módulos de infra, no del terminus §34.3.
2. **Chain-of-custody del adendum**: el WO-02d-verify editó el archivo auditado
   (`02d-RELAYS-CLIENT-SHARED.md`) para inyectar su propio dictamen PASS dentro del
   entregable del auditado. Divulgado y sin fabricación, pero un lector futuro no
   puede diferenciar limpiamente claim original vs veredicto del verificador.
   Preferible: adendum = solo puntero al archivo VERIFY.
3. **02b cross-check sigue ABIERTO**: 02d-DESIGN pidió a 02b contrastar si otros
   operadores llegan al terminus (`plan_validation::supports_strategy`). 02b ya está
   publicado y grep propio = CERO menciones de plan_validation/terminus en
   02b-OPERADORES-MATH-ENGINE.md. La pregunta de mesa quedó sin responder.
4. **Cero regresiones confirmado**: `git diff --stat -- backend/relays-client
   backend/shared-rs` = vacío; `git status` no muestra nada nuevo atribuible al WO
   (los M preexistentes — route_intent.rs, settings, etc. — ya estaban en el
   snapshot inicial de la conversación). Restricción no-cargo/git cumplida.

## 5. Dictamen

**El entregable SOSTIENE su núcleo**: el cuadro §1 (drift MainnetRefused, allowlist
explícito, no blacklist), el invariante del DESIGN, el grafo inverso y la disciplina
RULE 00 son fieles al código — lo verifiqué de forma independiente, no confío en el
verify. Los 2 defectos (conteo 33≠24, TTL 60≠300) son errores factuales del
documento que el gang debe corregir antes de que el board los consuma como canónicos;
ninguno toca el default-deny, ninguno exige acción de VPS/capital.

### Clasificación de gaps

| Gap | blocked_by |
|---|---|
| Corregir "33 archivos"→24 en 02d-RELAYS-CLIENT-SHARED.md §3 y en el resumen DONE del board (GOAL-WORKORDERS.md:29) | agent-fixable |
| Corregir "(TTL 300 s)"→"TTL 300 s (scanner/canonical_plan_consumer) / 60 s (candidate_simulation.rs:586)" en §4 | agent-fixable |
| Responder el cross-check 02b (¿otros operadores llegan al terminus?) — publicarlo en el board | agent-fixable (próxima oleada) |
| D1 remediación documental del drift (api-server O biblioteca skills, una superficie, P-∅) | operator-gated |
| D2 crear docs/EXECUTION_MODES_DOCTRINE.md o corregir la referencia en CLAUDE.md §34 | operator-gated |
| D3 materializar canary §34.5 (ARBX_LIVE_PRINCIPAL_CAP_* + capital_usd en VPS) | operator-gated (requiere cambio de env/VPS) |
