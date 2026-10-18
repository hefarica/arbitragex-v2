# PR-E — overflow a 390px, por cluster (2026-10-18) — **GATE ROJO: no publicar tal cual**

**Gate:** `scripts/gates/g-no-overflow-390-01.sh` → **FAIL** (queda **1 ruta de 58**).

## Resultado medido (local, build propia, baseline pre-fix del MISMO entorno)

| | rutas con overflow > 2px a 390 |
|---|---|
| **antes** (baseline local pre-fix) | **13 de 58** (peor `/live-readiness` 915 px) |
| **después** | **1 de 58** — `/omega-s5/factory` **26 px** |

Anti-trampa (condición 2): **PASS** — ninguna ruta pierde más del 5 % de su texto respecto a la
línea base, o sea que no se escondió contenido con `overflow-x:hidden` ni `display:none`.

## Los 4 clusters, y qué pasó con cada uno

1. **`/omega-s5/*` (6 rutas que comparten layout)** — tablas anchas sin contenedor que las desplace.
   Las 7 tablas de `app/omega-s5/*/page.tsx` se envolvieron en `<div className="mt-4 overflow-x-auto">`.
   **5 de 6 rutas cerradas**; `/omega-s5/registry` necesitó además `min-w-0` en la rejilla y
   `break-all` en el texto mono (`arbx:config:rpcs:reload` no partía). Queda **`/omega-s5/factory` (26 px)**.
2. **`/onboarding/*`** — la fila `flex items-center gap-3` (botón + `Badge` largo) no envolvía.
   Arreglo en `/onboarding/1-init` (`flex-wrap` + `whitespace-normal`). **Cerrada.**
3. **`/live-readiness` (915 px)** — la rejilla `mt-5 grid gap-3 md:grid-cols-2 xl:grid-cols-4` no podía
   encogerse (items con `min-width:auto` y un `truncate` dentro fijaba el `min-content`).
   `min-w-0` en la rejilla + `min-w-0 overflow-hidden` en la tarjeta. **Cerrada.**
4. **`/quant` (265 px)** — 4 tablas `w-full border-collapse` con celdas `whitespace-nowrap`.
   Envueltas en `overflow-x-auto`. **Cerrada.**

Extra no listado por vosotros: **`/config` (26 px)** — `flex-shrink-0` impedía encoger la columna;
arreglado con `min-w-0` + `flex-wrap` en la fila. **Cerrada.**

## Lo que queda abierto (y por qué NO se ha cerrado)

`/omega-s5/factory` sigue en **26 px**. Lo descartado **con medida**, no por intuición:

- **No es el ticker**: ocultando `.ticker` el `scrollWidth` sigue en **416** (en `/wallet`, control, da 390 con y sin ticker).
- **No hay elemento con `getBoundingClientRect().right > vw+2` sin ancestro que lo recorte**: el único
  candidato es `.ticker-track`, ya descartado. El desbordamiento viene de un elemento cuyo
  `scrollWidth` excede su rect sin desplazar su caja, así que el detector por rectángulos no lo aísla.

Por eso **no se aplica un `overflow-x: hidden` al contenedor de la página**: sería exactamente el
truco que el gate combate. Se deja declarado para el siguiente paso con una sonda por `scrollWidth`.

## Estado del commit

Commiteado en la rama como **NOT GREEN** para no perder el trabajo (12 de 13 rutas cerradas y el
anti-trampa en verde). **No publicar como PR cerrado**: el gate de PR-E exige las 58 rutas.

## P5-F6 — las 12 rutas de entidad (`/omega-s5/registry/[entity]`)

Barrido propio (48 filas = 12 rutas × 4 viewports) contra el sitio vivo; evidencia cruda en
`docs/reaintegration/evidence/P5-F6-entity-routes.json`.

- **48/48 HTTP 200**, **48/48 SSR 200**, todas con contenido (`body_text_len` mínimo 3.7k), **0 filas con problema**.
- **Overflow a 390: 11 de las 12 rutas desbordan exactamente 152 px** (todas menos `chain`) — mismo
  cluster `/omega-s5/registry` de arriba, con un valor idéntico que apunta a un componente compartido.
  Con estas 12 la matriz pasa de 58 a **70 rutas**; el gate pasará a 70 cuando se cierre el cluster.
- Las 12 entidades salen del registro real (`chain, rpc, dex, token, pool, wallet, strategy,
  contract, risk_gate, capital_gate, relay, agent`), el mismo listado que usa
  `frontend/e2e/operator_sovereignty.spec.ts` (matriz C9.4), no inventadas.

---

# CIERRE (2026-10-18, segunda pasada) — **GATE VERDE sobre las 70 rutas**

## El origen era UNO, no trece

`app/omega-s5/layout.tsx` montaba la fila como `flex gap-6` con una `aside.w-44.shrink-0` (176 px) y
un `gap` de 24 px: a 390 px le dejaba **~190 px** a la columna de contenido, y las tarjetas/tablas de
dentro tienen un `min-content` mayor que eso → la **página** desplazaba en horizontal.
`body{overflow-x:hidden}` no lo evitaba porque el desplazamiento ocurre en `documentElement`.

**Arreglo (una línea):** `flex flex-col gap-6 lg:flex-row`. Por debajo de `lg` la fila se apila
(la `aside` ocupa su propia fila, 176 px < 390); en `lg+` el layout es **idéntico** al de antes.
Sin `overflow-x:hidden`, sin ocultar nada.

Eso cierra **de golpe** el `/omega-s5/factory` (26 px) y las **12 rutas de entidad** (93 px locales
/ 152 px en el vivo), que era la hipótesis "es un arreglo de layout, no once".

## Evidencia (salida real)

```
$ scripts/gates/g-no-overflow-390-01.sh http://127.0.0.1:3166 /tmp/pre70.json
G-NO-OVERFLOW-390-01 — 390px overflow + text-parity anti-cheat
  PASS  (sweep) scanner completed
  PASS  (sweep) result file written
  info  routes compared: 70 (baseline 70, sweep 70)
  PASS  (1) every route has scrollWidth - clientWidth <= 2 at 390
  PASS  (2) no route lost >5% of its text vs the pre-fix baseline
G-NO-OVERFLOW-390-01: PASS
```

- **Baseline pre-fix, mismo entorno** (build local, 70 rutas a 390): **13 rutas** con overflow
  (12 de entidad a 93 px + `/omega-s5/factory` 26 px). **Después: 0.**
- La matriz del gate pasa de **58 a 70 rutas** (`docs/reaintegration/evidence/routes.json`), así que
  al cerrar el cluster se cierra también **P5-F6**.
- `npx tsc --noEmit` **PASS**; `npx vitest run` **155 archivos / 1639 tests, 0 fallos**.

## Estado

**PR-E y P5-F6 CERRADOS.** Revert de un paso: revertir este commit devuelve la fila a `flex gap-6`
y el overflow medido vuelve exactamente a 13 rutas.
