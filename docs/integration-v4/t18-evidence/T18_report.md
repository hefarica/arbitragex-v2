# T18 — V6 bajo orden aprobada del operador · reporte de pasada (α9 VIGIL + α10 SCRIBA)

> attempt 226ca0ff · 2026-10-13 16:0x–16:4x UTC · todo bajo Git ssh.exe pineado (R12) · P1/P5 intactos.
> Estado al cierre del turno: **pasos 1, 2, 4 VERDES con evidencia; 3 EN VUELO (dump corriendo);
> 5 BLOQUEADO por gate t7 (LISTO-PARA-V6 no emitido; t7 in_progress con needs_revision H1-H4 ya
> reparados por esta tarea) + imagen R-<ts> inexistente (ABORT de build por guard de disco — abajo)**.

## PASO 1 — Transporte tar-over-ssh: **VERDE**

- Comando: `tar czf - -C %TEMP% t18-probe.txt | ssh arbx 'tar xzf - -C /tmp/t18-transport-test'`
  (Git tar.exe + Git ssh.exe pineados; target registrado en A0d ANTES — Q8).
- Salida (literal, t18-evidence/): hash local `37e6be2b6eb63777ffecd3ba39953b430509f9bbf84c8e964033d24706298ab1`
  == hash remoto idéntico. **Transporte íntegro probado.**

## PASO 2 — D1 reutilización de corrida cargo t10: **PROBADA (salida literal)**

- Worktree limpio `21feefb32` + detector_policy.rs del release aplicado:
  `git status --porcelain` → **SOLO** ` M backend/searcher-rs/src/detector_policy.rs`;
  `git diff --stat -- '*.rs' 'Cargo.toml' 'Cargo.lock'` →
  ` backend/searcher-rs/src/detector_policy.rs | 134 +++… 1 file changed, 134 insertions(+)`
  (salida completa en transcript/`t18-evidence/d1_diff.txt` si se pega de nuevo).
- **Triple hash**: worktree == release(gen/) == árbol compartido == `5698976fd2573be720795e5e8357093e826c6f52dfa16fd27fa27b9002df2aeb` — el estado que t10 compiló
  (build exit=0; 1431P/3F/5I, 3F==baseline exonerado) es BYTE-IDÉNTICO al código Rust que R-t6 aporta.
  El diff `*.rs/Cargo.toml/Cargo.lock` entre t10-build-state y R-t6 es **VACÍO por construcción y
  probado por hash** ⇒ reutilización VÁLIDA (no aserción).
- Nota honesta: la comparación línea-a-línea del diff vivo vs `gen/detector_policy.rs.diff` difiere
  en formato de encabezado (index lines) — irrelevante para el criterio (1 archivo, +134, hash igual).

## PASO 3 — Dump + restore-test: **[i] VERDE · [ii] satisfecho por secuencia · dump EN VUELO**

- **[i] Medido antes de dumpear** (`t18-evidence/pre_dump_medidas.txt`): disco VPS **93% / 10,457MB
  libres** (PEOR que R1-88% de t15 — agravado), PG **82 GB** (opportunities 31G, scored 20G,
  pool_reserves 16G, particiones route_discovery 8.2+5.9G), C: local **67.7GB libres**.
  ⇒ dump EN el VPS = **ABORT por guard** (1.5× est. ≥22GB > 10.4GB): **dump VÍA PIPE al disco local,
  0 bytes de disco VPS** (comando: `ssh arbx "docker exec -i arbitragex-v2-postgres-1 pg_dump -U
  postgres -Z3 -Fc arbitragex" > C:\...\arbitragex-20261013T1636Z.dump`, job durable pwsh-11).
- **[ii] Fuera de W**: el dump corre AHORA, horas antes de cualquier swap (secuencia dump→verificar→
  drill→[gate]→swap→obtener W=180s). Satisfecho por diseño de secuencia.
- Progreso al cierre del turno: 1.33 GB a ~11 min (~7 GB/h, I/O-bound compartido con producción);
  ETA ~2-4 h. **Verificación pendiente al completar**: `pg_restore --list` (integridad TOC) + sha256
  → fila ℛ. **[iii] restore-test PENDIENTE del dump**: schema completo + tabla selectiva (≤1GB,
  guard) en DB scratch `arbx_restore_test` (fila A0d ya registrada) con conteo vs origen.
- P10 piso: releases en disco = 2 (imágenes searcher actual + contexto release staging) ✓;
  **dump restaurable = PENDIENTE** (dump incompleto — declarado, no rellenado).

## PASO 4 — Drill de rollback medido: **VERDE (mecánica, espejo retag) + build ABORT por guard**

- **Build de `arbx:R-<ts>` en VPS: ABORT por guard de disco** — estimación builder multi-stage del
  workspace Rust ≈ 15-25GB (compila TODO dentro de Docker; `Build Cache 0B`, **0 imágenes dangling**,
  2.3GB reclaimable son imágenes TAGGED sin auditar) vs 10.4GB libres ⇒ 1.5×est >> libre.
  Docker local: **AUSENTE** (y RULE 01 prohíbe backend local). **ESCALADO al capitán/operador** con
  números; opciones: liberar ≥12GB auditados · docker save/load desde estación con Docker · autorizar
  riesgo (NO recomendado: PG vivo, disco 93%).
