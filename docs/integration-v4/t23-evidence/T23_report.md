# T23 — Piezas 2-4 + señal 5/5 · reporte (α9 VIGIL) · attempt 8376c747

> 2026-10-13 17:2x–18:0x UTC · regla permanente aplicada (tracciones foreground cmd /c / nohup+poll;
> JAMÁS job-background-ssh) · 0 reintentos usados (ninguna pieza murió) · PAPER ON intacto.

## Resultado global: **5/5 piezas F4+F1 VERDES — dump por piezas COMPLETO**

| pieza | tabla(s) | bytes | sha256 (local==VPS) | TOC | F1 conteo (dump vs origen-en-punto) |
|---|---|---|---|---|---|
| 0 resto | todo menos 4 grandes | 47,387,312 | `aa4d4fc7…58f64` ✓ | 688 | restore completo: 170==170, 2339==2339 EXACTOS; simulations 294,870 = punto dump 17:07:02Z |
| 1 route_discovery* | madre+p13+p14/15+rollup_5m | 329,584,156 | `af158dfa…d605` ✓ | 68 | restore exit=0 0 errores; rollup 2,434,138==2,434,138 EXACTO; p13 en punto 17:07:38Z |
| 2 opportunities | opportunities | 1,772,937,116 | `21a10d08…b443` ✓ | 19 | schema exit=0 0 err; **stream 11,208,139 vs punto 11,208,211** (Δ72 = borde MVCC, 0.0006%, declarado); última fila coherente |
| 3 scored | scored_opportunities | 627,870,540 | `bdc00f07…dbf8` ✓ | 20 | schema exit=0 0 err; **11,542,424 == 11,542,424 EXACTO** |
| 4 pool_reserves | pool_reserves | 4,864,476,896 | `ee415e26…b983` ✓ (sha del FINAL; el 7eab… previo era del parcial en generación — falso mismatch resuelto re-midiendo) | 12 | schema exit=0 0 err; **70,032,394 vs 70,032,393** (Δ1 en 70M, borde) — punto extraído de la PROPIA última fila del dump (17:38:24.51327 == TOC) |

Total dump local: ~7.64GB comprimidos (82GB lógicos) en `C:\Users\HFRC\Desktop\arbx-backups\`
(5 piezas con sha en ℛ). Mecánica por pieza: nohup VPS → status+sha VPS → tracción foreground
`cmd /c` (1.1–8.5 min por pieza, ~10MB/s) → sha local==VPS → TOC por head-streaming →
schema-restore al scratch + conteo-streaming (`pg_restore -f - --data-only | awk`) contra
origen-en-punto-temporal (patrón piece-0; punto = timestamp del TOC o de la última fila).

### Detalles fail-honest del camino

- `pg_restore` desde stdin exige `-f -` para salida (error literal capturado y corregido).
- Escapes anidados pwsh→cmd→ssh→docker→awk: resueltos transportando scripts (cnt.awk, q*.sql)
  byte-exactos por tar-over-ssh — cero escapes.
- El primer `pkill -f heartbeat` se suicidó (matcheó su propio cmdline) y el segundo intento
  cortó una sesión ssh (exit 255): el heartbeat se mató por PID NUMÉRICO (7308) y la sesión
  siguiente verificó OK.
- Limpieza final total: `/opt/arbx-pieces/` VACÍO y directorio ELIMINADO, scratch
  `arbx_restore_test` DROPpeada, `/tmp/t23` y `q*.sql` removidos (limpieza_final_v3.txt).

## df final: 88% / 18,317MB libres — vs ruling α11 (piso 18.7GB): **NO alcanzado por 375MB**

El día siguió creciendo (~+400MB) durante la generación de piezas (p13 viva). El ruling dice
"build PROCEDE a ≥18,7GB con df-watchdog vivo (abort <5GB) + cero dumps concurrentes". La señal
al capitán declara: **5/5 VERDE + release transportado + df 18.3GB (375MB bajo el piso)** —
proceder AHORA es defendible bajo el watchdog vivo del propio ruling (que sirve el propósito del
guard), PERO la letra del piso no se cumple en este instante: decisión del capitán/operador
(proceder con watchdog / esperar vacuum-04:17Z).

## RULING COLCHÓN α11 (documentado para t20 y el re-despacho)

- Guard estricto 1.5× (builder 14-18GB ⇒ 21-27GB libres) FALLA a ~18GB.
- Evidencia empírica: el deploy de HOY 06:16Z (deploy.sha 5f8ea3d6, /api/status id 38353373370)
  cupo con MENOS espacio ⇒ footprint real del build ≤ ~12GB.
- **Ruling: build PROCEDE a ≥18,7GB libres con df-watchdog EN VIVO (abort inmediato si <5GB
  libres) + CERO dumps concurrentes durante el build. El guard 1.5× queda SUPERSEDO-POR-MEDICIÓN
  para el build ÚNICAMENTE (guards de dump conservan 1.5×).** Estado al cierre: 18,317MB — bajo
  el piso por 375MB (declarado arriba).

## V-MAP-Δ periódico

- Pasada 6 (17:5xZ): **deriva=0** (G2 0-failed, puertos 22/80/8787, /opt/arbx ausente, HEAD
  5f8ea3d6, untracked = 3 previos + docs/integration-v4/ del release transportado — DECLARADO).
  `docker compose ls` no parseado en p6 por CRLF residual (fallo cosmético, salida capturada);
  cobertura del invariante compose por pasada 5 (running(25), hace ~40min).
- Producción sin mutación más allá de lo sancionado: piezas+scratch+heartbeat (todo limpiado),
  DROP p20261012 (sancionado), release transportado (sancionado). PAPER ON intacto.

## Señal

**5/5 VERDE + restore-test 5/5 + release transportado + ℛ 66 filas + df 18.3GB (piso 18.7 por
375MB — decisión capitán).** ⇒ re-despachar t20 (attempt 2) con la regla permanente de tracciones
y este ruling en el paquete.
