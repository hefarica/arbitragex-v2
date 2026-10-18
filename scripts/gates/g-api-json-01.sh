#!/usr/bin/env bash
# G-API-JSON-01 — `/api/*` es una superficie de API: NUNCA negocia contenido.
#
# DEFECTO QUE PINA (medido 2026-10-18, ambos hosts, antes del arreglo):
#   UA de navegador + `Accept: text/html`  -> 404 {"detail":"html_served_by_spa"}
#   UA de CLI (curl)  o Accept: application/json -> 200 JSON con 7/7 upstreams
# El StatusClient del navegador sondea `/api/status` atravesando el rewrite
# `/api/*` de Next, así que envía UA de Chrome y `Accept: text/html`: recibía un
# 404 mientras una sonda de CLI sobre la MISMA URL recibía 200. La negociación se
# escribió para la ruta de PÁGINA `/status` (que colisiona con la página /status
# de Next) y se reutilizó para el alias de API.
#
# QUÉ COMPRUEBA
#   (1) `/api/status` con UA de navegador + `Accept: text/html` -> **200** y JSON,
#       en LOS DOS hosts;
#   (2) `/api/status` con UA de CLI -> 200 JSON (no se rompe lo que ya funcionaba);
#   (3) una ruta NO-API (la página `/status`) sigue sirviendo la SPA para un
#       navegador (200 + text/html) — el arreglo no debe tocar la negociación.
#
# Uso: g-api-json-01.sh [frontend_url] [edge_url]
# Exit: 0 PASS, 1 FAIL, 2 SKIP (sin curl).
set -u
FRONTEND="${1:-https://arbx.ape-tv.net}"
EDGE="${2:-https://edge-arbx.ape-tv.net}"
BROWSER_UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
FAIL=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; FAIL=1; fi; }

say "G-API-JSON-01 — /api/* no negocia contenido"
command -v curl >/dev/null 2>&1 || { say "  SKIP  curl ausente"; exit 2; }

probe() { # probe <url> <ua> <accept> -> "status|content_type|body_head"
  curl -s -o /tmp/g-api-json.out -w "%{http_code}|%{content_type}" -m 25 \
    -A "$2" -H "Accept: $3" "$1" 2>/dev/null
  printf '|%s' "$(head -c 80 /tmp/g-api-json.out | tr -d '\n')"
}

for H in "$FRONTEND" "$EDGE"; do
  say ""
  say "host: $H"

  # (1) el caso que estaba roto
  R=$(probe "$H/api/status" "$BROWSER_UA" "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
  ST=${R%%|*}; REST=${R#*|}; CT=${REST%%|*}; BODY=${REST#*|}
  say "  info  navegador -> $ST | $CT | ${BODY:0:60}"
  [ "$ST" = "200" ]; chk "(1) /api/status con UA de navegador responde 200" $?
  case "$CT" in *json*) chk "(1) /api/status responde JSON al navegador" 0 ;; *) chk "(1) /api/status responde JSON al navegador" 1 ;; esac
  printf '%s' "$BODY" | grep -q "html_served_by_spa"; [ $? -ne 0 ]
  chk "(1) /api/status NO devuelve el 404 html_served_by_spa" $?
  printf '%s' "$BODY" | grep -q '"ok"'; chk "(1) el cuerpo es el JSON del backend (contiene ok)" $?

  # (2) lo que ya funcionaba no se rompe
  R=$(probe "$H/api/status" "curl/8.4.0" "*/*")
  ST=${R%%|*}
  [ "$ST" = "200" ]; chk "(2) /api/status con UA de CLI sigue dando 200" $?
done

# (3) la negociación del resto no se toca
say ""
say "host: $FRONTEND"
R=$(probe "$FRONTEND/status" "$BROWSER_UA" "text/html,application/xhtml+xml")
ST=${R%%|*}; REST=${R#*|}; CT=${REST%%|*}
say "  info  /status (página) navegador -> $ST | $CT"
[ "$ST" = "200" ]; chk "(3) la página /status sigue sirviendo la SPA (200)" $?
case "$CT" in *html*) chk "(3) la página /status sigue siendo text/html" 0 ;; *) chk "(3) la página /status sigue siendo text/html" 1 ;; esac

say ""
if [ "$FAIL" = "0" ]; then say "G-API-JSON-01: PASS"; else say "G-API-JSON-01: FAIL"; fi
exit "$FAIL"
