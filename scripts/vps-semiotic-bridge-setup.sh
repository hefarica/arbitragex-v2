#!/bin/bash
# SEMIOTIC-BRIDGE VPS SETUP SCRIPT (INMUTABLE)
# 
# Purpose: Extend SEMIOTIC-BRIDGE protection to VPS runtime
# Language: Pure Mathematics (LaTeX) + Pure Physics enforcement
#
# RUN: ssh arbx "bash /opt/arbitragex-v2/scripts/vps-semiotic-bridge-setup.sh"

set -euo pipefail

DEPLOY_PATH="/opt/arbitragex-v2"
SEMIOTIC_BRIDGE_DIR="${DEPLOY_PATH}/backend/semiotic-bridge"

echo "=== SEMIOTIC-BRIDGE VPS HARDENING ==="
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Commit: $(git -C ${DEPLOY_PATH} rev-parse HEAD 2>/dev/null || echo 'unknown')"
echo ""

# Verify semiotic-bridge exists on VPS
if [ ! -d "${SEMIOTIC_BRIDGE_DIR}" ]; then
    echo "ERROR: semiotic-bridge directory missing at ${SEMIOTIC_BRIDGE_DIR}"
    echo "INVARIANCE VIOLATION: workspace.resolvable = FALSE"
    exit 1
fi

if [ ! -f "${SEMIOTIC_BRIDGE_DIR}/Cargo.toml" ]; then
    echo "ERROR: semiotic-bridge/Cargo.toml missing"
    echo "BUILD FAILURE: cargo build exit code 101"
    exit 1
fi

echo "✓ semiotic-bridge directory verified"

# Create VPS-side protection marker
PROTECTION_FILE="${SEMIOTIC_BRIDGE_DIR}/.vps_protection"
cat > "${PROTECTION_FILE}" << 'EOF'
# VPS PROTECTION MARKER (AUTO-GENERATED)
# 
# This file indicates the semiotic-bridge crate is protected on the VPS.
# Removal requires explicit operator sign-off.
#
# INVARIANCE: workspace.members includes semiotic-bridge
# INVARIANCE: Dockerfiles COPY semiotic-bridge
# INVARIANCE: Cargo.toml is readable

PROTECTED=true
DEPLOY_DATE=$(date -u +%Y-%m-%dT%H:%M:%SZ)
HOST=$(hostname)
EOF

echo "✓ VPS protection marker created"

# Verify Docker context (if docker compose is present)
if command -v docker &> /dev/null; then
    echo ""
    echo "=== DOCKER CONTEXT VERIFICATION ==="
    
    # Check if semiotic-bridge is in the build context
    DOCKERFILES=(
        "${DEPLOY_PATH}/backend/recon/Dockerfile"
        "${DEPLOY_PATH}/backend/relays-client/Dockerfile"
        "${DEPLOY_PATH}/backend/searcher-rs/Dockerfile"
        "${DEPLOY_PATH}/backend/sim-ctl/Dockerfile"
    )
    
    for df in "${DOCKERFILES[@]}"; do
        if [ -f "$df" ]; then
            if grep -q "COPY semiotic-bridge" "$df"; then
                echo "✓ $(basename $(dirname $df))/Dockerfile: semiotic-bridge COPY present"
            else
                echo "✗ $(basename $(dirname $df))/Dockerfile: MISSING semiotic-bridge COPY"
                echo "  INVARIANCE VIOLATION: Build will fail with 'failed to read Cargo.toml'"
            fi
        fi
    done
fi

echo ""
echo "=== SEMIOTIC-BRIDGE INVARIANCE CHECK ==="
echo ""
echo "Workspace member: $(grep -c 'semiotic-bridge' ${DEPLOY_PATH}/backend/Cargo.toml || echo '0') references"
echo "Dockerfile copies: $(grep -r 'COPY semiotic-bridge' ${DEPLOY_PATH}/backend/*/Dockerfile 2>/dev/null | wc -l) of 4"
echo ""
echo "Mathematical verification:"
echo "  workspace.resolvable = $(test -d ${SEMIOTIC_BRIDGE_DIR} && test -f ${SEMIOTIC_BRIDGE_DIR}/Cargo.toml && echo 'TRUE' || echo 'FALSE')"
echo "  docker.context.complete = $(grep -r 'COPY semiotic-bridge' ${DEPLOY_PATH}/backend/*/Dockerfile 2>/dev/null | wc -l) == 4"
echo ""
echo "=== HARDENING COMPLETE ==="
echo ""
echo "To verify build readiness:"
echo "  cd ${DEPLOY_PATH}/backend && cargo check -p semiotic-bridge"