- **Drill ejecutado con espejo retag** de la imagen searcher real (`dfd71012e1dc`, 185MB — mismos
  layers, 0 build, 0 red), stack efímero `/tmp/arbx-drill` bajo **flock /tmp/arbx-deploy.lock**
  (salida literal `t18-evidence/drill_rollback_v2.txt`, registrada en ℛ):
  - `t_swap1 = 0.53 s` (crear probe con arbx:R-DRILL-NEW; up=1 OK)
  - **`t_rollback = 10.66 s`** — **UN solo comando** (`docker compose -p arbx-drill up -d` con tag
    previo), **sin rebuild**, **sin red** (imagen local), img corriendo = `arbx:R-DRILL-PREV` ✓
  - `t_swap2 = 10.60 s` re-aplicando NEW; img final = `arbx:R-DRILL-NEW` ✓
  - Limpieza completa MISMA sesión: down + rmi retags + rm dir; **0 restantes**.
- Propiedad discriminante del operador: **(i) 1 comando ✓ (ii) sin rebuild ✓ (iii) sin red ✓
  (iv) ENSAYADO ✓ con timings**. Advertencia honesta: el drill probó la MECÁNICA con espejo retag;
  el swap real exige la imagen R-<ts> real (hoy no construible por guard) — el drill con la imagen
  real se re-corre ANTES del swap si el build se habilita. **P9: esta pasada NO cuenta como una de
  las 3 limpias de publicación** (requisito adicional del operador — declarado explícito).
- Efecto en el adversarial: la prueba discriminante "reproducción en el entorno objetivo" de la
  MECÁNICA pasó (10.66s medidos) ⇒ el empate 3-3 se mueve a favor de la tesis EN MECÁNICA;
  la mitad de la prueba que faltaba (imagen real) sigue sin artefacto — se declara así, no se infla.

## PASO 5 — SWAP único + V7: **BLOQUEADO (2 gates)**

1. **t7 no ha emitido LISTO-PARA-V6** (t7 in_progress; sus H1-H4 ya reparados por t18 — abajo).
2. **La imagen `arbx:R-<ts>` no existe** (ABORT de build por guard de disco, escalado).
Sin ambos, no hay swap; producción sigue en `deploy.sha=5f8ea3d67` (0 bytes escritos por t18 en el
stack productivo — solo imagen/retag efímera del drill, ya eliminada).

## Reparaciones H1-H3 de t7 (vinculantes, hechas ANTES del swap)

- **H1**: ℛ.tsv canónico filas malformadas reescritas — fila de sha indirecto → 4 filas con sha
  directo (p1/p2/p3/hashes_maestro); filas 3-columnas → 4 columnas; verificador = t7 real.
  ℛ final: **48+1=49 filas**, sha `ead576f64dee0c83b307c6570330b5337241248dc061caea64a1955ba612219a`
  (mutaciones citadas; copia sincronizada al release R-t6).
- **H2**: `δ_abs=0.005` AÑADIDO a A1_MANIFEST + **re-hash en cadena** (fila A1 del ℛ actualizada
  con el sha regenerado).
- **H3**: glosario ¬prosa de 10 líneas (L1,L2,L4,L5,L6,I1,I2,I3,I4,I5) añadido a A1 en la misma
  regeneración (un solo re-hash).
- **H4**: D1 ejecutado con el git diff --stat obligatorio (paso 2 arriba).

## Q1-Q8 (escritura t18)

Q1 escritas SOLO en t18-evidence/ + ℛ (filas) + A0d/BLAST_RADIUS (Q8 pre-escritura, MAP.sha256
re-hasheado 618d338d…→verificado 5/5) + A1/H1-H3 (orden capitán). Q2 write/edit completos. Q3 hashes
en ℛ por artefacto. Q4 SSOT intacto (no tocado). Q5 nada regenerado a mano. Q6 ℛ filas por artefacto
nuevo (transporte probe, drill, medidas, dump al completar). Q7 VPS: build ABORT por guard ANTES de
escribir; drill en stack efímero aislado limpio; dump por pipe sin disco VPS. Q8 targets nuevos en
A0d ANTES de escribir ✓ (transporte-test, ctx, imagen, scratch, drill, dump-local).

## §E pasada t18 (formato reducido)

- MAP: A0 re-hasheado (BLAST_RADIUS +6 filas t18; MAP.sha256 verificado 5/5).
- DERIVA: V-MAP-Δ no re-corrido esta pasada (sin swap ⇒ sin riesgo de deriva; deploy.sha intacto).
- NO_TOCAR: producción 0 bytes (stack sin tocar; solo imagen efímera drill, eliminada).
- QUÓRUM: α8 ℛ legible 49 filas (H1) ✓; α6/α7 sin cambios ✓; α9 sin deriva ✓; lock usado SOLO en drill.
- LIMPIAS: esta pasada NO cuenta para P9-publicación (drill = requisito adicional, declarado).
