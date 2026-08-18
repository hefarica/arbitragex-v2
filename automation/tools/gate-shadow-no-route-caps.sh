#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# GATE: SHADOW-NO-ROUTE-CAPS (operator directive 2026-08-18)
#
# "Nunca, bajo ninguna condición, me capee rutas en modo shadow."
#
# Live evidence at directive time: routes_capped=true, routes_found=500 (=
# exactly the cap), routes_dropped_for_cap=13, mode=shadow — and worse, the
# loop restarted each tick, so the SAME 500 routes were rediscovered forever
# while the rest of the topology was never explored.
#
# This gate pins the DeferNeverDrop invariant so no PR can reintroduce route
# LOSS in shadow:
#   G-1  The shadow worker builds its finder with CapPolicy::DeferNeverDrop.
#   G-2  The depth floor clamp (MIN_SHADOW_DEPTH) exists and is applied —
#        shadow cannot be configured shallower than the sim layer's max.
#   G-3  The H1 exhaustiveness oracle test exists (incremental union == the
#        uncapped legacy set) — the mathematical proof the engine defers
#        instead of dropping.
#   G-4  No Rust source reintroduces the lossy `routes_capped` telemetry
#        semantics (the field is renamed routes_deferred; historical names
#        appear only in docs/plans).
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail
cd "$(dirname "$0")/../.."

FAIL=0
WORKER="backend/searcher-rs/src/route_discovery/route_discovery_worker.rs"
FINDER="backend/searcher-rs/src/route_discovery/unique_route_finder.rs"

# ── G-1: shadow worker must construct DeferNeverDrop ──────────────────────
if ! grep -q "policy: CapPolicy::DeferNeverDrop" "$WORKER"; then
    echo "FAIL G-1: the route-discovery worker no longer pins CapPolicy::DeferNeverDrop." >&2
    echo "  Shadow must NEVER drop routes to a cap (SHADOW-NO-ROUTE-CAPS)." >&2
    FAIL=1
fi

# ── G-2: the depth floor clamp must exist ──────────────────────────────────
if ! grep -q "MIN_SHADOW_DEPTH" "$WORKER"; then
    echo "FAIL G-2: MIN_SHADOW_DEPTH floor clamp missing from the worker." >&2
    echo "  Shadow discovery depth cannot be configured below the sim layer's max." >&2
    FAIL=1
fi
# Any ARBX_ROUTE_DISCOVERY_MAX_DEPTH set in tracked env/compose files must be >= 7.
for f in .env.example docker/compose.prod.yml docker/compose.dev.yml; do
    [ -f "$f" ] || continue
    while IFS='=' read -r k v; do
        [ "$k" = "ARBX_ROUTE_DISCOVERY_MAX_DEPTH" ] || continue
        if [ "${v:-0}" -lt 7 ]; then
            echo "FAIL G-2: $f sets ARBX_ROUTE_DISCOVERY_MAX_DEPTH=$v (< 7) — shadow floor violated." >&2
            FAIL=1
        fi
    done < <(grep -E "^ARBX_ROUTE_DISCOVERY_MAX_DEPTH=" "$f" || true)
done

# ── G-3: the H1 exhaustiveness oracle test must exist ─────────────────────
if ! grep -q "fn h1_exhaustive_union_equals_uncapped_legacy" "$FINDER"; then
    echo "FAIL G-3: the H1 exhaustiveness oracle test was removed." >&2
    echo "  It is the mathematical proof that DeferNeverDrop loses no route." >&2
    FAIL=1
fi

# ── G-4: the lossy semantics must not come back in Rust sources ───────────
if grep -rn "routes_capped\|routes_dropped_for_cap" backend/ --include="*.rs"; then
    echo "FAIL G-4: lossy route-cap telemetry reintroduced in Rust sources." >&2
    echo "  Shadow defers (routes_deferred + deferred_cursor), never caps." >&2
    FAIL=1
fi

# ── Summary ───────────────────────────────────────────────────────────────
if [ "$FAIL" -ne 0 ]; then
    echo "" >&2
    echo "══════════════════════════════════════════════════════════" >&2
    echo "  SHADOW-NO-ROUTE-CAPS GATE: FAILED" >&2
    echo "" >&2
    echo "  Shadow enumeration must be exhaustive-by-deferral:" >&2
    echo "    budget pauses the traversal, the cursor resumes next" >&2
    echo "    tick, parallel pools rotate across ladders. No route" >&2
    echo "    is EVER lost to a cap in shadow mode." >&2
    echo "══════════════════════════════════════════════════════════" >&2
    exit 1
fi

echo "✓ SHADOW-NO-ROUTE-CAPS gate: PASS (DeferNeverDrop pinned, depth floor clamped, H1 oracle present, no lossy telemetry)"
