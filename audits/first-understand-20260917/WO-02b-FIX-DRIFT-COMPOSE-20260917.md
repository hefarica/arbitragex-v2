# WO-02b-FIX-DRIFT-COMPOSE — drift documental "31 operators" en compose.prod.yml no flaggeado por §5

> Fixer del Gang Omniscience · ronda 1 · 2026-09-17 · charter: cross-examiner gap.
> Sólo edición documental en archivos de auditoría. Cero git/cargo/VPS/HTTP (0/5 requests).
> Marker de diffs propios: `// WO-02b (2026-09-17)` / `// WO-02b-FIX-DRIFT-COMPOSE (2026-09-17)`.

## 1. El gap

`docker/compose.prod.yml:197-198` (bloque math-engine, inmediatamente bajo el publish
`:196` que el §5 de la ficha SÍ cita) dice:

```yaml
    # Pure compute service — no DB/Redis dependency. Serves the 31 topological
    # operators (list/toggle/compute/264×31 matrix projection) over HTTP.
```

El registry que sirve ese servicio registra **32** operadores. El §5 de
`02b-OPERADORES-MATH-ENGINE.md` analizó ese archivo (cita `:196`) y no flaggeó el
drift del comentario — pese a que la PROPIA ficha ya censaba 32 en §1 (:27), el
DESIGN §4.1 decía "OperatorRegistry registra EXACTAMENTE ids 1..=32" y la
VERIFICACIÓN (1) había hecho el censo exacto. Omisión de cruce interno, no de
evidencia faltante.

## 2. Evidencia (CANONICAL_REPO, reproducible)

- Registry 32: `backend/math-engine/src/operators/mod.rs:139-172` — macro `register!`
  ids 1..=32; `32 => op_32_nsga2::Nsga2Operator::new()` en :172.
  `OPERATOR_COUNT: u8 = 32` en mod.rs:57.
- Endpoints servidos sobre ese registry: `math-engine/src/api.rs:210-216`
  (`/api/operators`, `/api/operators/:id/toggle`, `/api/compute`,
  `/api/matrix/projection`, `/api/matrix/operators`); api.rs:57.
- El segundo "31" del comentario SÍ es correcto: es la matriz de proyección canónica
  264×31 (`COLS=31`, `matrix/topology_map.rs`; congelada por test
  `operator_bounds_cover_entire_registry_without_changing_projection`,
  api.rs:437-446). El comentario MEZCLA dos conteos: census del registry (32) vs
  columnas de la matriz canónica (31). La ficha §1:30 ya documentaba esa asimetría
  deliberada (op_32 fuera de la 264×31 por diseño).
- Origen temporal (git):
  - Comentario introducido con 31 ops reales: `c7ad837e` 2026-07-28
    "feat(math-engine): run the 31 topological operators as a service + frontend toggles"
    (`git log -S 'Serves the 31 topological' -- docker/compose.prod.yml`).
  - op_32_nsga2 agregado al registry después: `95c69d47` 2026-09-11
    (`git log --diff-filter=A -- backend/math-engine/src/operators/op_32_nsga2/`),
    sin actualizar el comentario.
- Impacto runtime del drift: CERO (comentario YAML; no afecta healthcheck ni
  scheduling). No es RULE 00 (no hay dato servido fabricated) — es documentación
  desactualizada que induce a subestimar el census del service.

## 3. Corrección aplicada (documental, append-only + bullet en §5)

1. `02b-OPERADORES-MATH-ENGINE.md` §5: bullet nuevo "DRIFT DOCUMENTAL" con marker
   `// WO-02b (2026-09-17, fixer gang ronda 1)` — cita el comentario, el registry 32,
   la distinción matrix-31 vs registry-32, el origen git y la regla gated.
2. `02b-OPERADORES-MATH-ENGINE.md` §11 (nuevo, append-only): errata completa con
   evidencia, clasificación, contaminación cruzada y propuesta gated.

## 4. Corrección del YAML — NO aplicada (gated, como exige el hint)

Propuesta para PR gated del operador (§32 / NO-GIT / P-∅: un PR = un ID):
`docker/compose.prod.yml:197` `s/31 topological operators/32 topological operators/`,
dejando INTACTO el "264×31" de :198 (ese 31 es la matriz canónica y es correcto).
NADA fue editado en `.yml`/`.rs` — verificación abajo.

## 5. Verificación post-edit

- `git status --porcelain` del repo: sin cambios en `docker/` ni `backend/` por este
  fix (los archivos M preexistentes del working tree no fueron tocados).
- Grep "31 topological" en el dir de auditoría: aparece solo en la ficha (bullet §5 +
  §11, citando el comentario) — ningún otro reporte repite el claim.
- Contaminación cruzada verificada = 0: los archivos que citan compose.prod.yml
  (WO-02b-DESIGN.md:58/:108, 02c-SIM-STACK-SELECTOR.md:142, INFORME.md:45,
  WO-02c-DESIGN.md:22, WO-02c-verify-CROSS-EXAM.md:19, 02-VPS-REMAP) citan el
  publish :196 / SIM_BACKEND / :409 — ninguno endosa el "31 operators".
- Sin contradicciones con pares: WO-02b-FIX-N1 (:§10) y FIX2 (§9) son erratas de
  otras secciones; conviven append-only. El drift doctrina↔código de 02d
  (MainnetRefused) es otro dominio (terminus), no tocado.

## 6. Archivos tocados por este fix

- `audits/first-understand-20260917/02b-OPERADORES-MATH-ENGINE.md` (bullet §5 + §11).
- `audits/first-understand-20260917/WO-02b-FIX-DRIFT-COMPOSE-20260917.md` (este reporte).
- `audits/first-understand-20260917/GOAL-WORKORDERS.md` (entrada de board).
