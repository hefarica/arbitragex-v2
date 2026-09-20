# WO-02b-FIX-TERMINUS — 2026-09-17 · fixer gang ronda 1 · GAP G-2 del cross-check de mesa

> Charter: cerrar el GAP #3 declarado por el cross-examiner de 02d
> (`WO-02d-verify-CROSS-EXAM.md` §4.3: "02b cross-check sigue ABIERTO") y responder
> la pregunta de `WO-02d-DESIGN.md` §"Notas para la mesa" (:108-111):
> ¿el whitelist de 11 strategy_kinds (`relays-client/src/plan_validation.rs:17-32`)
> es el cuello de botella que descarta los demás cartuchos, o el mapa
> operador→cartucho de 02b dice lo contrario?
> Reglas respetadas: RULE 00 (todo re-derivado, nada heredado), §32/§33 read-only,
> §34.3 intocado, NO-GIT, restricción extra del board (cero cargo/npm/build —
> análisis por lectura/grep SOLAMENTE). Cero VPS/HTTP (0/5 requests).

## 1. Respuesta corta (para el board)

**El whitelist ES el cuello de botella — terminal, no upstream. El mapa de 02b NO
dice lo contrario: es compatible y lo confirma.** El wiring upstream (operador↔cartucho)
cubre 264/264 cartuchos sin filtrar por kind; la primera compuerta estrecha de kind sobre
el espacio de 264 es `supports_strategy` en el terminus, que admite exactamente **8
stems de cartucho (3.0%)** → **256/264 (97.0%) descartados estructuralmente en el terminus**.
Precisión aritmética: el "253" del charter (264−11) trata los 3 kinds genéricos como
cartuchos; `dex_arb`/`flashloan_arb`/`triangular` NO son stems de ningún .rhai de los
264 (verificado `whitelist − stems = {dex_arb, flashloan_arb, triangular}`).

## 2. Método (reproducible, RULE 00)

Parser regex sobre los 264 `.rhai` de `backend/searcher-rs/cartridges/strategies/`:

```python
pat_p = re.compile(r'Primary operators[^\[]*\[([0-9,\s]*)\]')
pat_s = re.compile(r'Secondary operators[^\[]*\[([0-9,\s]*)\]')
whitelist = {  # plan_validation.rs:17-32, transcripto literal
 "dex_arb","flashloan_arb","triangular",
 "mev_01_001_dex_dex_arbitrage","mev_01_002_cross_pool_arbitrage",
 "mev_01_008_amm_amm_arbitrage","mev_01_015_two_leg_arbitrage",
 "mev_01_016_triangular_arbitrage","mev_01_017_quadrangular_arbitrage",
 "mev_01_018_n_leg_cyclic_arbitrage","mev_01_019_multi_hop_arbitrage",
}
# por cartucho: stem (== strategy_kind, contracts.rs:34) ∈ whitelist ?
# por op: unión de cartuchos que lo declaran primary|secondary; intersección con admitidos
```

Salida del cruce (op | cartuchos any | primary | admitidos | llega al terminus):

```
op_01 |  27 |   0 | 8 | YES(sec-only)   op_16 | 113 |  44 | 8 | YES
op_05 |  24 |  22 | 0 | NO              op_17 |   1 |   1 | 0 | NO
op_06 |  30 |  30 | 0 | NO              op_19 |  56 |  55 | 0 | NO
op_07 |  30 |  28 | 0 | NO              op_20 |  19 |   4 | 0 | NO
op_08 | 175 | 131 | 0 | NO              op_21 | 178 | 176 | 8 | YES
op_10 |  80 |  45 | 0 | NO              op_22 | 247 |  16 | 8 | YES
op_11 | 170 |  36 | 0 | NO              op_23 | 111 |  64 | 0 | NO
op_13 | 149 |  71 | 0 | NO              op_24 |  23 |  14 | 0 | NO
op_14 |  14 |   4 | 0 | NO              op_25 |  12 |  12 | 0 | NO
op_15 |  65 |  54 | 8 | YES             op_26 |  33 |   7 | 8 | YES
op_27 |  71 |  53 | 8 | YES             op_29 |  16 |  16 | 0 | NO
op_30 |  27 |   0 | 8 | YES(sec-only)
```

- Ops con ≥1 cartucho admitido (8): **op_01, 15, 16, 21, 22, 26, 27, 30**.
  Nota: op_01 y op_30 tienen 0 primary en los 264 (coincide con §2 de la ficha 02b);
  su vía al terminus es SOLO evidencia secondary de los 8 admitidos.
- Ops 100% fuera del terminus (15): **op_05, 06, 07, 08, 10, 11, 13, 14, 17, 19,
  20, 23, 24, 25, 29** — ninguno de sus cartuchos (primary NI secondary) pasa el
  whitelist. Su señal vive solo upstream (Redis `arbx:math_evidence:*` /
  `strategy_evidence_key`) y en scoring paper.
