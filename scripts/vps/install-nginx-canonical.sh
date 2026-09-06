#!/usr/bin/env bash
# NGINX-CONF-DRIFT-01 — instala el config canónico versionado:
#   /etc/nginx/sites-available/arbitragex = config/nginx/arbitragex.conf
#   /etc/nginx/sites-enabled/arbitragex   = symlink → ../sites-available/arbitragex
# Con backup fechado, validación `nginx -t`, ROLLBACK automático si la
# validación falla, reload + smoke (:80 escuchando, / responde).
#
# Uso (VPS, desde /opt/arbitragex-v2):  bash scripts/vps/install-nginx-canonical.sh
set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/../.." && pwd)
CANON="$REPO_ROOT/config/nginx/arbitragex.conf"
AVAIL=/etc/nginx/sites-available/arbitragex
ENABLED=/etc/nginx/sites-enabled/arbitragex

[ -f "$CANON" ] || { echo "FATAL: canónico ausente: $CANON"; exit 2; }

TS=$(date -u +%Y%m%dT%H%M%SZ)
BAK=/etc/nginx/arbitragex.pre-swap-$TS

# Backup del contenido vivo (cp dereference: sirve para archivo regular o symlink)
if [ -e "$ENABLED" ] || [ -L "$ENABLED" ]; then
  cp "$ENABLED" "$BAK"
  echo "backup: $BAK ($(wc -c <"$BAK") bytes)"
else
  BAK=""
  echo "backup: ninguno (enabled no existía)"
fi

install -m 0644 "$CANON" "$AVAIL"
rm -f "$ENABLED"
ln -s /etc/nginx/sites-available/arbitragex "$ENABLED"
echo "instalado: $ENABLED → symlink a sites-available (canónico repo)"

if ! nginx -t 2>&1; then
  echo "ROLLBACK: nginx -t falló → restaurando $BAK"
  rm -f "$ENABLED" "$AVAIL"
  if [ -n "$BAK" ]; then
    cp "$BAK" "$ENABLED"
    nginx -t && systemctl reload nginx
  fi
  exit 1
fi

systemctl reload nginx
sleep 1

# Smoke: :80 escuchando y / responde (anti "reload silencioso", incidente 08-23)
if ! ss -tln 2>/dev/null | grep -q ':80 '; then
  echo "FATAL: :80 no escucha tras reload — restaurando backup y restart"
  rm -f "$ENABLED"
  [ -n "$BAK" ] && cp "$BAK" "$ENABLED"
  systemctl restart nginx
  ss -tln | grep ':80 ' || true
  exit 1
fi
CODE=$(curl -s -o /dev/null -w '%{http_code}' -m 10 http://127.0.0.1/ || echo 000)
echo "smoke: GET / → $CODE (200/30x esperado vía frontend)"
case "$CODE" in
  2*|3*) ;;
  *) echo "ADVERTENCIA: código inesperado $CODE — revisar curl -I manualmente" ;;
esac

echo "OK. Verificación completa: bash scripts/vps/verify-deploy.sh (L0 debe decir live == repo)."
