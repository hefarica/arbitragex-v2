#!/usr/bin/env bash
# Emit the REQUESTED-credentials manifest as TSV:  KEY <TAB> VALUE <TAB> NOTES
# Authority = .env.example (what the app requests). Reconciled with the live .env:
#   ok                       -> set with a real value (VALUE = actual value)
#   FALTA-LLENAR             -> present in .env but EMPTY (VALUE blank; operator must fill)
#   REEMPLAZAR (placeholder) -> present but still a placeholder
#   REQUERIDA (default: X)   -> in .env.example but NOT in .env (code default applies; X = hint)
# VALUE is only emitted for "ok" rows; gaps have a blank VALUE so the push never sends a
# placeholder/empty (ExportCore skips blank values). Run on the VPS; output captured by the pull.
set -euo pipefail
cd /opt/arbitragex-v2
ex=.env.example
en=.env
[ -f "$ex" ] || { echo "ERROR: no .env.example" >&2; exit 2; }
[ -f "$en" ] || { echo "ERROR: no .env" >&2; exit 2; }

# union of keys: .env.example order first, then any .env-only keys, de-duplicated
keys=$( { grep -oE '^[A-Za-z_][A-Za-z0-9_]*=' "$ex"; grep -oE '^[A-Za-z_][A-Za-z0-9_]*=' "$en"; } | tr -d '=' | awk '!seen[$0]++' )

while IFS= read -r k; do
  [ -z "$k" ] && continue
  if grep -qm1 "^${k}=" "$en"; then
    v=$(grep -m1 "^${k}=" "$en" | cut -d= -f2-)
    if [ -z "$v" ]; then
      printf '%s\t\tFALTA-LLENAR\n' "$k"
    elif printf '%s' "$v" | grep -qiE '<[^>]+>|REPLACE_ME|CHANGE_ME|placeholder|your_key|xxxx'; then
      printf '%s\t\tREEMPLAZAR (placeholder)\n' "$k"
    else
      printf '%s\t%s\tok\n' "$k" "$v"
    fi
  else
    xv=$(grep -m1 "^${k}=" "$ex" | cut -d= -f2-)
    printf '%s\t\tREQUERIDA (default: %s)\n' "$k" "$xv"
  fi
done <<< "$keys"
