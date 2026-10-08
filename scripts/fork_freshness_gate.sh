#!/bin/sh
# FORK-FRESH-01 — gate de EDAD del fork, con veredicto observable y fallo RUIDOSO.
#
# POR QUE EXISTE (medido, t184 + t185): `anvil --fork-url` SIN `--fork-block-number` deja el
# fork CLAVADO en el bloque que era cabeza al arrancar el contenedor y no vuelve a leer. En el
# runtime medido el 2026-10-08 eso dio fork_block=26148216 contra una cabeza de 26149916:
# 1700 bloques = 340 min = 5,67 h de estado viejo. Cualquier `size*` medido contra ese fork
# es arqueologia, no mercado.
#
# QUE HACE: lee el bloque del fork y el de la cadena, calcula la edad y la compara con un
# umbral EN MINUTOS. No escribe nada, no toca el fork, no reinicia nada: solo lee por JSON-RPC.
#
# CONTROL DE VACUIDAD (el objeto de esta tarea): con un fork stale el gate FALLA. Un gate de
# edad que no falla con el fork viejo no es un gate, es un adorno. Ver docs/sre/FORK-FRESH-01.md.
#
# USO:
#   scripts/fork_freshness_gate.sh --fork-rpc <URL> --head-rpc <URL> [--max-age-minutes N]
#                                 [--seconds-per-block N] [--timeout-seconds N]
#
#   --fork-rpc           (o ARBX_FORK_RPC)  endpoint del fork. REQUERIDO, sin default.
#   --head-rpc           (o ARBX_HEAD_RPC)  endpoint de la cadena real. REQUERIDO, sin default.
#                        Deben ser endpoints DISTINTOS: comparar el fork consigo mismo siempre
#                        diria FRESH, que es exactamente el adorno que este gate evita.
#   --max-age-minutes    (o ARBX_FORK_MAX_AGE_MINUTES)  umbral en MINUTOS. Default: 10.
#   --seconds-per-block  segundos por bloque de la cadena (default 12; post-merge mainnet).
#   --timeout-seconds    timeout por llamada RPC (default 8).
#
# SALIDA (una linea greppable, con las URLs REDACTADAS):
#   FORK_FRESHNESS verdict=FRESH|STALE|UNMEASURABLE fork_block=<n> head_block=<n> age_blocks=<n>
#                   age_minutes=<x.xx> threshold_minutes=<t> seconds_per_block=<s> reason=<...>
#
# EXIT CODES (deliberadamente distintos: "no pude medir" NO es "esta viejo", y ninguno es "fresco"):
#   0 = FRESH         la edad esta dentro del umbral.
#   1 = STALE         el fork esta mas viejo que el umbral.  => el llamador DEBE abortar.
#   2 = UNMEASURABLE  no se pudo medir (RPC vacio/ilegible/timeout) o la config es invalida
#                     (mismo endpoint, umbral no numerico, head < fork).
#   Un fallo de medida NO se degrada a FRESH: el sistema no puede simular con estado viejo
#   en silencio porque no sabe que el estado es viejo; ese caso es UNMEASURABLE, no verde.
set -u

MAX_AGE_MINUTES_DEFAULT=10   # <-- PUNTO EXACTO DONDE EL GATE LEE EL UMBRAL POR DEFECTO (10 min)
SECONDS_PER_BLOCK_DEFAULT=12
TIMEOUT_SECONDS_DEFAULT=8

FORK_RPC="${ARBX_FORK_RPC:-}"
HEAD_RPC="${ARBX_HEAD_RPC:-}"
MAX_AGE_MINUTES="${ARBX_FORK_MAX_AGE_MINUTES:-$MAX_AGE_MINUTES_DEFAULT}"
SECONDS_PER_BLOCK="$SECONDS_PER_BLOCK_DEFAULT"
TIMEOUT_SECONDS="$TIMEOUT_SECONDS_DEFAULT"

die_usage() {
  echo "usage: $0 --fork-rpc <URL> --head-rpc <URL> [--max-age-minutes N] [--seconds-per-block N] [--timeout-seconds N]" >&2
  exit 2
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --fork-rpc)          [ "$#" -ge 2 ] || die_usage; FORK_RPC="$2"; shift 2 ;;
    --head-rpc)          [ "$#" -ge 2 ] || die_usage; HEAD_RPC="$2"; shift 2 ;;
    --max-age-minutes)   [ "$#" -ge 2 ] || die_usage; MAX_AGE_MINUTES="$2"; shift 2 ;;
    --seconds-per-block) [ "$#" -ge 2 ] || die_usage; SECONDS_PER_BLOCK="$2"; shift 2 ;;
    --timeout-seconds)   [ "$#" -ge 2 ] || die_usage; TIMEOUT_SECONDS="$2"; shift 2 ;;
    -h|--help)           die_usage ;;
    *)                   echo "unknown argument: $1" >&2; die_usage ;;
  esac
done

[ -n "$FORK_RPC" ] || { echo "FORK-FRESH-01: --fork-rpc (o ARBX_FORK_RPC) es obligatorio" >&2; die_usage; }
[ -n "$HEAD_RPC" ] || { echo "FORK-FRESH-01: --head-rpc (o ARBX_HEAD_RPC) es obligatorio" >&2; die_usage; }

redact() { # nunca imprime credenciales embebidas en la URL
  printf '%s' "$1" | sed -E 's#^(https?://[^/]+).*#\1/<REDACTADO>#'
}

normalize_endpoint() { # host:port sin esquema ni path, para detectar el mismo endpoint
  printf '%s' "$1" | sed -E 's#^https?://##; s#/.*$##; s#/+$##'
}

