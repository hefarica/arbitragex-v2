# ROUTES-FOUND-0 — Causa raíz DEFINIDA y verificada (2026-09-16 ~23:15Z)

## Veredicto

`routes_found=0` sostenido NO es un grafo muerto ni un problema de config de base_tokens.
Es un defecto de diseño del DFS acotado: **el presupuesto global de trabajo (100.000
visitas de arista, compartido entre TODOS los starts) se agota en los primeros tokens
hoja (grado 1) antes de alcanzar cualquier hub con ciclos.**

Evidencia viva (tick productivo, servido público en https://arbx.ape-tv.net/api/route-discovery/tick?chain_id=1):
- `edges_built=374` (grafo VIVO) · `routes_found=0`
- `discovery_edge_visits=100000` · `discovery_work_limited=true`  ← la firma exacta del bug
- route_scanner: `cycles_found=0, capped=true, enumeration_ms=5-12`

## Mecanismo (reproducido bit a bit en local con los datos vivos)

Código: `unique_route_finder.rs:435-462` (route_discovery worker) y
`multi_hop_search.rs:183-208` (route_scanner) — ambos:
1. starts = TODOS los tokens del grafo (base_tokens vacío), ordenados por DIRECCIÓN (`sort_unstable`).
2. Presupuesto GLOBAL `max_edge_visits=100_000` compartido entre todos los starts.

Grafo real reconstruido desde Redis (235 snapshots frescos) + PG (237 pools activos):
- 198 pools aceptados → 396 aristas → 122 tokens, de los cuales 93 (76%) son HOJAS (grado 1).
- Una hoja NO PUEDE cerrar un ciclo simple (requeriría reusar su único pool), pero a
  `max_depth=7` su subárbol cuesta decenas de miles de visitas.
- Los primeros 3 starts por dirección (0x0267…, 0x03ec…, 0x0604…, todos grado 1) queman
  35.068 + 35.068 + 29.864 = 100.000 visitas → budget muerto → WETH (grado 170, dueño
  de los ciclos, dirección 0xc02a…, ordena al final) NUNCA es arrancado.

Réplica exacta (misma semántica Rust, mismos datos): `routes_found=0, edge_visits=100000, work_limited=true`.
Contraste: mismos datos, starts desde hubs → **29.709 ciclos cerrados** (280 de 2 hops).
Contraste 2: misma semántica con `max_depth=3` → **routes_found=502** (las hojas solo queman ~858 visitas c/u a profundidad 3 y el budget llega a los hubs).

## Por qué colapsó el 2026-09-16 (01:00–11:00Z) y no antes

- `.env:34 ARBX_ROUTE_DISCOVERY_MAX_DEPTH=7` existe desde el 2026-09-07 (stat mtime del
  .env 12:12Z 07-09; commit 1c020aeb documentaba "max_depth=5 (7 env override)").
- El 07-09 el discovery seguía vivo porque la COMPOSICIÓN del universo fresco era otra
  (outcomes is_opportunity ≈ 16.800/hora hasta el 16-09 01:00Z; colapso a 0 entre 01:00
  y 11:00Z del 16-09, tabla route_discovery_outcomes).
- Disparador: entraron al universo de reserves frescas pools long-tail cuyos tokens hoja
  ordenan por dirección ANTES que los hubs y cuyos subárboles a profundidad 7 son
  gigantes. Es determinístico: por eso los 3 restarts (21:54Z, 22:20Z, 22:49Z) no lo
  "arreglaron" — no es estado, es algoritmo × datos.

## Refutaciones (hipótesis descartadas con evidencia)

- `base_tokens=0 en config`: FALSO como causa. Código: base_tokens vacío ⇒ usa TODOS los
  tokens. Además base_tokens=0 estaba al boot (log 22:50:13Z) y el DFS sí corrió.
- hop_mask MEV-01-001 bloqueando: FALSO. `hop_mask("MEV-01-001")=Some(63)` (0b111111, hops 2-7 permitidos).
- Grafo sin ciclos: FALSO. El MISMO grafo contiene 29.709 ciclos.
- Mala configuración de freshness: NO es la causa del 0 (el grafo construye 374 aristas);
  sí poda 81 edges (missing_slot0 38, missing_token_metadata 37, low_liquidity 6).

## Daño colateral explicado

Sin rutas ⇒ sin candidatos ⇒ sin oportunidades nuevas ⇒ burst v3_quote_unavailable
colapsa también; sims/paper/heredan el hambre. Este bug es UPSTREAM de todo el embudo.

## Propuesta de blindaje (sin tocar nada — requiere autorización para implementar)

1. FIX del defecto (PR con ID de anomalía, §37 P-∅): starts ordenados por grado
   descendente (hubs primero) o filtrar starts con grado ≥2 (una hoja jamás cierra ciclo
   simple); presupuesto por-start o reserva para hubs; `DEFAULT_MAX_EDGE_VISITS` escalado
   a edges_built×profundidad. Nivel 1 de congelación (route-discovery): cambio quirúrgico.
2. GATE nuevo en CI (doctrina §37 "todo incidente cierra con revert + gate nuevo"):
   test de regresión con fixture con forma de prod (hub + ≥90 hojas con direcciones
   bajas) a max_depth=7 que exija routes_found>0. guarantees.rs hoy solo prueba grafos
   minúsculos con config default (depth 3) — por eso CI no atrapó esto.
3. ALERTA fail-loud: Prometheus+Alertmanager ya corren; regla: routes_found=0 ∧
   edges_built>0 ∧ discovery_work_limited=true por N ticks consecutivos ⇒ page. El
   sistema ya emite la señal honesta — solo falta que alguien la escuche.
4. CANARY post-deploy en el deploy-veraz: tras cada deploy, 3 ticks con
   edges_built>0 ⇒ exigir routes_found>0 o razón de no-starvation.
5. GOBERNANZA del .env (lo que "se está moviendo"): el .env del VPS no está versionado
   ni auditado; los cambios de knobs (p.ej. =7 el 07-09) viajan sin ID de anomalía.
   Propuesta: attestation de checksum de .env en cada deploy + diff obligatorio contra el
   último attestado; restarts solo con deploy-veraz (3 restarts esta noche no cambiaron
   nada porque el bug es determinístico).

## Artefactos de evidencia

- Datos vivos: $LOCALAPPDATA/Temp/arbxgraph/ (pools.txt 237, reserves.txt 116, slot0.txt 119)
- Réplicas: analyze.py (29.709 ciclos desde hubs), exact_dfs.py (0 rutas, semántica exacta;
  502 con depth 3)
- Tick público: dominio vivo sirve discovery_edge_visits=100000/work_limited=true (HTTP 200, 0.62s)
