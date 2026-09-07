#!/bin/bash
# WO-15 (2026-09-06) — READ-ONLY Redis consumer-group census (XLEN/XINFO/XPENDING only, 0 writes).
R="docker exec arbitragex-v2-redis-1 redis-cli --raw"
printf "%-26s %-20s %9s %9s %14s %s\n" "STREAM" "GROUP" "CONSUMERS" "PEND>0" "MAX-IDLE(ms)" "MAX-IDLE(h)"
total=0
while read -r stream group; do
  out=$($R XINFO CONSUMERS "$stream" "$group" 2>/dev/null | awk '
    /^name$/     { n++ }
    /^pending$/  { getline v; if (v+0 > 0) p++ }
    /^idle$/     { getline v; if (v+0 > m) m = v+0 }
    END { printf "%d %d %d", n+0, p+0, m+0 }')
  set -- $out
  printf "%-26s %-20s %9s %9s %14s %.1f\n" "$stream" "$group" "$1" "$2" "$3" "$(echo "$3/3600000" | bc -l)"
  total=$((total + $1))
done <<'EOF'
arbx:opps:detected enricher
arbx:opps:detected paper-archiver-g0
arbx:opps:detected selector-g0
arbx:opps:validated sim-ctl-g0
arbx:opps:simulated relays-client-g0
arbx:hot:detected ws-emitter-g0
arbx:hot:simulated ws-emitter-g0
EOF
echo "TOTAL_CONSUMERS_ALL_GROUPS=$total"
echo "--- XPENDING summary (grupo con pending>0) ---"
$R XPENDING arbx:opps:detected selector-g0
