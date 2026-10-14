# T22 — DESBLOQUEO-V6 · reporte (α9 VIGIL) · attempt 1994c190

> 2026-10-13 17:0x–17:3x UTC · Git ssh.exe pineado · PAPER ON (killswitch=false sin tocar) · P1 intacto.

## (1) MATAR ZOMBI: VERDE (con corrección de cronología)

- Diagnóstico propio ANTES de matar (zombi_antes.txt): PID 659745 `pg_dump -Z3 -Fc` ELAPSED 11:34,
  CPU 6:35 creciendo, `COPY route_discovery_outcomes_p20261013` ACTIVO en pg_stat_activity —
  era el proceso de mi job-2 cuyo LECTOR había muerto: `.part` congelado en 233,738,240 bytes
  (lectura 25s: 0 MB), **0 procesos git-ssh locales**, job pwsh-12 "unknown". Es decir: el
  pg_dump VPS quedó escribiendo AL VACÍO (pipe sin lector, shim docker exec sin SIGPIPE) —
  exactamente el patrón de t20, confirmado por medición propia esta vez.
- **kill -9 659745** → verificado MUERTO en host y contenedor; sesiones pg_dump en PG: NINGUNA
  (kill_zombi_y_drop.txt, salida literal completa).

## (2) DROP SANCIONADO p20261012: VERDE

- `DROP TABLE route_discovery_outcomes_p20261012` (guard rollup-first verificado 2×: mío +
  el del propio script :338-349). **df: 93% (10,492MB) → 88% (18,692MB) = +8.2GB físicos al SO**
  (los 10.4GB eran pg_total_relation_size lógico; el unlink devuelve los bloques reales — declarado).
- Meta "≥21GB libres": **18.7GB — NO alcanzada** (asumía 10.4GB físicos). Margen para el build:
  builder transitorio ≤14-18GB empírico (deploy de hoy cupo con ~18GB) ⇒ viable JUSTO, sin colchón;
  decisión de margen al capitán/operador. Restantes: p20261013 (viva) + p14/15 (vacías) ✓.

## (3) DUMP POR PIEZAS: causa IDENTIFICADA + 2/5 piezas F4+F1 VERDES + pieza-2 en vuelo

**DIAGNÓSTICO DE CAUSA (obligatorio tras 3ª mortalidad) — causa (b) confirmada por canario:**
- 3/3 jobs del harness con `ssh` de stdio redirigido murieron/colgaron sin output final
  (pwsh-11: truncado 3.87GB; pwsh-12: congelado a los ~3min con zombi VPS; pwsh-13 CANARIO
  `sleep 900`: sigue "running" >28min — colgado).
- 4/4 operaciones con el patrón **nohup-VPS + tracción foreground `cmd /c`** funcionaron
  perfecto (piezas 0/1 completas + transporte release). ⇒ el patrón mortal es
  **ssh.exe Git redirigido dentro de background jobs del harness** (hipótesis (b) del capitán,
  con componente (a) indistinguible); la mitigación estructural ya está en producción de piezas.

| pieza | contenido | estado | F4 (sha + TOC) | F1 (restore + conteo==origen) |
|---|---|---|---|---|
| **pieza-0 resto** | todo menos 4 grandes (47,387,312 B) | ✓ COMPLETA local | sha `aa4d4fc7…58f64` local==VPS; 688 entradas TOC (CUSTOM 1.14) | restore `--section=pre-data+data`: paper_trade_runs_daily **170==170** ✓, token_safety_cache **2339==2339** ✓, simulations 294,870 == punto temporal del dump 17:07:02Z (origen posterior 295,049 = tabla viva ~17 filas/min — consistente); errores SOLO en vistas dependientes de tablas excluidas (esperado en piezas parciales) |
| **pieza-1 route_discovery*** | madre+p13+p14/15+rollup_5m (329,584,156 B) | ✓ COMPLETA local (rm VPS verificado) | sha `af158dfa…d605` local==VPS; 68 TOC | restore **exit=0, 0 errores**; rollup **2,434,138==2,434,138 EXACTO** ✓; p13 11,811,120 == punto temporal dump 17:07:38Z (origen crecido a 11,862,948 en la ventana — tabla viva ~137 filas/s) |
| **pieza-2 opportunities** | 31GB lógico | **GENERANDO en VPS** (nohup, pid 704935; 213MB a las 17:18Z) | pendiente | pendiente |
| pieza-3 scored / pieza-4 pool_reserves | 20G/16G lógico | pendientes (secuenciales) | — | — |

Mecánica por pieza (replicable): nohup VPS `pg_dump -Z3 -Fc -t/-T …` → status+sha256 VPS →
tracción `cmd /c ssh cat > local` (reanudable con dd skip si muriera) → sha local==VPS →
pg_restore --list (head 2MB→contenedor) → restore-test scratch `--section=pre-data+data` con
conteo==origen → `rm` pieza VPS (verificado). Máx 3 reintentos/pieza (§D) — 0 usados.

## (5) TRANSPORTE DEL RELEASE: VERDE

- tar-over-ssh del árbol `docs/integration-v4/releases/R-20261013T1545Z-t6` →
  `/opt/arbitragex-v2/docs/integration-v4/releases/` (creación limpia, como declaró t9-V4).
- **27/27 archivos; hashes verificados idénticos**: V1_hashes `ab9b54d4…` (sello A9),
  SSOT `strategy_hop_map.json` `7fb529c6…` (release_local_manifest.txt con manifiesto completo).

## V-MAP-Δ periódico

- **Pasada 5: DERIVA=0 COMPLETA** (G2 0-failed, puertos 22/80/8787, /opt/arbx ausente, HEAD
  5f8ea3d6, compose arbitragex-v2 running(25) compose.prod.yml). Deriva DECLARADA: untracked
  `?? docs/integration-v4/` = el release transportado (sancionado, fila A0d); los 3 untracked
  previos intactos.

## Q8/A0d

BLAST_RADIUS revisión t22 (+3 filas: /opt/arbx-pieces/, nohups+heartbeat, DROP ejecutado) ANTES
de escribir; MAP.sha256 re-hasheado (`47686d9e…`) y verificado.

## Pendiente para cerrar t22 (paso 6 NO procede aún)

1. Pieza-2: tracción+F4+F1 cuando termine de generarse (nohup inmune; el archivo VPS es el
   seguro intermedio).
2. Piezas 3 (scored) y 4 (pool_reserves): mismos pasos.
3. Con 5/5 piezas F4+F1 + df + release transportado ⇒ mensaje para re-despachar t20 (attempt 2).
t21 sigue NO procedente. La causa (b) hace que CUALQUIER tracción deba ser foreground `cmd /c`
o nohup-VPS+poll — jamás job-background-ssh.
