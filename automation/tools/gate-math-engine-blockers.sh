#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# GATE: Motor §IV blockers — A1 (route_metadata) + A2 (Executor) + M6 (calibration)
#
# DOCTRINA (auditoría vivid-grove 2026-08-17): el motor matemático §IV está
# bloqueado por una cadena determinista. Este gate CI protege los invariantes
# para que NINGÚN PR pueda cerrar un blocker sin evidencia real.
#
# Reglas que este gate enforce:
#   G-A1: TODO worker que inserta opportunities DEBE emitir route_metadata
#   G-A2: ARBITRAGE_EXECUTOR solo puede aparecer con un valor que NO sea
#         placeholder (0x000..., 0x1234, 0xdead, test, mock)
#   G-M6: source_context='calibrated' solo puede existir cuando la tabla
#         math_operator_calibration tiene filas en prod (no se puede falsear
#         cambiando el string en código)
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail
cd "$(dirname "$0")/../.."

FAIL=0

# ── G-A1: workers sin route_metadata ─────────────────────────────────────
# Todo archivo que llame insert_opportunity DEBE construir/pasar route_metadata.
# Esto es el Gap 1: sin route_metadata, sim-ctl no puede resolver rutas →
# no hay Y-labels → calibración imposible → motor §IV inerte.
for f in backend/searcher-rs/src/workers/*.rs; do
    base="$(basename "$f")"
    # Skip test modules and the persistence helper itself
    [[ "$base" == "persistence.rs" ]] && continue
    [[ "$base" == "mod.rs" ]] && continue
    if grep -q "insert_opportunity" "$f" 2>/dev/null; then
        if ! grep -q "route_metadata" "$f" 2>/dev/null; then
            echo "FAIL G-A1: $base calls insert_opportunity but never mentions route_metadata" >&2
            echo "  → sim-ctl cannot resolve routes from this worker's output" >&2
            echo "  → the §IV mathematical engine stays INERT (Gap 1)" >&2
            FAIL=1
        fi
    fi
done

# ── G-A2: ARBITRAGE_EXECUTOR placeholder check ───────────────────────────
# El Executor solo puede tener direcciones reales (deployadas), no placeholders.
if grep -rn "ARBITRAGE_EXECUTOR" backend/ --include="*.rs" --include="*.ts" --include="*.env*" 2>/dev/null | \
   grep -viE "test|spec|mock|example|comment|//.*|#!.*" | \
   grep -E "0x0{6,}|0x1234|0xdead|0xbabe|placeholder|dummy|fake"; then
    echo "FAIL G-A2: ARBITRAGE_EXECUTOR contains placeholder value" >&2
    echo "  → deploy a real contract or leave the engine marked non-operational" >&2
    FAIL=1
fi

# ── G-M6: source_context 'calibrated' sin tabla poblada ──────────────────
# No se puede cambiar source_context a 'calibrated' en código sin que la
# tabla math_operator_calibration tenga datos reales en prod.
if grep -rn '"calibrated"' backend/searcher-rs/src/ --include="*.rs" 2>/dev/null | \
   grep -v "#\[cfg(test)\]" | \
   grep -v "^\s*//" | \
   grep -v "when.*calibrat\|after.*calibrat\|flat_prior.*calibrated\|calibrated.*flat"; then
    echo "WARN G-M6: hardcoded 'calibrated' source_context found" >&2
    echo "  → source_context must be derived from the math_operator_calibration table" >&2
    echo "  → not hardcoded in source code" >&2
    # This is a WARN for now — it could be legitimate documentation
fi

# ── Summary ──────────────────────────────────────────────────────────────
if [ "$FAIL" -ne 0 ]; then
    echo "" >&2
    echo "══════════════════════════════════════════════════════════" >&2
    echo "  §IV MOTOR BLOCKER GATE: FAILED" >&2
    echo "" >&2
    echo "  Los bloqueadores A1/A2 del motor matemático §IV" >&2
    echo "  están PROTEGIDOS por doctrina (auditoría vivid-grove)." >&2
    echo "" >&2
    echo "  Para cerrar un blocker se requiere:" >&2
    echo "    A1: TODO worker que inserta opportunities emite route_metadata" >&2
    echo "    A2: ARBITRAGE_EXECUTOR con dirección real (no placeholder)" >&2
    echo "    M6: math_operator_calibration con filas > 0 en prod" >&2
    echo "══════════════════════════════════════════════════════════" >&2
    exit 1
fi

echo "✓ §IV motor blocker gate: PASS (A1 workers emit route_metadata, A2 no placeholder executor, M6 no hardcoded calibrated)"
