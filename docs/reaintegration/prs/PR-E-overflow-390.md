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
