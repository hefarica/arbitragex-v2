#!/usr/bin/env bash
# G-NGINX-R1-01 — la regla R1 del sync canónico de nginx decide por CLASE de delta.
#
# QUÉ PINA
#   R1 abortaba el install del canónico ante CUALQUIER línea cambiada que no fuera
#   un `add_header ... Content-Security-Policy`, **incluidas las de COMENTARIO**.
#   El canónico trae comentarios que el fichero vivo no tiene ⇒ R1 abortaba
#   SIEMPRE y el canónico no se instalaba nunca: sólo se parcheaba el drift en
#   sitio y el resto del canónico quedaba sin aplicar de forma indefinida.
#
#   Decisión (2026-10-18): aflojar R1 para que un delta de SÓLO comentarios no
#   aborte; el aborto se conserva para drift REAL.
#
# CÓMO SE PRUEBA (no tautológico)
#   Este gate NO reimplementa R1: **extrae el bloque R1 tal cual del workflow**
#   (`vps-nginx-canonical-sync.yml`), lo desindenta y lo EJECUTA contra parejas de
#   fixtures. Así lo que se prueba es el código que se va a desplegar, no una copia.
#   Además reproduce la predicado ANTERIOR (`NONCSP`) para dejar constancia de que
#   el defecto existía y de que el arreglo cambia el veredicto.
#
#   La instalación real, `nginx -t` y el conteo de CSP por host tras el reload
#   requieren VPS + credencial y NO se simulan aquí: este gate cubre la
#   clasificación y la decisión, que es la parte que se rompió.
#
# Uso: g-nginx-r1-01.sh
# Exit: 0 PASS, 1 FAIL, 2 SKIP (sin python3/diff).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WF="$ROOT/.github/workflows/vps-nginx-canonical-sync.yml"
WORK="$(mktemp -d /tmp/g-r1-XXXXXX)"
FAIL=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; FAIL=1; fi; }

say "G-NGINX-R1-01 — clasificación del delta de R1 (nginx canónico)"
command -v python3 >/dev/null 2>&1 || { say "  SKIP  python3 ausente"; exit 2; }
command -v diff >/dev/null 2>&1 || { say "  SKIP  diff ausente"; exit 2; }
[ -f "$WF" ]; chk "existe el workflow del sync" $?

# --- extraer el bloque R1 real del workflow --------------------------------
python3 - "$WF" "$WORK/r1.sh" <<'PY'
import sys, re
wf, out = sys.argv[1], sys.argv[2]
lines = open(wf, encoding="utf-8").read().split("\n")
start = next(i for i, l in enumerate(lines) if "--- R1 (revisado" in l)
# el bloque termina en el `fi` de 10 espacios que cierra el if/else exterior
end = None
for i in range(start + 1, len(lines)):
    if lines[i] == " " * 10 + "fi":
        end = i
        break
assert end is not None, "no se encontró el cierre de R1"
block = lines[start:end + 1]
body = "\n".join(l[10:] if l.startswith(" " * 10) else l.lstrip() for l in block)
body = body.replace("\\$", "$")           # el heredoc del workflow escapa $ para el shell local
open(out, "w", encoding="utf-8").write(body + "\n")
open(out + ".lines", "w").write(str(len(block)))
PY
R1LINES=$(cat "$WORK/r1.sh.lines" 2>/dev/null || echo "?")
say "  info  bloque R1 extraído del workflow: $R1LINES líneas -> $WORK/r1.sh"
chk "el bloque R1 extraído usa la clasificación nueva (R1_REAL)" \
    "$(grep -q 'R1_REAL' "$WORK/r1.sh" && echo 0 || echo 1)"