- Ops nunca declarados en .rhai (9: 02, 03, 04, 09, 12, 18, 28, 31, 32-registro):
  doblemente fuera (ni cartucho ni terminus) — coincide con G5 de la ficha 02b.

## 3. Evidencia de cuello de botella TERMINAL (no upstream)

1. **Identidad kind==stem**: `shared-rs/src/contracts.rs:34` ("A cartridge identity —
   the `.rhai` filename stem (a canonical strategy_kind)"); emisión real
   `cartridge_boot.rs:1179` `strategy_kind: StrategyKind::cartridge(cartridge_id.clone())`.
2. **Sin whitelist antes del terminus**: el único filtro de kind aguas arriba es
   `sim-ctl/src/tx_builder.rs:65-66 + :144-147` (`is_non_swap_strategy_kind` rechaza
   SOLO `liquidation`/`liquidation_snipe`; el comentario :140-142 declara que es para
   evitar build errors de router engañosos, no una política de admisión).
3. **Los 3 call-sites del whitelist, todos en relays-client** (grep backend completo):
   `bundle_builder.rs:60` (pre-broadcast, `UnsupportedStrategy`),
   `execution_admission.rs:23` (`refresh`, Err `execution_surface_adapter_required`),
   `plan_validation.rs:182` (`validate_binding`, Err `unsupported_strategy`).
   Paper/shadow NO lo invoca → el descarte aplica SOLO al terminus de ejecución
   (§34.1.3: los modos difieren solo en el terminus — el hot-path mode-invariant no
   se viola; la detección/simulación/scoring de los 264 sigue íntegra).
4. **Causa raíz del whitelist** (INFERRED del código): la superficie de calldata que
   el terminus sabe validar — wrapper TLS `REQUEST_FLASH_LOAN_SELECTOR` con inner
   `EXECUTE_ARBITRAGE_FLASH_FUNDED` y routers forward/backward
   (`plan_validation.rs:53-80`, selectors importados de prioritization_spine :6-8).
   Extender a más kinds = nueva superficie de adapter = WO propio con diff,
   operator-gated (NO propuesto aquí; consistente con la doctrina P-∅).
5. **Las 3 familias genéricas = productores legacy HUEHUFO**: `flashloan_arb` lo emite
   `engines/flashloan_engine.rs:26/:348`; `triangular`/`dex_arb` vía label mapping
   `cartridge_boot.rs:498,785-790` — el stack legacy que WO-02a clasificó default-OFF
   post-Phase-15. El `"triangular"` de `candidate_simulation.rs:660` es fixture
   `#[cfg(test)]` (mod tests abre :599). La ficha 02d §F-06 ya lo decía
   ("8 cartuchos MEV-01 + 3 familias base") — este cruce lo confirma desde el lado 02b.

## 4. Relación con los pares (sincronía de mesa)

- **02d-DESIGN :108-111**: pregunta RESPONDIDA (arriba). El "gap está aquí (cuello de
  botella real del pipeline, no upstream)" de 02d se CONFIRMA con el cruce cuantificado.
- **02d-RELAYS-CLIENT-SHARED.md :105/:140-142**: consistente (multi-dex restringido a
  8+3); este fix añade la vista por operador que faltaba.
- **WO-02d-verify-CROSS-EXAM.md §4.3**: GAP cerrado. Su tabla "agent-fixable (próxima
  oleada)" queda servida.
- **02c**: su mitad ya estaba cerrada (cita y difiere a 02d — verificado: no toca el
  mapa operador). Sin contradicción.
- **02b §2/§3/§9**: el mapa operador→cartucho que este cruce consume NO cambia;
  los conteos p/s por operador ya estaban doble-verificados (FIX-MATHMAP/FIX2).
  La reclasificación de "ACTIVO de wiring" (02b §7:200-201) NO cambia: ese destino
  describe wiring de evidencia, no alcanzabilidad del terminus — la diferencia la
  introduce esta adenda (§12 de la ficha): 15 de los 23 ops declarados son
  wiring-activos PERO terminus-muertos.

## 5. Cambios aplicados

| Archivo | Cambio | Marcador |
|---|---|---|
| `02b-OPERADORES-MATH-ENGINE.md` | §12 append-only (respuesta a la mesa) | `// WO-02b-FIX-TERMINUS (2026-09-17)` |
| `GOAL-WORKORDERS.md` | entrada de discrepancia bajo WO-02b | `// WO-02b-FIX-TERMINUS` |
| `WO-02b-FIX-TERMINUS-20260917.md` | este reporte | — |

Cero archivos de otros WO pisados (02d/02c/02a intactos; el adendum vive en el
entregable del propio 02b, misma disciplina de FIX-N1/FIX-DRIFT-COMPOSE).
Cero .rs/.yml mutados. Cero git/cargo/npm/VPS/HTTP (0/5 requests).

