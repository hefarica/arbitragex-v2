# FE-04 · DESIGN — EL PLAN (por todos, para todos): FRONTEND-DOCTRINE.md

> **WO:** FE-04 · kind: redesign (composición — CERO edición de código de producción)
> **Agente:** ecc:react-reviewer (Gang Omniscience, mesa frontend-doctrine 2026-09-07)
> **Entregable:** `FRONTEND-DOCTRINE.md` en la RAÍZ del repo (creado por mí — único write de producción-adyacente).
> **Reglas respetadas:** NO-GIT (0 commit/push/PR/deploy) · RULE 00/R8 · §32/§33 read-only (VPS ni tocado —
> toda la evidencia de pares fue re-verificada contra el ÁRBOL local, no contra el dominio público:
> presupuesto HTTP 0/5 usado) · Lexicon OMEGA en todo el texto.

---

## 0. Sincronía de mesa redonda (mi PRIMERA acción fue el dir + pares)

Leídos completos ANTES de diseñar (orden de lectura):
1. `audits/frontend-doctrine-2026-09-07/GOAL-WORKORDERS.md` (board — mi fila era PENDIENTE).
2. `FE-01-DESIGN.md` (censo 58 + mapa + FE-01-GATE-1) — dependencia CUMPLIDA.
3. `FE-02a-DESIGN.md` (BUG-01..10 + GAP-11..13 + FIX-1..10, integridad de datos) — CUMPLIDA.
4. `FE-02b-DESIGN.md` (FE-02b-01..10, render/hidratación/a11y/overflow/memoria) — CUMPLIDA.
5. `FE-03-DESIGN.md` (5 pilares A-E + olas 1-5) — CUMPLIDA.

**NINGUNA sección quedó DEPENDENCIA-PENDIENTE (R8): los 4 reportes de pares existían al iniciar.**
Canon consultado para BR-11 antes de opinar: `docs/ROUTES_CROWN_JEWEL_DOCTRINE.md:42-52`
(financing = dimensión de ruta; `OWN_CAPITAL|AAVE_FL|BALANCER_FL|V2_FLASH_SWAP`; fees on-chain; R8) y
`backend/searcher-rs/src/financing.rs:70-77` (parse canónico de los 4 tokens) — citados en §5.1.2 de la doctrina.

## 0.1 Estado del árbol (declaración R8 — el árbol compartido ROTÓ durante la sesión)

- Snapshot de arranque de MI sesión: branch `a6-cbprom-01` (git status con `M frontend/lib/websocket-client.ts`,
  `M RuntimePostureBar.tsx`, `?? websocket-client.test.ts`, `M scanner.rs`, etc.).
- Al ejecutar mi verificación: **branch `feat/hops-live-01` @ `27aca289`** — un peer conmutó el árbol
  compartido (§36 disciplina de branches; el propio FE-02a reportó trabajar sobre esa branch). Dirty ahora:
  `M frontend/lib/drift/useDriftDetection.ts` + untracked `frontend/app/control/`, `ControlBoardLed.tsx`,
  `ControlBoard.test.tsx` (programa CB-03 en vuelo — NO tocados).
- Consecuencia: la doctrina lleva un bloque "estado del árbol al componer" + regla §7.1 de re-anchoring, y la
  coordinación CB-03 (P0-5 espera al merge del programa CB).

## 1. Verificación adversarial propia (qué re-verifiqué y qué encontré)

Como compositor NO re-derivo lo que los pares probaron — RE-VERIFICO los puntos de carga estructural:

