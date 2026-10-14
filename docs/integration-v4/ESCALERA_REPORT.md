# §E — ESCALERA_REPORT (t6) · V0-V8 · Φ · QUÓRUM §7 · P9 · Λ_vps

> Productor: qa/t6 (α9 VIGIL + α10 SCRIBA) · attempt 84db9165-35d5-4e4a-addc-7ac6ad910cf0 ·
> 2026-10-13 15:36–16:1x UTC · rama `feat/exp-20261013` @ 21feefb32 (base limpia del release).
> Release: **R-20261013T1545Z-t6** (sello A9 = sha256 V1_hashes `ab9b54d4…ae9f43`).
> Regla dura respetada: **NADA SE PUBLICA HASTA V5 VERDE** — V5 verde; V6 BLOQUEADO (decisión operador).

## Escalera V0-V8 — estado por paso

| paso | qué | resultado | evidencia |
|---|---|---|---|
| V-MAP-Δ ×3 (α11-D) | G2/G4/G6 + git + compose vs A0, 3 pasadas | **deriva=0 (3/3 limpias)** | t6-evidence/vmap_delta_p{1,2,3}.txt + `tools/vmap_delta.py --check-artifacts` rc=0 |
| V0 | snapshot + baseline con XREF + condición F4 | **OK** | V0_baseline.md: Λ 7/7 OK, killswitch off, deploy.sha==git==A0; feed 35 items 100% rechazo, 0 viables; F4: t17 3/26 (máx 9.15 USD) registrada, re-check V0 → 0/35 (intermitente, condición conocida) |
| V1 | hashes A1..A9 == A1 GENERADOR + A0∈A1 | **OK** | V1_hashes.txt (31) + A1_MANIFEST.md (tabla §11-bis completa, 15 filas) |
| V2 | estáticos + enmienda cond7 + DFAC + DERIV_POS + IMP≥BASE + alertas t8 | **OK** | V2_static.txt (10 checks): phi_static 7/7 exit=0 · 𝒞 canónica rc=0 · DFAC 5698976f… · deriv_pos=3560 · 31744≥19744 (+60.778% t8) · alertas cargadas |
| V3/V4 (baseline compuesto §17) | batería completa | **PASS — 0 fallos nuevos** | V3_build.txt: cargo 1431P/3F/5I vía t10 (conjunto idéntico; local AppControl 4551 ×2 DECLARADO) · frontend 1418/1418 propio · api-server 937/937 propio (flake 1ª corrida documentado) · t13 10/10 propio · guard pii 6 timeouts (t3) citados |
| V5 SHADOW | Φ(shadow)=1 ∧ x(θ) tolerancia | **VERDE** | V5_shadow.md: Φ=1 (g1-g6 exit=0 + g7 inspección 10/10) · x(θ): viables 0 vs 0, gas Δ0.0050≤δ_abs, deploy.sha sin cambio · 0 bytes VPS |
| V6 SWAP | git+rebuild compose | **BLOQUEADO** | 2 decisiones operador SIN respuesta (topología; backup R9) + adversarial EMPATE 3-3 (ver §V6) |
| V7 POST | — | PENDIENTE de V6 | runbook §V6-RUNBOOK |
| V8 ROLLBACK §B | — | ARMADO (sin esperar) | runbook §V6-RUNBOOK paso 6 |

## Φ := ⋀ g₀..g₇

- g1 strategies==264 ✓ · g2 detectors==60 ✓ · g3 combos==1436 ✓ · g4 DERIV_POS=3560>0 ✓ ·
  g5 IMP 31744 ≥ BASE 19744 ✓ · g6 delta +60.8% (A1) / **+60.778% MEDIDO** (t8) ≥ +58% ✓ —
  instrumento `phi_static.py` exit=0 (g1..g6 SOLO del MANIFEST origen, anti-circularidad §7).
- g7 cap nuevas +3 (INSPECCIÓN) ✓ — ADM-52/54/55 (t13) en `adm/`, unittest 10/10 re-ejecutado.
- **Φ(paquete R-nuevo) = 1.** (Φ del sistema ACTIVO se re-evalúa en V7 tras V6.)

## QUÓRUM §7 (antes de V6) — 5 condiciones + 2 condiciones t17

| condición | estado | evidencia |
|---|---|---|
| α6 𝒞=1 (enmienda cond7, mask ⊇ {2}) | ✓ | c_evaluator.py --canonical rc=0 (persistido y demostrado) |
| α7 NO_REFUTADO F1-F12 | ✓ | FALSATION_REPORT.md 779bea98… (F12-agente NO CONFORME registrado en t17 evidence_note — NO se suaviza) |
| α9 deriva=0 | ✓ | V-MAP-Δ ×3, total_drift=0 |
| α8 hashes∈ℛ | ✓ | ℛ.tsv canónico 31→41 filas (mutación t6 citada; sha canónico 5845edf0…) |
| lock L (flock /tmp/arbx-deploy.lock) | **PENDIENTE** | solo se toma al ejecutar V6 — V6 bloqueado ⇒ lock NO adquirido |
| (a) L3/F4 'no refutado, NO MEDIBLE' | ✓ registrado | V0_baseline.md §F4 + A7 |
| (b) 𝒞 y V-MAP-Δ EJECUTABLES persistidos | ✓ | tools/ con demostración rc=0 en esta sesión |

**Quórum: 6/7 satisfechas; la 7ª (lock L) es consecuencia directa de ejecutar V6.**

## §I Q1-Q8

En `releases/R-20261013T1545Z-t6/Q1_Q8.md` (8/8 OK; Q8 = V6 bloqueado por decisión operador, no ABORT de instrumento).

## P9 — contador de pasadas limpias (t7)