## 6. Verificación post-edit

- `grep -n "supports_strategy\|terminus" 02b-OPERADORES-MATH-ENGINE.md` → solo §12.
- `grep -c "## 12" 02b-OPERADORES-MATH-ENGINE.md` → 1 (sin duplicados).
- Conteos re-ejecutados dos veces (parser idéntico, salida idéntica).
- `git status --porcelain -- backend/` → sin cambios nuevos atribuibles a este WO
  (solo el diff huérfano route_intent.rs preexistente, ya clasificado por ERRATA-WO-02a).

## 7. VERIFICACIÓN DUAL INDEPENDIENTE — // WO-02b-FIX-TERMINUS-VERIFY (2026-09-17, fixer gang ronda 1, respawn paralelo)

> Despachado con el MISMO charter (respawn/convergencia del gang). Al encontrar el
> entregable ya publicado, este fixer NO duplicó: re-derivó TODO con parser propio
> (RULE 00, cero herencia) y registra la convergencia. Mismas reglas: cero
> cargo/npm/build, cero VPS/HTTP (0/5), cero .rs tocados, NO-GIT.

Convergencia COMPLETA — las 23 filas de la tabla §2 reproducidas idénticamente por
script independiente (glob+regex distinto, sin reutilizar el parser del par):

- 264 .rhai confirmados en `backend/searcher-rs/cartridges/strategies/` (wc -l).
- Stems admitidos por el whitelist = **8** exactos; `whitelist − stems =
  {dex_arb, flashloan_arb, triangular}` confirmado (los 3 kinds genéricos NO son
  stems de ningún .rhai) → 256/264 descartados en terminus. Coincide §1.
- Ops que llegan al terminus = **{01, 15, 16, 21, 22, 26, 27, 30}**, con los mismos
  4 flags sec-only (01, 22, 26, 30) y los mismos conteos any/prim por op (23 filas,
  0 diffs).
- Ops terminus-muertos = **{05, 06, 07, 08, 10, 11, 13, 14, 17, 19, 20, 23, 24,
  25, 29}** (15) — idéntico.
- Ops nunca declarados = **{02, 03, 04, 09, 12, 18, 28, 31, 32}** (9) — idéntico.

Evidencia de código re-verificada byte-exact por este fixer:

- `plan_validation.rs:17-32` — whitelist de 11 kinds transcripto literal y cotejado.
- `searcher-rs/src/cartridge_boot.rs:1179` — `strategy_kind:
  StrategyKind::cartridge(cartridge_id.clone())` (identidad kind==stem).
- `shared-rs/src/contracts.rs:8-9,:34` — doc-comment "each cartridge IS a canonical
  strategy_kind" / ".rhai filename stem".
- `sim-ctl/src/tx_builder.rs:144-147` — `is_non_swap_strategy_kind` rechaza SOLO
  `liquidation`/`liquidation_snipe` (único filtro de kind upstream; comentario
  :138-142 confirma propósito build-error, no admisión).
- `grep -rn supports_strategy backend --include=*.rs` = 5 hits: los **3 call-sites
  de producción** citados (bundle_builder.rs:60, execution_admission.rs:23,
  plan_validation.rs:182) MÁS **2 asserts `#[cfg(test)]`** (plan_validation.rs:478-479,
  `triangular()` admitido / `liquidation()` rechazado). Precisión sobre §3.3: "3
  call-sites" = correcto en producción; los 2 hits restantes son tests, no vías de
  descarte. El claim de fondo (todas las vías productivas viven en relays-client)
  SOBREVIVE.
- `searcher-rs/src/engines/flashloan_engine.rs:24-28` + doc ~:345-350 — productor
  legacy de `flashloan_arb` (FlashloanArb override) confirmado.
- `searcher-rs/src/cartridge_boot.rs:498` ("triangular" por n-legs) y `:783-792`
  (label mapping dex_arb/flashloan_arb) confirmados.
- `git status --porcelain` — sin mutaciones nuevas (solo diff huérfano
  route_intent.rs preexistente + untracked audits/).

Hallazgo estructural ADICIONAL de la verificación (nuevo, menor): los 8 stems
admitidos son **homogéneos en wiring** — todos declaran exactamente Primary
{15, 16, 21, 27} y Secondary {01, 22, 26, 30}. La superficie operador visible en
el terminus es por tanto un bloque único de 4 primary + 4 secondary, idéntico para
los 8 cartuchos MEV-01 admitidos (clasificación: INFERRED del cruce .rhai×whitelist;
consistente con que los 8 comparten familia y plantilla).

Veredicto: GAP G-2 CERRADO y DOBLE-VERIFICADO. La discrepancia publicada en §1
queda como respuesta canónica de la mesa al cross-check 02b↔02d.