| Punto | Método | Resultado |
|---|---|---|
| `websocket-client.ts` muerto (FE-03 §1.2#5) vs "revivido" (FE-02a §0 WO-01 `325e3154`) | `grep websocket-client frontend/**/*.{ts,tsx}` + `git log -- frontend/lib/websocket-client*` + lectura del archivo | **Ambos tienen razón parcial — adjudicado (§2).** El commit `325e3154` existe (realineó el adapter al contrato `row_to_json(NEW)` con doc-comentario RULE 00), pero en HEAD `27aca289` el módulo sigue importado SÓLO por su test (`websocket-client.test.ts:17`). El patrón BUG-07 (`client.connect(); setConnected(true);` — hoy ~línea 229-231) persiste. WO-01 no montó consumidor de producción. |
| `/control` huérfana (FE-01 §4.1) — CB-03 | `grep -n 'href="/control"' frontend/` | **0 matches — CONFIRMADO.** La página existe (`app/control/{page.tsx,ControlBoardClient.tsx}` untracked) y es R1-verificada por el peer CB-VERIFY, pero nadie linka a ella. |
| `routeKeyOf` sin topología (BUG-01) | lectura `OpportunitiesClient.tsx:40-50` + render `:436-449` | **CONFIRMADO** — key legada exactamente como cita FE-02a; el doc-comment de diseño ("re-detected route → MISMA card") confirma la intención de identidad-de-ruta que la implementación traiciona en ciclos cerrados. |
| Store: `MAX_OPPORTUNITIES=200`, `pruneStale` | `grep` `omni-store.ts:187,409` | CONFIRMADO (anclas de FE-03 correctas). |
| Canon financiamiento BR-11 | `ROUTES_CROWN_JEWEL_DOCTRINE.md:42-52` + `financing.rs:70-77` | CONFIRMADO — 4 tokens canónicos exactos; la doctrina los adopta verbatim. |
| `nav-items.ts` (para P0-5) | lectura completa (46 items, grupo `control` líneas 70-77) | CONFIRMADO — `GaugeIcon` ya importado; el diff P0-5 es 1 línea + comentario. |

## 2. Adjudicaciones y refutaciones (deber de la mesa — nadie se ignora)

1. **FE-03 ola-1 "eliminar websocket-client.ts" vs FE-02a "WO-01 lo revive"** — la ÚNICA contradicción real
   entre pares. Evidencia propia (§1): muerto-como-consumidor PERO con contrato recién corregido. **Fallo:**
   la eliminación PROCEDE (el argumento estructural de FE-03 — dos mappers paralelos = drift — es correcto),
   CON dos condiciones que FE-03 no tenía: (a) trasplantar el doc-comentario del contrato `row_to_json(NEW)`
   (el conocimiento WO-01) a `socket-lifecycle.ts`; (b) gate condicional que ABORTA la eliminación si un
   consumidor de producción aterrizó mientras tanto (entonces: FIX-9 de FE-02a + ruta por el provider).
   Nadie refutado por completo: FE-03 gana la acción, FE-02a gana la preservación del conocimiento y el gate.
2. **BUG-06 (FE-02a) = FE-02b-01** — convergencia independiente de dos pares con la misma cura
   (`roi_pct ?? null`). No hay contradicción: la doctrina adopta la UNIÓN de los diffs (FE-02b cubre los 3
   sitios de render + flecha; FE-02a cubre el estado vacío con razón). Cito ambos en P0-2.
3. **FE-03 ola-2 vs FE-02a FIX-1** — no son contradictorios pero TOCAN el mismo archivo con modelos de
   identidad distintos (FE-03: `order` por id de detección; FE-02a: una card por RUTA). **Fallo de composición
   (aporte central de FE-04):** el store normalizado se re-keyed POR RUTA — `order: routeKey[]`,
   `entities/metrics: Record<routeKey, …>` — con resolución latest-by-route DENTRO de
   `applySnapshotAndDeltas` (la biyección INV-A deja de ser un dedupe de render y pasa a ser propiedad
   estructural del store). Secuencia: FIX-1 como hotfix (ola 0) ANTES de la ola 2.
4. **A FE-03 §0 (refinamiento menor):** su nota "AUSENTES al momento de escribir: FE-01/FE-02a/b" explica por
   que su ola-1 no conoció WO-01 — no es error del par, es orden de llegada; lo dejo registrado para que el
   cross-examiner no lo cuente como inconsistencia.
5. **FE-01 cifra 58 vs goal 57** — adopto la lectura de FE-01 (drift histórico 56→58, "57" ambiguo [UN]) sin
   re-abrir el conteo; FE-01-GATE-1 queda como mecanismo anti-drift vivo en el checklist (§3 de la doctrina).

## 3. El entregable — FRONTEND-DOCTRINE.md (estructura y decisiones)

`C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\FRONTEND-DOCTRINE.md` — 9 secciones:

1. **Resumen ejecutivo** (operador): 58 páginas/6 transportes; lo SANO congelado como canon (FE-02b §3);
   lo ROTO con la cadena BR-11 de 3 defectos (FE-02a §1); la meta SDQ; BR-11 y CB-03 en 4 líneas;
   la adjudicación del conflicto.
2. **(=§2 doctrina) Mejoras P0/P1/P2** — tabla esfuerzo×impacto + ola; cada ítem con diff exacto: los de
   origen peer se citan a SU sección verbatim con esencia reproducida; P0-5 (`/control` al sidebar) es diff
   NUEVO de FE-04 marcado `// FE-04 (2026-09-07)` con gate (a)(b)(c)(d), rollback y coordinación CB-03.
3. **(=§3) Checklist PR-validable** — ★ bloqueantes: R1-forma, suppress span-only (grep INV-1), superficie
   4-variantes, error verbatim, filtro honesto (guard BR-11), allSettled, RULE 00 render, wei→USD prohibido,
   financiamiento verbatim, store con FetchStatus, `useOmniStore()` sin selector prohibido, allowlist io(),
   cleanup 28/28, React-18 pin, minmax(0,1fr), keys por ruta, comparador completo, a11y mínimo, censo
   FE-01-GATE-1, perf-budget, marcador FE-04.
4. **(=§4) Delta-streaming POR COMPONENTE** — cadena de ingesta punta-a-punta (incluye mergeRawPush de P0-4
   como capa de la costura); tabla de 12 componentes × (suscripción · diff consumido · unidad DOM · memo
   boundary); invariantes INV-DS-1..4 + GATE-DS con extensión de ruta propia.
5. **(=§5) Aceptación BR-11** — 6 criterios operativos (capital configurado visible, financiamiento verbatim
   4 tokens canon + interim honesto, waterfall por hop con montos exactos, entrega final Δ ciclo,
   persistencia del dinero, visibilidad multihop estructural) + gate local (CI) + gate browser (FE-05) +
   qué NO puede cerrar el FE (§8.3 backend).
6. **(=§6) Roadmap SIN big-bang** — 6 olas (0 = hotfixes P0/P1 en 9 micro-PRs con orden interno explícito
   0a→0i; 1 = limpieza adjudicada; 2 = SDQ re-keyed por ruta; 3 = contrato de superficie; 4 = presupuesto;
   5 = consolidación de transporte) — cada una con GATE de salida y ROLLBACK (requisito del charter que
   añadí: FE-03 no especificaba rollbacks).
7. **(=§7) Contrato de adopción** — 7 reglas imperativas para cualquier agente sin contexto: antes/durante/
   después, RULE 00, BR-11 como requisito no cosmética, CB-03 LED honesto, verificación obligatoria,
   mecanismo de amend de la doctrina (versionada por la mesa).
8. **(=§8) Dependencias** — operador (D-11 + CF cache), programas hermanos (CB-03 untracked, HOPS-LIVE-01),
   backend (financing_mode wire, ledger por hop del kernel triangular, enriquecer puente LISTEN, ancla USD).
9. **(=§9) Atribución** — tabla par→aporte→verificación FE-04; clasificación [CR]/[UN] honesta (rooms
   metrics/convergence/prices quedan [UNKNOWN] heredados de FE-03 hasta ola 5).

**Invariante del diseño (FE-04-INV-1):** toda afirmación de la doctrina es citable a (a) un file:line del
árbol, (b) la sección de un reporte peer, o (c) mi verificación §1 — cero afirmaciones huérfanas. El
checklist §3 es 100% mecánico (grep/test/build) — ningún ★ depende de juicio subjetivo del reviewer.

**Gate de verificación (FE-04-GATE-1):**
```bash
test -f FRONTEND-DOCTRINE.md && grep -c "FE-04 (2026-09-07)" FRONTEND-DOCTRINE.md   # >0 (marcadores presentes)
grep -c "FE-0[123]" FRONTEND-DOCTRINE.md                                            # >30 (citas a los 4 pares)
git status --short -- frontend/ app/ 2>/dev/null | grep -v control                  # sin código de producción tocado por FE-04
```
Escritura verificada: `git status` antes/después — mis únicos writes son `FRONTEND-DOCTRINE.md` (nuevo),
este reporte y la fila FE-04 del board. CERO commit/push. CERO VPS. CERO HTTP público (0/5).

## 4. Límites honestos de ESTE reporte (RULE 00 sobre mí mismo)

1. **CERO código editado** — los diffs de la doctrina son diseño; la aplicación es de olas con PRs (P-∅).
2. No re-verifiqué los 13 hallazgos de FE-02a ni los 10 de FE-02b uno por uno (los pares dan file:line + PG
   vivo); re-verifiqué SOLO los 6 puntos de carga estructural (§1) — el cross-examiner puede auditar el resto.
3. `[UNKNOWN]` heredados deliberadamente: shapes/cadencias de rooms metrics/convergence/prices (FE-03 §9.3);
   método contable del "57" del goal (FE-01 §0 [UN]).
4. El baseline tsc/vitest (120/1,114/0) es el medido por FE-02b en `a6-cbprom-01` a las ~09:55 — el árbol
   rotó a `feat/hops-live-01` después; NO lo re-corrí (no edité código: nada mío que compilar). Quien aplique
   la ola 0 re-baselinea sobre la branch vigente.
5. Presupuesto dominio público: 0/5 requests (composición estática). VPS: no tocado (§32/§33).

## 5. Para FE-05 (verificación browser — el siguiente de la mesa)

La doctrina te asigna los journeys: (a) ANTES de ola 0 — evidencia del estado actual (grid con colapso de
cards por key, ticker con % fabricado si hay filas cotizadas, /config scroll-x en 320px); (b) DESPUÉS de cada
ola — los criterios de aceptación BR-11 §5.2 (browser) + presupuesto §ola-4 (heap/nodos/commits vía CDP).
Recuerda: el dominio público corre POLLING hasta que el operador remedie D-11 — no diagnostiques "WS muerto"
como defecto del frontend en esa superficie (FE-02a §0; INV-DS-3 de la doctrina).

— ecc:react-reviewer · WO FE-04 · 2026-09-07
