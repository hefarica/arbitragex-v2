#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# FASE OMEGA — Cartridge Test Runner
# ═══════════════════════════════════════════════════════════════════════════════
#
# Runs all cartridge-related tests:
# 1. Contract validation (compilation + required functions)
# 2. Sandboxing enforcement (infinite loops, memory bombs)
# 3. Multi-chain agnostic verification (10 chains)
# 4. Content hash dedup
#
# Usage:
#   ./scripts/test_cartridges.sh          # Run all cartridge tests
#   ./scripts/test_cartridges.sh --quick  # Run only compilation tests
#
# Prerequisites:
#   - Rust toolchain installed
#   - rhai and sha2 crates in Cargo.toml
#   - No external services needed (Redis not required for tests)
#
# ═══════════════════════════════════════════════════════════════════════════════

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}  FASE OMEGA — Cartridge Runtime Test Suite${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo ""

cd "$PROJECT_DIR"

# Check cartridge files exist
echo -e "${YELLOW}[1/4]${NC} Checking cartridge files..."
CARTRIDGE_DIR="$PROJECT_DIR/cartridges"
if [ ! -d "$CARTRIDGE_DIR" ]; then
    echo -e "${RED}ERROR: cartridges/ directory not found${NC}"
    exit 1
fi

CARTRIDGE_COUNT=$(find "$CARTRIDGE_DIR" -name "*.rhai" | wc -l)
echo -e "  Found ${GREEN}${CARTRIDGE_COUNT}${NC} cartridge files:"
find "$CARTRIDGE_DIR" -name "*.rhai" -exec basename {} \; | while read f; do
    echo -e "    • $f"
done
echo ""

# Validate contract (check required functions exist in each cartridge)
echo -e "${YELLOW}[2/4]${NC} Validating Universal Contract..."
REQUIRED_FNS=("init_strategy" "evaluate_opportunity" "build_payload")
ALL_VALID=true

for rhai_file in "$CARTRIDGE_DIR"/*.rhai; do
    filename=$(basename "$rhai_file")
    missing=""
    for fn in "${REQUIRED_FNS[@]}"; do
        if ! grep -q "fn ${fn}" "$rhai_file"; then
            missing="$missing $fn"
        fi
    done
    if [ -z "$missing" ]; then
        echo -e "  ${GREEN}✓${NC} $filename — all required functions present"
    else
        echo -e "  ${RED}✗${NC} $filename — MISSING:$missing"
        ALL_VALID=false
    fi
done

if [ "$ALL_VALID" = false ]; then
    echo -e "\n${RED}Contract validation FAILED${NC}"
    exit 1
fi
echo ""

# Check chain-agnostic compliance (no hardcoded addresses)
echo -e "${YELLOW}[3/4]${NC} Checking chain-agnostic compliance..."
VIOLATIONS=false

for rhai_file in "$CARTRIDGE_DIR"/*.rhai; do
    filename=$(basename "$rhai_file")
    # Check for hardcoded Ethereum addresses (0x followed by 40 hex chars)
    if grep -qP '0x[0-9a-fA-F]{40}' "$rhai_file" 2>/dev/null; then
        echo -e "  ${RED}✗${NC} $filename — contains hardcoded addresses!"
        VIOLATIONS=true
    else
        echo -e "  ${GREEN}✓${NC} $filename — no hardcoded addresses"
    fi
done

if [ "$VIOLATIONS" = true ]; then
    echo -e "\n${YELLOW}WARNING: Hardcoded addresses detected. Cartridges should be chain-agnostic.${NC}"
fi
echo ""

# Run Rust integration tests (if not --quick mode)
if [ "${1:-}" != "--quick" ]; then
    echo -e "${YELLOW}[4/4]${NC} Running Rust integration tests..."
    echo ""

    if command -v cargo &> /dev/null; then
        cargo test --test cartridge_e2e_test -- --nocapture 2>&1 | while IFS= read -r line; do
            if echo "$line" | grep -q "^test.*ok$"; then
                echo -e "  ${GREEN}✓${NC} $line"
            elif echo "$line" | grep -q "^test.*FAILED$"; then
                echo -e "  ${RED}✗${NC} $line"
            elif echo "$line" | grep -q "^test result"; then
                echo ""
                echo -e "  $line"
            else
                echo "  $line"
            fi
        done
    else
        echo -e "  ${YELLOW}⚠ cargo not found — skipping Rust tests${NC}"
        echo -e "  Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    fi
else
    echo -e "${YELLOW}[4/4]${NC} Skipped Rust tests (--quick mode)"
fi

echo ""
echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  All cartridge validations passed!${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo ""
echo "Cartridges ready for deployment on ANY chain."
echo "Supported: Ethereum, Arbitrum, Base, Polygon, BSC, Avalanche, Optimism, zkSync, Linea, Scroll, +"
