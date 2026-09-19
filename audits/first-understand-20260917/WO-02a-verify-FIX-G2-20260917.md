# WO-G2 — Errata de sincronía de mesa: §0 del verify refutado y reconciliado — // WO-G2 (2026-09-17)

> Fixer gang ronda 1. Charter: gap G2 del orquestador (= hallazgo **R2** del
> cross-examiner, `WO-02a-verify-CROSS-EXAM.md` §2-R2 — NO el G2 de la tabla del
> cross-exam). Entregable: errata append-only `## 10` en
> `WO-02a-verify-VERIFY.md` que corrige el §0 ("no toca mi claim... sin
> contradicciones"), nombra la contradicción con 02d §5/:212 y reconcilia con
> 02b/02c. Restricciones: 0 git/cargo/VPS/HTTP (cumplido — Read/Grep/Edit
> markdown solamente).

## 1. Cambio aplicado

- `WO-02a-verify-VERIFY.md` — nueva sección `## 10. ERRATA DE SINCRONÍA DE MESA —
  §0 queda REFUTADO — // WO-G2 (2026-09-17)` (líneas ~297-399). Append-only: §0-§9
  y erratas previas intactos; el texto original de §0 se conserva in-place para
  trazabilidad y queda formalmente superseded para citas.
- NO se tocó: `WO-02a-DESIGN.md` (la mitad del diseñador ya fue reconciliada por
  `WO-02a-FIX-S9-20260917.md` §9.1 — sin solape, este fix cierra la mitad
  verificador), ni reportes de otros owners (`02b/02c/02d`,
  `WO-02a-verify-CROSS-EXAM.md` — su drift ":148" queda corregido solo DENTRO de mi
  errata §10.2, sin editar su archivo).

## 2. Evidencia re-abierta ANTES de escribir (todo CANONICAL_REPO, byte-exact)

- `02d-RELAYS-CLIENT-SHARED.md:212` — etiqueta `capital_usd` como "fuente REAL de
  objetivo_usd runtime" (exacto).
- `02d` §5 (`:245-258`) — tres fuentes USD runtime (exacto), anclajes de código
  verificados por mí:
  - `shared-rs/src/trading_config.rs:183` `pub capital_usd: f64` — EXACTO.
  - `relays-client/src/execution_admission.rs:193-201` — `usd(amount_in) <=
    capital_usd/50` (cap 2 %) — EXACTO.
  - `shared-rs/src/config.rs:156-158` — `default_max_value() -> 1.0` (1.0 ETH) — EXACTO.
  - `searcher-rs/src/scanner.rs:2568` — `min_profit_threshold: cfg.min_profit_usd`
    en `decode_and_score_tx` sin feature-gate — EXACTO (piso USD absoluto vivo).
  - `trading_config.rs:249/:260/:622` — `simulation_target_profit_usd`,
    `min_profit_usd`, `effective_min_profit_usd(strategy)` — EXACTOS.
- `02b-OPERADORES-MATH-ENGINE.md:154` — G1 objetivo_usd por operador = GAP. **Drift
  corregido**: el cross-exam citó ":148"; la línea real es **:154** (:148 es el
  texto del toggle math-engine).
- `02c-SIM-STACK-SELECTOR.md:209-211` — §7 objetivo_usd = GAP en los 4 crates — EXACTO.
- Timestamps (ls del dir): 02d 01:02 · 02b 01:06 · 02c 01:06 · verify ~01:07
  (observación del cross-exam 01:13; mtime actual del verify ya no lo conserva por
  las erratas append-only) — confirma la falla: 02b/02c publicados ANTES del
  dictamen y no citados en §0.

## 3. Reconciliación publicada (§10.3 de la errata)

Los cuatro reportes son COMPATIBLES una vez acotados sus dominios — la
"contradicción" era aparente: **no existe objetivo_usd por estrategia (02a §9+FIX-R1)
ni por operador (02b G1:154) ni en sim/selector (02c §7), pero SÍ existen blancos/
caps USD runtime globales del operador (02d §5/:212) y un piso USD absoluto vivo en
el hot-path del scanner** (scanner.rs:2568). Esto es lo que los gates 1-8 deben
consumir; el PASS angosto del verify (GAP por-estrategia) sobrevive, la lectura de
board "umbrales solo relativos" queda re-stringida.

## 4. Concurrencia detectada y resuelta honestamente

- Mientras escribía, un **par G3** appendeó `WO-02a-FIX-G3` (:403+) en el MISMO
  archivo corrigiendo la premisa engines-gate del §3 (el G2 de la tabla del
  cross-exam, que yo había declarado PENDIENTE en §10.4). Re-verifiqué su corrección
  por lectura propia (`engines/mod.rs:25-28` +3 "Task 3" :31-33 ungated; **7**, no
  8, módulos gated :38-52 — el miscount "8" era del propio cross-exam y G3 también
  lo corrigió; yo lo había replicado y corregí mi propio texto). Mi §10.4 fue
  actualizado in-place (sección propia, escrita este mismo turno) para marcar ese
  ítem como RESUELTO-por-par-concurrente en vez de pendiente.
- Ambas secciones coexisten append-only; orden en archivo: errata G2 (:297) antes
  que G3 (:403) por el ancla de inserción — sin pérdida de contenido de nadie.

## 5. Regla de proceso adoptada (pedido del hint del cross-examiner)

Queda inscrita en §10.5 de la errata: **contra-citar pares por archivo:línea antes
de declarar "sin contradicciones"** — una declaración de no-contradicción sin citas
nombradas por archivo:línea se considera NO EMITIDA. Aplica a todo verify/cross-exam
posterior de esta mesa.

## 6. Verificación del fix (re-ejecutada)

- Marcador: grep `WO-G2 (2026-09-17)` en WO-02a-verify-VERIFY.md = 1 hit (§10).
- Estructura post-edit verificada con grep de headers: §0..§7 originales + §8 +
  E-a..E-c + mi §10 + G3 :403 — nada borrado ni reordenado.
- Citas re-abiertas post-escritura: las 11 de §2 arriba — todas exactas.
- `git status`: sin cambios en `backend/` (markdown-only; restricción board WO-02
  de no-cargo/no-build intacta; CERO git).
