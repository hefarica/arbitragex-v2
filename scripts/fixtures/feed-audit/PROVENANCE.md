# Fixtures de `scripts/checks/feed-audit.py` — procedencia y controles

Un instrumento de medición sin control positivo es un instrumento que no se
sabe si mide. Estas tres fixtures fijan su comportamiento y **no se tocan para
obtener verde**: si un control cambia de resultado, cambia el instrumento y se
explica por qué.

| fixture | tipo | exit esperado | qué prueba |
| --- | --- | --- | --- |
| `positive.json` | sintética | **0** | todas las puertas pueden cumplirse a la vez (si no, el instrumento es inquebrantable) |
| `negative.json` | real, congelada | **1** | los bytes medidos del wire el 2026-10-21 producen violaciones |
| `empty.json` | sintética | **2** | población 0 ⇒ NOT COMPUTED: el vacío NUNCA es «OK» |

Se corren con `python3 scripts/checks/feed-audit.py --controls` (offline, sin
red) y en CI con `.github/workflows/feed-audit.yml`.

## Procedencia

**`negative.json`** — payload completo del endpoint público de oportunidades,
`order=profit_usd&route_representative=best_net`, `limit=200&max_age_seconds=86400`.

- fuente: `GET https://edge-arbx.ape-tv.net/api/opportunities/live?viable_only=false&limit=200&max_age_seconds=86400&order=profit_usd&route_representative=best_net`
- capturado: `2026-10-21T06:01:30Z`
- `payload sha256`: `05f380836621b9f8…` (los 16 primeros dígitos, como los imprime la captura)
- `window_total` en la captura: 266 · filas servidas: 200 · `viable_only=true` → `count=0`
- Son datos de mercado reales de un endpoint público (direcciones de tokens,
  precios). Congelada a propósito: la corrida del control es determinista
  aunque el wire cambie entre lecturas — de hecho cambia (ver «Límites»).

**`positive.json`** — 3 filas construidas a mano. **No contiene datos de
mercado reales**: los importes son inventados a propósito. Las direcciones son
tokens y pozos canónicos de Ethereum (WETH/USDC, pozos de Uniswap v3/Uniswap v2)
para no introducir direcciones centinela. Vive fuera del camino de producción:
ningún pipeline la lee.

**`empty.json`** — forma medida del endpoint cuando no hay detecciones.

## Controles y recuento esperado

| control | exit | violaciones | puertas NOT COMPUTED |
| --- | --- | --- | --- |
| `positive` | 0 | 0 | 0 |
| `negative` | 1 | 7 (G02, G03, G04, G08, G10, G11, G12) | 0 |
| `empty` | 2 | — (población 0) | todas |

### Reconciliación con la corrida v1 del 2026-10-21T05:55:29Z (8 violaciones)

El recuento **baja de 8 a 7 y no es un ajuste para verde**: es la aplicación de
dos decisiones del dueño del instrumento, y cada cambio tiene su razón.

| v1 | v2 | qué pasó |
| --- | --- | --- |
| G8 marcaba 12 filas (3 imposibles + 9 en la banda 5 %–100 %) | G8 marca **3** (sólo `ratio > 1`) | la banda pasa a **G09, diagnóstico declarado**: un ratio < 1 no es anomalía mientras el campo publicado no declare unidades (si publica el importe de salida, es lo esperado). El tripwire `> 1` sí es definicional: un ida y vuelta autofinanciado no devuelve más que su nocional |
| G5 anclaje (49/200) y G6 confianza (0/200) contaban como violación | **COBERTURA** | no hay requisito declarado sobre ellas: se declaran, no se exigen |
| G7 «sin bloqueo de allowlist» contaba como violación | **censo**, no puerta | la allowlist está congelada por decisión; bloquear no es un defecto del instrumento |
| — | **G10** política apaga el ranking: 17 fallos | nuevo: una fila rechazada por política seguía compitiendo por el top |
| — | **G11** nocional reproducible: 0/200 | nuevo: `token_prices_usd` llega sin instante de precio, así que el nocional cambia entre lecturas |
| — | **G12** el orden publicado: violación | nuevo y medido: el orden sólo lo explica `COALESCE(net_expected, expected)` (n=30), no el campo que se publica |
| — | **G13** traza aritmética del net: OK 13/13 | nuevo: los 13 nets publicados salen de su propio breakdown; los suelos por fila van de **93 a 1248 bps** (el suelo global de 93 bps es decorativo) |

El control negativo sigue en **rojo**, que es lo que debe probar.

## Umbrales — versionados con el instrumento

`th/2026-10-21.1`. Cada corrida imprime su `sha256[0:12]` y esta versión. Un
umbral sólo cambia con una versión nueva del archivo.

- `ratio_imposible = 1.0` — definicional, sin tuning.
- `ratio_banda = 0.05` — sólo delimita el diagnóstico G09; no decide nada.
- `suelo_global_bps = 93.0` — **referencia de comparación**, prohibida como
  compuerta: el suelo real es por fila y sale de su propio breakdown.
- `exige_price_ts = True` — un nocional sin instante de precio no es reproducible.

## Límites del instrumento (lo que NO mide)

- **No lee la base de datos.** Mide la superficie servida; una fila almacenada
  que no se publique no aparece aquí.
- **Ventana máxima 24 h**: `max_age_seconds` se recorta a 86400 (verificado), y
  la respuesta se corta en 200 filas aunque `window_total` sea mayor: toda tasa
  que se publique desde aquí tiene que decir su denominador.
- **El payload no es estable entre llamadas** (mismo tamaño, `sha256` distinto a
  18 s de distancia): cada cifra está atada a su instante de captura.
- **No explica el 7,008·10¹⁰.** El tripwire lo bloquea; la fórmula que lo genera
  sigue abierta y no se rellena con una hipótesis.
- El UA de navegador en las lecturas de red es condición de acceso: el WAF del
  edge responde 403 al UA por defecto de `urllib` (medido).
