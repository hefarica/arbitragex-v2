#!/usr/bin/env bash
# G-ADMIN-ROOT-01 — /admin root resolution gate.
#
# DECISION (ADR-002): /admin (bare) stays 404. No consumer exists.
# Rationale: frontend/components/nav-items.ts references only /admin/topology,
# /admin/chains and /admin/signin. Adding an app/admin/page.tsx would create a
# new navigable route with no consumer — forbidden by the "no new wire field
# without a real consumer" invariant. The gate therefore asserts the decision
# is still consistent, so a future change cannot silently contradict it.
#
# Exit 0 = PASS (decision holds), exit 1 = FAIL (decision contradicted).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
fail=0
chk() { if [ "$2" = "0" ]; then echo "  PASS  $1"; else echo "  FAIL  $1"; fail=1; fi; }

echo "G-ADMIN-ROOT-01 — /admin root resolution gate"

# 1. no bare /admin route file exists (decision = 404)
[ ! -f frontend/app/admin/page.tsx ]; chk "frontend/app/admin/page.tsx does NOT exist (bare /admin = 404)" $?

# 2. the admin sub-routes that DO exist
SUBS=$(find frontend/app/admin -maxdepth 2 -name page.tsx | sort | tr '\n' ' ')
echo "  info  existing admin routes: $SUBS"

# 3. no nav item / href targets bare /admin (there is no consumer to satisfy)
if grep -rqE 'href[=:][[:space:]]*"/admin"' frontend/components frontend/features frontend/app --include='*.ts' --include='*.tsx' 2>/dev/null; then
  chk "no href/route literal points at bare /admin" 1
else
  chk "no href/route literal points at bare /admin" 0
fi

# 4. every existing /admin/* route is reachable from nav-items.ts
for f in $(find frontend/app/admin -maxdepth 2 -name page.tsx); do
  route="/${f#frontend/app/}"; route="${route%/page.tsx}"
  grep -q "\"$route\"" frontend/components/nav-items.ts
  chk "route $route is present in nav-items.ts" $?
done

echo ""
if [ "$fail" = "0" ]; then echo "G-ADMIN-ROOT-01: PASS"; else echo "G-ADMIN-ROOT-01: FAIL"; fi
exit "$fail"