# --- fixtures --------------------------------------------------------------
cat > "$WORK/live.conf" <<'EOF'
server {
    listen 443 ssl;
    server_name arbx.ape-tv.net;
    location / {
        proxy_pass http://127.0.0.1:8080;
        add_header Content-Security-Policy "default-src 'self'" always;
    }
}
EOF
# (a) delta SÓLO de comentarios  ← el caso que abortaba
{ cat "$WORK/live.conf"; printf '# canon comment A\n# canon comment B\n'; } > "$WORK/canon_comment.conf"
# (b) delta SÓLO del CSP esperado
sed 's/default-src .self./default-src '"'"'self'"'"' https:\/\//' "$WORK/live.conf" > "$WORK/canon_csp.conf"
# (c) drift REAL (cambia el upstream, no es comentario ni CSP)
sed 's|http://127.0.0.1:8080|http://127.0.0.1:9090|' "$WORK/live.conf" > "$WORK/canon_real.conf"
# (d) mixto: CSP + comentarios (lo normal en la realidad)
{ sed 's/default-src .self./default-src '"'"'self'"'"' https:\/\//' "$WORK/live.conf"; printf '# nota del canónico\n'; } > "$WORK/canon_mixed.conf"

run_case() { # run_case <nombre> <canon> -> imprime salida; SKIP_CANON en la última línea
  ( cd "$WORK" && LIVE="$WORK/live.conf" CANON="$2" bash -c '
      set -u
      . "'"$WORK"'/r1.sh"
      echo "SKIP_CANON=$SKIP_CANON"
    ' 2>&1 )
}

say ""
say "-- (a) delta SÓLO de comentarios (el defecto) --"
OUT=$(run_case comment "$WORK/canon_comment.conf"); echo "$OUT" | sed 's/^/      /'
echo "$OUT" | grep -q "COMMENT : 2"; chk "(a) clasifica las 2 líneas como COMMENT" $?
echo "$OUT" | grep -qE "REAL    : 0"; chk "(a) REAL = 0" $?
echo "$OUT" | grep -q "SKIP_CANON=0"; chk "(a) NO aborta ⇒ el canónico se instala (era ABORT antes del arreglo)" $?

say ""
say "-- (b) delta SÓLO del CSP esperado --"
OUT=$(run_case csp "$WORK/canon_csp.conf"); echo "$OUT" | sed 's/^/      /'
echo "$OUT" | grep -q "SKIP_CANON=0"; chk "(b) no aborta (comportamiento previo preservado)" $?

say ""
say "-- (c) drift REAL (proxy_pass cambiado) --"
OUT=$(run_case real "$WORK/canon_real.conf"); echo "$OUT" | sed 's/^/      /'
echo "$OUT" | grep -qE "REAL    : 2"; chk "(c) clasifica el drift como REAL" $?
echo "$OUT" | grep -q "SKIP_CANON=1"; chk "(c) SÍ aborta (el aborto se conserva para drift real)" $?

say ""
say "-- (d) mixto CSP + comentario --"
OUT=$(run_case mixed "$WORK/canon_mixed.conf"); echo "$OUT" | sed 's/^/      /'
echo "$OUT" | grep -q "SKIP_CANON=0"; chk "(d) no aborta (CSP + comentario)" $?

# --- procedencia del defecto: la predicción ANTERIOR abortaba en (a) --------
say ""
say "-- predicado ANTERIOR (NONCSP), para constancia del defecto --"
OLD_ABORT=$(cd "$WORK" && DIFF=$(diff -u live.conf canon_comment.conf || true); \
  printf '%s\n' "$DIFF" | grep -E '^[+-]' | grep -vE '^(\+\+\+|---)' \
  | grep -vE 'add_header.*Content-Security-Policy' | grep -c . || true)
say "      líneas 'ajenas al CSP' según R1 viejo en el caso (a): $OLD_ABORT"
[ "$OLD_ABORT" -ge 1 ] 2>/dev/null
chk "el R1 viejo habría abortado en (a) — el defecto era real" $?

say ""
if [ "$FAIL" = "0" ]; then say "G-NGINX-R1-01: PASS"; else say "G-NGINX-R1-01: FAIL"; fi
say "nota: instalación real / nginx -t / CSP por host tras reload requieren VPS+credencial (no simulados)"
exit "$FAIL"
