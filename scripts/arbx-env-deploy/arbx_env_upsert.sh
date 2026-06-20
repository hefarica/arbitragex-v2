#!/usr/bin/env bash
# Idempotent, structure-preserving .env upsert.
#   Usage: arbx_env_upsert.sh <target.env> <fragment.env> [--backup]
# - Replaces managed keys IN PLACE (order/position preserved).
# - Appends keys not yet present (fragment order), at the end.
# - Leaves all other lines (comments, blanks, other keys) byte-identical.
# - Values are LITERAL strings (no regex/sed escaping); safe for '=', '/', '&', ':'
#   (e.g. RPC multi-provider). Commented keys (#KEY=...) are NOT clobbered.
# - CRLF-SAFE: strips a trailing CR from every input line, so an Excel/Windows
#   fragment (VBA Print # writes CRLF) never injects \r into the Linux .env.
# - Idempotent: re-running with the same fragment yields a byte-identical file.
set -euo pipefail

target="${1:?target .env required}"
frag="${2:?fragment .env required}"
backup="${3:-}"
[ -f "$target" ] || { echo "target not found: $target" >&2; exit 2; }
[ -f "$frag" ]   || { echo "fragment not found: $frag" >&2; exit 2; }

[ "$backup" = "--backup" ] && cp -p "$target" "${target}.bak_$(date +%Y%m%d_%H%M%S)"

declare -A KV; order=()
while IFS= read -r line || [ -n "$line" ]; do
  line="${line%$'\r'}"                                 # CRLF-safe
  [ -z "${line//[[:space:]]/}" ] && continue           # blank
  [ "${line:0:1}" = "#" ] && continue                  # comment in fragment
  [ "$line" = "${line#*=}" ] && continue               # no '=' -> skip
  k="${line%%=*}"; v="${line#*=}"
  [ -z "$k" ] && continue
  if [ -z "${KV[$k]+x}" ]; then order+=("$k"); fi
  KV["$k"]="$v"
done < "$frag"

tmp="$(mktemp)"; declare -A seen
while IFS= read -r line || [ -n "$line" ]; do
  line="${line%$'\r'}"                                 # CRLF-safe (no-op on LF target)
  k="${line%%=*}"
  if [ "${line:0:1}" != "#" ] && [ "$line" != "${line#*=}" ] && [ -n "${KV[$k]+x}" ]; then
    printf '%s=%s\n' "$k" "${KV[$k]}"
    seen["$k"]=1
  else
    printf '%s\n' "$line"
  fi
done < "$target" > "$tmp"

for k in "${order[@]}"; do
  if [ -z "${seen[$k]+x}" ]; then printf '%s=%s\n' "$k" "${KV[$k]}" >> "$tmp"; seen["$k"]=1; fi
done

cat "$tmp" > "$target"   # in-place (preserves inode/perms — keeps chmod 600)
rm -f "$tmp"
echo "upsert ok: $(wc -l < "$target") lines"
