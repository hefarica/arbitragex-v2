# WO-02c-FIX-TALLY — Tallies de completitud §10.5/§6 aritméticamente exactos · 2026-09-17

> Fixer gang ronda 1 (charter: "Tallies de completitud del adendum §10.5 (02c) y §6
> (WO-02c-verify-VERIFY.md) aritméticamente erróneos... Bajo FAIL-HONEST del board los
> conteos de evidencia deben ser exactos"). Bautizado "TALLY" (era "G5" en mis primeros
> marcadores) para no colisionar con el fixer G5 del GAP-5/SIM_BACKEND que aterrizó en
> paralelo (02c §13).

## 0. Sincronía de mesa redonda

- Board `GOAL-WORKORDERS.md` leído completo. Pares leídos: `WO-02c-CROSS-EXAM.md`,
  `WO-02c-verify-VERIFY.md`, `02c-SIM-STACK-SELECTOR.md` (§1-§12), `WO-02c-FIX-G1/G2`.
- **Convergencia dual registrada**: al iniciar, un fixer paralelo (G4) YA había
  corregido la mitad selector-api (12/12→13/13) en ambos archivos (mtime 02:32). Mi
  re-derivación independiente concuerda (13 no-test en disco; fichas §5.1–5.9 cubren
  los 13 exactos: 5.5 = 2 archivos, 5.7 = 4). Este fix cierra la mitad **sim-ctl**,
  que quedó PEOR que antes del G4: la cadena "15/16 → 16/16" era errónea en sus TRES
  eslabones.

## 1. Hallazgo (re-derivado, RULE 00 — find/wc sobre disco, no heredado)

- **Denominador sim-ctl = 16**: 15 `.rs` en `sim-ctl/src/` + `build.rs` (árbol
  principal). `find backend/sim-ctl/src -type f | wc -l` → 15; build.rs existe (157 LOC).
- **Pre-FIX-G2 la cobertura era 14/16** — sin ficha eran DOS: `route_lookup.rs`
  (solo cita inline §1) Y `build.rs`. El verify §10.5/§6 reportaba "15/16" contando
  una sola omisión → off-by-one.
- **Post-FIX-G2 (ficha §12.1) = 15/16**, NO "16/16": build.rs sigue sin ficha (menor
  defendible, script de build). El §12.3 del entregable decía "pasa de 15/16 a 16/16"
  siendo auto-contradictorio (la misma línea admitía build.rs "permanece declarada").
- La nota del G4 en §10.5 ("superseded a 16/16 por FIX-G2") heredaba esa errata —
  el propio G4 dejó registrado "su aritmética detallada (14/16 con build.rs sin ficha)
  ... se cierra vía esa anotación, no re-derivada aquí": este fix ES esa re-derivación.

## 2. Filtro documentado contra lines-per-file.txt (hint del charter)

Contar SOLO rutas `./backend/...` — el artefacto mezcla árbol principal con worktree
stale `./.claude/worktrees/wf_257a24e1-859-2/backend/...` (sim-ctl worktree: 4 archivos
menos y LOC desfasadas, p.ej. consumer.rs 188 vs 937 real). tests/ excluido por la
convención "src no-test" del propio §10.5.

**Refutación parcial del hint**: la hipótesis "worktree explica el 12/12" NO se
sostiene — el worktree también tiene 13 no-test en selector-api (solo le faltan 2
ARCHIVOS DE TEST: sel-gate01.test.ts, internal_heuristic.test.ts). Ningún filtro de
árbol produce 12. El hazard de doble árbol es real para LOC pero NO explica los
tallies; el "12" y el "15" se clasifican UNKNOWN (desliz manual de conteo). Coincide
con G4 (02c §10.5 nota) — doble verificación independiente.

## 3. Cambios aplicados (todos marcados `// WO-02c-FIX-TALLY (2026-09-17)`)

En `02c-SIM-STACK-SELECTOR.md`:
1. §10.5 cuerpo (:339-341): "12/12"→"13/13" (in situ, complementa la nota append-only
   de G4 que lo documentaba sin editar el cuerpo) y "15/16"→"14/16".
2. §12.3 (:491): "pasa de 15/16 a 16/16" → "de 14/16 a 15/16" + denominador y filtro
   documentados inline.
3. Nota G4 en §10.5 (:372-375): "superseded a 16/16" corregido a la cadena exacta
   (14/16 → 15/16 post-G2).
4. Errata §15 append-only (:626+, numerada 15 porque los pares G5/G3 anexaron §13/§14
   en paralelo) con la aritmética completa, clasificación de evidencia y propagación.

En `WO-02c-verify-VERIFY.md`:
5. §6 (:86-91): "sim-ctl 15/16 (superseded a 16/16...)" → "**14/16** (era '15/16' —
   off-by-one...; tras FIX-G2 = 15/16, NO 16/16)".
6. Errata §10 append-only al pie (mitad sim-ctl; G4 ya cubría selector-api en §9).
7. Reconciliación en la nota de alcance de G4 (§9) desambiguada: "la de G5" → errata
   TALLY (el par la escribió antes de mi renombre; "G5" ahora nombra al fixer del
   GAP-5/SIM_BACKEND).

## 4. Verificación del fix

- Post-edit grep: cuerpo §10.5 lee "13/13" y "14/16"; §12.3 lee "de 14/16 a 15/16";
  "15/16 a 16/16" = 0 ocurrencias vivas (solo citas históricas en erratas). Ningún
  marcador `WO-02c-FIX-G5` residual es mío (los 4 restantes son del fixer GAP-5,
  verificado por grep con líneas).
- Ground truth re-ejecutado post-edit: 13 selector-api no-test, 15 sim-ctl src + build.rs.
- Restricciones: cero git/cargo/npm/build/VPS/HTTP (0/5 requests dominio público).
  Read/grep/sed-sobre-.md SOLAMENTE (restricción board WO-02 :17-19 respetada —
  nunca ejecuté cargo ni npm).
- Archivos tocados: SOLO `02c-SIM-STACK-SELECTOR.md` + `WO-02c-verify-VERIFY.md` +
  este reporte + entrada board. Reportes del gang, no código de producción → sin gate
  arbx-*, sin NO-GIT violation.
- **Conflicto de claims documentado (no pisado)**: mis correcciones contradicen las
  cifras "16/16" del FIX-G2 (peer) y "superseded a 16/16" del G4 (peer) — resueltas
  por errata documentada + marcador in situ, jamás por borrado del contenido fáctico
  ajeno (ficha §12.1, hallazgos §12.2, erratas G1/G2/G4/G5/G3 intactos).

## 5. Handoff

- Cifra canónica vigente para la mesa: **selector-api 13/13 · simulator-v2 6/6 ·
  sim-ctl 14/16 al momento del verify, 15/16 tras FIX-G2 (solo build.rs pendiente,
  menor defendible)**. Cualquier pares que cite "16/16" de sim-ctl está citando una
  cifra refutada.
- Para WO-03/WO-04: si fichan `sim-ctl/build.rs`, sim-ctl pasa a 16/16 de verdad.
