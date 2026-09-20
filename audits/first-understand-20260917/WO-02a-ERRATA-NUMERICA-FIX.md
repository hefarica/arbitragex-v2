# FIXER — Errata numérica WO-02a (respawn-B, gang 2026-09-17, ronda 1)

> Charter: corregir los números incorrectos que los pares citarán: (a) Corrección A del
> verify ("src .rs = 178 archivos, ~123.7K LOC"), (b) DESIGN §4 workers (22 arch/~16,000),
> (c) DESIGN §5 route_discovery (17 arch/~9,300). Hint del cross-examiner: errata
> append-only en WO-02a-verify-VERIFY.md + aviso al board.
> Mitad propia (respawn-B): ítems (c) y (b); (a) verificada por doble-fuente como
> cross-validation redundante (primario: fixer-A par).
> Reglas duras: CUMPLIDO — 0 git, 0 cargo/npm/build (restricción board WO-02), 0 VPS,
> 0 HTTP (0/5 presupuesto), 0 mocks. Solo lectura + recomputación local + appends.

## 1. Ground truth recomputado (doble fuente: disco `wc -l` + lines-per-file.txt)

Todos los comandos corridos desde la raíz del repo salvo indicación; `lines-per-file.txt`
= `audits/first-understand-20260917/lines-per-file.txt`.

| Ítem | Citado (erróneo) | Recomputado (disco) | lines-per-file.txt no-worktree |
|---|---|---|---|
| (a) searcher-rs `src/**/*.rs` | 178 arch, ~123.7K LOC | **178 arch / 90,401 LOC** | 178 arch / 90,395 (delta 6 = convención de conteo del generador) |
| (b) `src/workers/` | 22 arch, ~16,000 LOC | **23 arch / 14,797 LOC** | 23 arch / 14,797 (idéntico) |
| (c) `src/route_discovery/` | 17 arch, ~9,300 LOC | **16 arch .rs / 11,488 LOC** (+README.md 122 → 17 entradas / 11,610) | 17 entradas / 11,610 (incluye README.md) |

Comandos reproducibles (verificados re-ejecutándolos post-errata):

```bash
cd backend/searcher-rs
# (a)
find src -type f -name '*.rs' | wc -l                                   # 178
find src -type f -name '*.rs' -print0 | xargs -0 cat | wc -l            # 90401
# (b)
find src/workers -type f -name '*.rs' | wc -l                           # 23
find src/workers -type f -name '*.rs' -print0 | xargs -0 wc -l | tail -1 # 14797
# (c)
find src/route_discovery -type f -name '*.rs' | wc -l                   # 16
find src/route_discovery -type f -name '*.rs' -print0 | xargs -0 wc -l | tail -1 # 11488

# vía lines-per-file.txt (no-worktree):
grep -E ' \./backend/searcher-rs/src/.*\.rs$' ../audits/first-understand-20260917/lines-per-file.txt \
  | grep -v worktree | awk '{s+=$1;n++} END{print n, s}'                # 178 90395
grep -E ' \./backend/searcher-rs/src/workers/' ../audits/first-understand-20260917/lines-per-file.txt \
  | grep -v worktree | awk '{s+=$1;n++} END{print n, s}'                # 23 14797
grep -E ' \./backend/searcher-rs/src/route_discovery/' ../audits/first-understand-20260917/lines-per-file.txt \
  | grep -v worktree | awk '{s+=$1;n++} END{print n, s}'                # 17 11610 (incl. README.md)
```

## 2. Diagnóstico de cada error (por qué el número citado no reproducía)

- **(a)** 123,737 = suma de las **184 entradas** src no-worktree (178 `.rs` + 6
  fixtures/README), incluye ~33.3K LOC de fixtures JSON (`dirty_trace.fixture.json`
  22,929, etc.). El corrector de la contaminación-por-worktrees (verify §6 Corrección A,
  `WO-02a-verify-VERIFY.md:99`) arrastró un número contaminado-por-fixtures y lo etiquetó
  ".rs". El cross-examiner ya lo detectó (CROSS-EXAM §2 R3); aquí queda canónico.
