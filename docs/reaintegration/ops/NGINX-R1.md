# NGINX-R1 — el canónico se decide por CLASE de delta, no por "cualquier línea ajena"

**Decisión (2026-10-18):** aflojar R1 para que un delta de **sólo comentarios** no
aborte la instalación del canónico. El aborto se conserva para **drift real**, que
es su razón de existir.
**Gate:** `scripts/gates/g-nginx-r1-01.sh`.

## El defecto

R1 comparaba el fichero vivo con el canónico y abortaba si aparecía **cualquier**
línea cambiada que no fuera un `add_header ... Content-Security-Policy`:

```sh
NONCSP=$(diff -u "$LIVE_REAL" "$CANON" | grep -E '^[+-]' | grep -vE '^(\+\+\+|---)' \
          | grep -vE 'add_header.*Content-Security-Policy')
[ -n "$NONCSP" ] && ABORT
```

Los **comentarios** caían en `NONCSP`. Como el canónico trae comentarios que el
fichero vivo no tiene, R1 **abortaba siempre**: el drift CSP se parcheaba en sitio
y **el resto del canónico no se instalaba nunca**, de forma indefinida y silenciosa
(el job salía verde).

## El arreglo

Cada línea cambiada se **clasifica** y se **imprime** — nada silencioso:

| clase | qué es | ¿aborta? |
|---|---|---|
| `CSP` | `add_header ... Content-Security-Policy` | no (es el drift que R1 existe para matar) |
| `COMMENT` | `^[+-][[:space:]]*#` | **no** (no cambia el comportamiento de nginx) |
| `BLANK` | `^[+-][[:space:]]*$` | **no** (no cambia el comportamiento de nginx) |
| `REAL` | cualquier otra | **sí** |

`SKIP_CANON=1` sólo si `REAL` no está vacío. La salida incluye el recuento por clase
y una muestra de cada una, así que la decisión queda auditada en el log del job.

**Nota de alcance, declarada:** se exime también la línea **vacía**, porque no puede
cambiar el comportamiento de nginx y R1 debe abortar ante *drift real*, no ante
ruido de formato. Queda visible en la clasificación (columna `BLANK`), así que no es
una exención oculta.

## Evidencia (gate, sin VPS)

`G-NGINX-R1-01` **no reimplementa R1**: extrae el bloque R1 **tal cual del workflow**
y lo **ejecuta** contra parejas de fixtures. Prueba el código que se va a desplegar,
no una copia.

```
(a) delta SÓLO de comentarios  -> CSP:0 COMMENT:2 BLANK:0 REAL:0  SKIP_CANON=0  (instala)
(b) delta SÓLO del CSP         -> SKIP_CANON=0                                  (instala)
(c) drift REAL (proxy_pass)    -> CSP:0 COMMENT:0 BLANK:0 REAL:2  SKIP_CANON=1  (ABORTA)
(d) mixto CSP + comentario     -> CSP:2 COMMENT:1 BLANK:0 REAL:0  SKIP_CANON=0  (instala)
```

Y la procedencia del defecto: con el predicado **anterior** el caso (a) daba
**2 líneas "ajenas al CSP"** ⇒ habría abortado. Es decir, el defecto era real y el
arreglo cambia el veredicto.

## Lo que este gate NO cubre (declarado, no simulado)

La **instalación real**, `nginx -t` y el **conteo de CSP por host tras el reload**
necesitan VPS + credencial SSH. Este gate cubre la **clasificación y la decisión**,
que es la parte que se rompió. Medido hoy desde fuera (sin SSH): **1 cabecera CSP
por host** (`arbx.ape-tv.net` → `content-security-policy-report-only`;
`edge-arbx.ape-tv.net` → `content-security-policy`), que es el estado que el sync
debe preservar.

## Revert

Un paso: revertir el commit restaura el predicado `NONCSP` original.
