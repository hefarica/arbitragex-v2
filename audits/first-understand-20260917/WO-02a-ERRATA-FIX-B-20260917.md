# FIX-B · ERRATA dcfe890c — corrección de board (mitad B, 2026-09-17)

> Gang Omniscience ronda 1 · fixer reemplazo B (RESPAWN-2) · mitad = ítems ⌈3/2⌉+1..3
> del hint = **entrada del board GOAL-WORKORDERS.md**. Los appends en WO-02a-DESIGN.md y
> WO-02a-verify-VERIFY.md son de la mitad A (par en paralelo) — NO pisados.
> Método: git read-only + Edit append-only. CERO commit/push/PR/deploy (cumplido, verificado).
> CERO cargo/npm (restricción board WO-02 — cumplido; además innecesario: cambio solo docs).

## 1. Hecho a corregir

Atribución falsa de procedencia: el diff working-tree de
`backend/searcher-rs/src/route_intent.rs` (PancakeV3→Unknown) fue atribuido al commit
`dcfe890c` en WO-02a-DESIGN.md §0 (:12-14) y §3 (:101,:109), en la NOTA de D-1 (:396),
y repetido por el verificador en WO-02a-verify-VERIFY.md §7 (:128).

## 2. Evidencia forense (git corrido por mí — no asumido del errata)

- `git show dcfe890c --name-only` → SOLO `backend/shared-rs/src/chains.rs` +
  `backend/sim-ctl/src/tx_builder.rs`. route_intent.rs NO está en el commit.
- `git log --oneline -- backend/searcher-rs/src/route_intent.rs` → último commit =
  `a37bcbfc` ("fix(searcher): close V3 wire..."). Ningún commit reciente toca el archivo.
- `git status --porcelain -- backend/searcher-rs/src/route_intent.rs` → ` M` (modificado,
  NO staged, NO commiteado).
- `git diff -- route_intent.rs` → +7/−1 exactos: comentario :264-266 (ampliado), mapeo
  `PancakeV3 => Unknown` :275, test `(Shared::PancakeV3, RouterKind::Unknown)` :~402.
  El comentario interno dice "**PANCAKE-ROUTER-01 follow-up**" — apoya la hipótesis de
  autoría-operador SIN confirmarla (no hay commit ni firma que la pruebe).
- mtime verificado: **2026-09-17 01:36:02 -0500**. DISCREPANCIA MENOR vs el errata del
  cross-examiner (citaba 00:31): reporto el valor actual sin inventar causa (podría ser
  re-escritura del propio operador entre 00:31 y 01:36 — HYPOTHESIS, sin evidencia).
  En ambos casos es previo/compatible con la línea temporal del reporte WO-02a.

## 3. Corrección aplicada (mi mitad — board)

`GOAL-WORKORDERS.md` entrada WO-02a: añadido bullet **ERRATA-WO-02a (2026-09-17, fixer
gang)** (ahora :46-61, append-only, sin borrar la entrada previa). Reclasificación canónica:
**diff huérfano de working-tree, NO commiteado, autoría SIN confirmar (hipótesis: operador
follow-up de PANCAKE-ROUTER-01)** — consistente con WO-05 que ya lo llamaba "diff huérfano"
(board, WO-05: "Incluye el diff huérfano route_intent.rs (PancakeV3→Unknown)").

Nota honesta: la entrada del board NO nombraba dcfe890c literalmente (las ocurrencias
literales están en los archivos de WO-02a, mitad A); la contaminación del board era por
endoso del reporte. La errata del board lo declara explícitamente y aísla el hecho
corregido de la parte válida del reporte (15/15 fichas PASS en sustancia siguen en pie —
la atribución de procedencia era el defecto, no el contenido del diff).

## 4. Contaminación residual (documentada, NO tocada — archivos de otros owners)

| Sitio | Contenido | Acción |
|---|---|---|
| `INFORME.md:63` (REPARO-1) | "dcfe890c (PANCAKE-ROUTER-01) agregó RouterKind::PancakeV3 al shared enum y olvidó este From" — implica que el diff working-tree salió de la misma sesión que dcfe890c | Documentar; owner orquestador/WO-05 decide |
| `WO-02a-verify-CROSS-EXAM.md:35` | "el único diff working-tree es el PREEXISTE del operador (dcfe890c)" — el mismo error que originó la errata | Documentar (es la fuente del errata) |
| `WO-02a-DESIGN.md:393` | "configs/router_kinds.json ya tiene la dirección por dcfe890c" — dcfe890c tocó chains.rs, no router_kinds.json (la DIRECCIÓN vive en chains.rs; router_kinds.json queda POR VERIFICAR) | Mitad A (mismo archivo de su errata) |
| `02-VPS-REMAP-20260917.md:5` | "(dcfe890c + diff route_intent.rs) NO están desplegados" — agrupa commit y diff; el hecho deploy es VERDADERO para ambos, el agrupamiento sugiere procedencia común | Menor; documentar |

## 5. Verificación del fix

- Grep post-edit: bullet ERRATA-WO-02a presente en GOAL-WORKORDERS.md:46+; WO-05 línea
  "diff huérfano" intacta (consistencia lograda).
- `git status`: sin cambios staged, sin commits nuevos (HEAD sigue dcfe890c, branch
  fix/v3-slot0-coverage-20260917 intacta). Modificados solo los preexistentes del operador
  + el board editado por mí (untracked dir audits/first-understand-20260917/).
- Wordging auto-corregido: el board NO afirma que las erratas de mitad A ya estén
  aplicadas (verificado por grep: aún no lo están — declara "PENDIENTES... aún NO aplicadas").
  Anti-afirmación-de-humo: exactamente el defecto que esta errata corrige.

## 6. Clasificación

CANONICAL_REPO para todos los hechos git (recomputables con los comandos de §2).
HYPOTHESIS para autoría-operador del diff huérfano (comentario interno + timing, sin commit).