- **(b)** El header del DESIGN §4 (`WO-02a-DESIGN.md:112`) omitió el subdirectorio
  `route_scanner_worker/` (`provenance.rs` 86 + `provenance/tests.rs` 173) y redondeó
  mal el LOC. **Hallazgo propio adicional (RULE 00)**: el valor "23 archivos/14,624 LOC"
  dado como ground truth en el charter del fixer contiene el MISMO slip: 14,624 =
  14,797 − 173 (resta `provenance/tests.rs` pero cuenta 23 archivos). Recombinado:
  23/14,797 con tests, 22/14,624 sin tests — la mesa debe declarar UNA convención.
- **(c)** El header del DESIGN §5 (`WO-02a-DESIGN.md:180`) contó 17 entradas (incluye
  README.md, no solo .rs) y su "~9,300" no es reproducible desde ninguna fuente
  (undercount 2,188 vs los 11,488 .rs reales; el archivo más grande del módulo,
  `route_discovery_worker.rs` [2,614 LOC], ni siquiera figura en el per-file list del §5).

## 3. Fix aplicado (append-only, sin borrar trabajo ajeno)

1. **`WO-02a-verify-VERIFY.md`** — nueva sección "## ERRATA NUMÉRICA append-only —
   WO-02a (2026-09-17)" (E-a/E-b/E-c + impacto + aviso a la mesa) al final del archivo.
   Nada del contenido previo fue modificado.
2. **`GOAL-WORKORDERS.md`** — bullet "⚠ ERRATA NUMÉRICA WO-02a (2026-09-17, fixer
   respawn-B)" dentro de la sección WO-02: valores canónicos + directiva de que
   02b/02c/02d y WO-06 citen esos números con convención declarada.
3. **NO edité `WO-02a-DESIGN.md`** (pertenece al diseñador WO-02a): los headers §4/§5
   quedan como historial; la errata del archivo de verificación los supersede para citas.

## 4. Conflictos de archivo / sincronía de mesa

- **Concurrencia detectada y respetada**: mientras yo escribía, otro fixer (mitad del
  charter de procedencia del diff PancakeV3) agregó al board su propia "ERRATA-WO-02a"
  (board líneas 46-62: diff huérfano de working-tree, NO dcfe890c). Mi edición aterrizó
  DESPUÉS de la suya, sin pisarla (bloques disjuntos). Sus erratas pendientes asignadas
  ("WO-02a-DESIGN.md §0/§3/§10-D1, WO-02a-verify-VERIFY.md §7") son de SU mitad — no las
  toqué; nota: su errata menciona el §7 del verify, que en mi lectura del archivo es la
  sección "Tally final" (la Corrección A con el número erróneo vive en §6) — indicación
  menor de línea para ese par.
- Cito como base: `WO-02a-verify-CROSS-EXAM.md` §2 R3/G3 (origen del gap (a));
  `WO-02a-verify-VERIFY.md` §5-§6 (tabla de omisiones y Corrección A que corrijo);
  `WO-02a-DESIGN.md` :112/:180 (headers con números erróneos, NO tocados).
- No contradigo hallazgo previo alguno: la conclusión estructural (charter 193 ≠ 178
  real; estados de fichas) NO cambia con la errata.

## 5. Verificación del fix

- Los comandos del §1 re-ejecutados tras escribir la errata: misma salida (178/90401,
  23/14797, 16/11488; y 178/90395, 23/14797, 17/11610 vía txt). Reproducibles por
  cualquier par.
- Solo se tocaron 2 archivos existentes con appends puros + 1 reporte nuevo. `git status`
  del repo no muestra ningún archivo `src/` tocado por mí (los diffs PREEXISTES del
  operador — route_intent.rs — siguen intactos, verificados por el otro fixer con git).
- Sin cargo/tsc aplicable (fix documental; la restricción del board prohíbe builds).

Clasificación de evidencia: CANONICAL_REPO (conteos recomputables con find/wc/awk).