**P9 = 3** (tres pasadas V-MAP-Δ limpias, deriva=0; artefactos p1/p2/p3 con hash). Adicionalmente:
Φ gate ×1 limpio, 𝒞 ×2 (canónica rc=0, literal rc=1 esperado), unittest ×1, vitest ×2, F4-recheck ×1.

## Λ_vps — contra /api/status (canónico, t15)

`GET /api/status` → 200 en 0.851 s · ok:true · **7/7 servicios OK** · killswitch.enabled=false (PAPER ON)
· deploy.sha 5f8ea3d67… == git VPS == A0 (**drift=0**) · deploy.at 2026-10-13T06:16:24Z (id 38353373370).
Nota: runtime-status del edge respondió 200 en 8.98 s (lento — registrado como señal, no adaptado).

## §D — ABORT + 3 intentos (fall-honest de instrumentos)

| instrumento | intentos | resultado | resolución |
|---|---|---|---|
| ssh.exe NATIVO | 1 (+t15/t16) | exit 255, stderr 0 B | fallback **Git ssh.exe pineado** (exit=0, 5 sesiones de lectura) |
| cargo test local | **2 literales** | AppControl os error 4551 (TEMP: eyre; Desktop: serde_core+portable-atomic) | instrumento DECLARADO BLOQUEADO (patrón t11-anvil); V3 cubierto por t10 sobre conjunto byte-idéntico |
| `wc -l` remoto en p2/p3 | 2 | `wc: invalid option '\r'` (CRLF del here-string) | invariante cubierto por `docker compose ls running(25)` (parseado por vmap_delta.py) |
| probe x(θ) V5 (1er intento) | 1 | timeout 60 s (borde lento) | reintento por partes exit=0 en 11.4 s |
| sql_query | 1 (t16) | sin filas | NO usado; PG vía wire (runtime-status postgres:ok) |

## No-pérdida (§17)

Diff árbol compartido vs HEAD = HEAD limpio + lista intencional SIN pérdida: ASSEMBLY t9 · seam SSE t4
(+14B) · changedPaths t4/t3/t5/t13 · ADM-n · manifest 264 · t12/t17-evidence · detector_policy t10 ·
A7/A8 t14 · vps-map t15 · XREF t16 · t6-evidence + R-20261013T1545Z-t6. Los 4498 M pre-existentes
(t1) INTACTOS; build en worktree detached, limpiado en esta sesión (R11).

## §V6 — BLOQUEADO (motivo exacto) + adversarial + RUNBOOK

**Motivo exacto (orden capitán):** dos decisiones del operador escaladas SIN respuesta —
(1) **topología** (compose actual `/opt/arbitragex-v2/docker/compose.prod.yml` vs `/opt/arbx` nuevo);
(2) **backup R9** (único backup PG 5 GB del 27-sep vs DB 78 GB — t15 §L-R9).
**Adversarial (regla permanente, ejecutado):** frame + veredicto → **EMPATE 3-3**, las dos pruebas
discriminantes (artefacto del efecto; reproducción en entorno objetivo) SIN resultado — la seguridad
de la ejecución NO es decidible sin ejecutar; V5 verde es necesario, no suficiente. Riesgos R1/R9
cargados como evidencia contra.

### §V6-RUNBOOK (ejecutar SOLO con las 2 decisiones respondidas)

1. Verificar decisiones registradas (topología elegida; backup R9 EJECUTADO y verificado con hash/tamaño).
2. Ventana de mutación: cualquier hora UTC salvo 04:00–05:00 y 06:30–07:30 (+dom 05:00–06:00).
3. Transporte: push del release (hoy P1=0 commits — el push es parte de la decisión de publicación).
4. `ssh`(pineado) arbx → `flock -n /tmp/arbx-deploy.lock -c 'cd /opt/arbitragex-v2 && git pull && docker compose --env-file .env -f docker/compose.prod.yml build --no-cache && docker compose --env-file .env -f docker/compose.prod.yml up -d'` (RULE 03: --no-cache + --env-file SIEMPRE; R3: el watchdog defiere con el lock).
5. W=180 s de estabilización; luego **V7**: V1-V2 sobre ACTIVO + GET /opportunities (card servida, RG-01) + histograma vs V0 (aceptando la condición F4 registrada) + Λ_vps (/api/status deploy.sha NUEVO) + Φ del sistema activo. Cero derivas no declaradas. **+ WATCH-V7-001 (t28/t29): cualquier 500 en {paper-mode/state, readiness*, runtime-status, scanner/heartbeat, opportunities/live} durante V7 ⇒ reproducible ⇒ investigar antes de V8.**
6. **V8 (§B, sin esperar)** ante cualquier V7 rojo: `git checkout 5f8ea3d67…` + rebuild + up -d; re-verificar Λ; reporte §L del incidente. **ENMIENDA §B (t29, defecto de diseño hallado por el operador — C4): el rollback original verificaba LIVENESS (restarts=0, 25 Up, 7/7) NO CORRECTITUD DE RUTAS. Desde t29, TODO rollback (V8) incluye OBLIGATORIAMENTE un SMOKE DE RUTAS post-revert: ≥50 GETs por endpoint del control-plane {paper-mode/state, readiness, readiness/decision, readiness/steps, strategies/runtime-status, scanner/heartbeat, opportunities/live} con distribución de códigos — un 200-only cierra el rollback; cualquier no-2xx reabre el incidente. La enmienda también aplica a WATCH-V7-001.**

## Veredicto global t6

**OK hasta V5 (todo medido, todo con artefacto y hash); V6 BLOQUEADO con motivo exacto —
entrega completa, publicación espera únicamente las dos decisiones del operador.**
Producción NO tocada (0 bytes VPS escritos; deploy.sha sin cambio en el probe final de V5).