report() { # verdict reason fork head age_blocks age_minutes
  echo "FORK_FRESHNESS verdict=$1 reason=$2 fork_block=$3 head_block=$4 age_blocks=$5 age_minutes=$6 threshold_minutes=$MAX_AGE_MINUTES seconds_per_block=$SECONDS_PER_BLOCK fork_rpc=$(redact "$FORK_RPC") head_rpc=$(redact "$HEAD_RPC")"
}

is_number() { case "$1" in ''|*[!0-9]*) return 1 ;; *) return 0 ;; esac; }

is_number "$MAX_AGE_MINUTES"   || { echo "FORK-FRESH-01: --max-age-minutes no es un entero: '$MAX_AGE_MINUTES'" >&2; exit 2; }
is_number "$SECONDS_PER_BLOCK" || { echo "FORK-FRESH-01: --seconds-per-block no es un entero: '$SECONDS_PER_BLOCK'" >&2; exit 2; }

# GUARDA CONTRA EL ADORNO: si el fork y la cabeza son el mismo endpoint, la edad siempre daria 0.
if [ "$(normalize_endpoint "$FORK_RPC")" = "$(normalize_endpoint "$HEAD_RPC")" ]; then
  echo "FORK-FRESH-01: fork-rpc y head-rpc apuntan al MISMO endpoint ($(normalize_endpoint "$FORK_RPC")) — comparar el fork consigo mismo daria FRESH siempre; se rechaza (exit 2)." >&2
  report UNMEASURABLE same_endpoint - - - -
  exit 2
fi

rpc_block_number() { # $1 = url -> imprime 0x... o vacio
  curl -s -m "$TIMEOUT_SECONDS" -X POST -H 'Content-Type: application/json' \
    --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' "$1" 2>/dev/null \
    | sed -n 's/.*"result"[[:space:]]*:[[:space:]]*"\(0x[0-9a-fA-F][0-9a-fA-F]*\)".*/\1/p'
}

hex2dec() { # POSIX puro: no depende de gawk strtonum ni de python
  h=${1#0x}; d=0
  while [ -n "$h" ]; do
    c=$(printf '%s' "$h" | cut -c1); h=$(printf '%s' "$h" | cut -c2-)
    case "$c" in
      [0-9]) v=$c ;;
      a|A) v=10 ;; b|B) v=11 ;; c|C) v=12 ;; d|D) v=13 ;; e|E) v=14 ;; f|F) v=15 ;;
      *) return 1 ;;
    esac
    d=$(( d * 16 + v ))
  done
  printf '%s' "$d"
}

FORK_HEX=$(rpc_block_number "$FORK_RPC")
if [ -z "$FORK_HEX" ]; then
  echo "FORK-FRESH-01: el fork no respondio eth_blockNumber (endpoint $(redact "$FORK_RPC"), timeout ${TIMEOUT_SECONDS}s) — NO SE PUEDE AFIRMAR FRESCURA." >&2
  report UNMEASURABLE fork_rpc_no_answer - - - -
  exit 2
fi
HEAD_HEX=$(rpc_block_number "$HEAD_RPC")
if [ -z "$HEAD_HEX" ]; then
  echo "FORK-FRESH-01: la cabeza no respondio eth_blockNumber (endpoint $(redact "$HEAD_RPC"), timeout ${TIMEOUT_SECONDS}s) — NO SE PUEDE AFIRMAR FRESCURA." >&2
  report UNMEASURABLE head_rpc_no_answer - - - -
  exit 2
fi

FORK_BLOCK=$(hex2dec "$FORK_HEX") || { report UNMEASURABLE fork_block_unparsable - - - -; exit 2; }
HEAD_BLOCK=$(hex2dec "$HEAD_HEX") || { report UNMEASURABLE head_block_unparsable - - - -; exit 2; }

if [ "$HEAD_BLOCK" -lt "$FORK_BLOCK" ]; then
  echo "FORK-FRESH-01: head_block ($HEAD_BLOCK) < fork_block ($FORK_BLOCK) — endpoints cruzados o fork adelantado al head; no es medible como edad." >&2
  report UNMEASURABLE head_below_fork "$FORK_BLOCK" "$HEAD_BLOCK" - -
  exit 2
fi

AGE_BLOCKS=$(( HEAD_BLOCK - FORK_BLOCK ))
AGE_MINUTES=$(awk -v ab="$AGE_BLOCKS" -v spb="$SECONDS_PER_BLOCK" 'BEGIN { printf "%.2f", ab * spb / 60 }')
AGE_MINUTES_INT=$(awk -v m="$AGE_MINUTES" 'BEGIN { printf "%d", m }')

if [ "$AGE_MINUTES_INT" -le "$MAX_AGE_MINUTES" ]; then
  echo "FORK-FRESH-01 OK: el fork esta a ${AGE_BLOCKS} bloques (${AGE_MINUTES} min) de la cabeza, dentro del umbral de ${MAX_AGE_MINUTES} min."
  report FRESH within_threshold "$FORK_BLOCK" "$HEAD_BLOCK" "$AGE_BLOCKS" "$AGE_MINUTES"
  exit 0
fi

echo "FORK-FRESH-01 STALE: el fork esta a ${AGE_BLOCKS} bloques (${AGE_MINUTES} min = $(awk -v m="$AGE_MINUTES" 'BEGIN { printf "%.2f", m/60 }') h) de la cabeza, POR ENCIMA del umbral de ${MAX_AGE_MINUTES} min." >&2
echo "FORK-FRESH-01 STALE: fork_block=$FORK_BLOCK head_block=$HEAD_BLOCK. Simular contra este estado seria medir el pasado, no el libro de ordenes actual: el llamador NO debe continuar." >&2
report STALE over_threshold "$FORK_BLOCK" "$HEAD_BLOCK" "$AGE_BLOCKS" "$AGE_MINUTES"
exit 1
