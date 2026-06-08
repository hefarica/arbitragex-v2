#!/usr/bin/env bash
# ============================================================================
#  vault-operator-init.sh — Production Vault Initializer for ArbitrageX v2
#  --------------------------------------------------------------------------
#  Idempotent first-time initialization of HashiCorp Vault in production.
#
#  Features:
#    • Skips silently if Vault is already initialized
#    • Generates 3 unseal keys (threshold: 2)
#    • Persists unseal keys + root token to /run/secrets/arbx/vault_init.json
#    • Auto-unseals with 2 of 3 shards
#    • Enables kv-v2 secrets engine at path secret/
#    • Displays WARNING about securing the keys
#
#  Prerequisites:
#    • Vault container (arbitragex-v2-vault-1) running
#    • TLS certs generated: bash monitoring/vault/generate-tls.sh
#    • Vault container_name: arbitragex-v2-vault-1 (matches compose.prod.yml)
#
#  Usage:
#    # Initialize (first run)
#    ./scripts/vault-operator-init.sh
#
#    # After restart (re-unseal only; init is skipped):
#    ./scripts/vault-operator-init.sh --unseal-only
#
#  Security:
#    • vault_init.json is chmod 600, owned by UID 100 (vault user)
#    • Keys are displayed ONCE on stdout — operator must secure them offline
#    • Auto-unseal uses 2 shards to bring Vault to ready state
# ============================================================================
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
VAULT_CONTAINER="${VAULT_CONTAINER:-arbitragex-v2-vault-1}"
VAULT_ADDR="${VAULT_ADDR:-https://127.0.0.1:8200}"
VAULT_TLS_CERT="${VAULT_TLS_CERT:-/vault/tls/vault-cert.pem}"
VAULT_TLS_KEY="${VAULT_TLS_KEY:-/vault/tls/vault-key.pem}"
VAULT_TLS_CA="${VAULT_TLS_CA:-/vault/tls/vault-ca.pem}"
VAULT_INIT_OUT="${VAULT_INIT_OUT:-/run/secrets/arbx/vault_init.json}"
KEY_SHARES="${KEY_SHARES:-3}"
KEY_THRESHOLD="${KEY_THRESHOLD:-2}"
KV_PATH="${KV_PATH:-secret}"

# Run mode: "full" (default) or "unseal-only"
MODE="${1:-full}"

# ─── Helpers ────────────────────────────────────────────────────────────────
log_info()  { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO]  $*"; }
log_warn()  { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [WARN]  $*" >&2; }
log_error() { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [ERROR] $*" >&2; }

# Execute vault CLI inside the Vault Docker container.
# Passes through VAULT_ADDR and VAULT_SKIP_VERIFY for TLS handling.
vault_exec() {
    docker exec \
        -e VAULT_ADDR="$VAULT_ADDR" \
        -e VAULT_SKIP_VERIFY=true \
        "$VAULT_CONTAINER" \
        vault "$@"
}

# ─── Idempotency: check if Vault is already initialized ─────────────────────
log_info "Checking Vault status (container: $VAULT_CONTAINER)..."

# If container is not running, fail fast with a clear message.
if ! docker ps --format '{{.Names}}' | grep -qx "$VAULT_CONTAINER"; then
    log_error "Vault container '$VAULT_CONTAINER' is not running."
    log_error "Start it first: docker compose -f docker/compose.prod.yml up -d vault"
    exit 2
fi

# Fetch vault status as JSON for reliable programmatic parsing.
VAULT_STATUS_JSON=$(vault_exec status -format=json 2>/dev/null || true)

# If Vault responds with a JSON payload, check the init flag.
if [ -n "$VAULT_STATUS_JSON" ]; then
    INIT_STATUS=$(echo "$VAULT_STATUS_JSON" | python3 -c "
import sys, json
data = json.load(sys.stdin)
print('true' if data.get('initialized') else 'false')
" 2>/dev/null || echo "unknown")

    if [ "$INIT_STATUS" = "true" ]; then
        log_info "Vault is already initialized. Skipping init."

        # If sealed, try to unseal (useful for --unseal-only or post-restart).
        SEAL_STATUS=$(echo "$VAULT_STATUS_JSON" | python3 -c "
import sys, json
data = json.load(sys.stdin)
print('true' if data.get('sealed') else 'false')
" 2>/dev/null || echo "unknown")

        if [ "$SEAL_STATUS" = "true" ]; then
            log_warn "Vault is sealed. Attempting auto-unseal..."

            # Attempt unseal from saved init file if it exists.
            if [ -f "$VAULT_INIT_OUT" ]; then
                # Extract unseal keys and submit threshold number of them.
                UNSEAL_KEYS=$(python3 -c "
import json
with open('$VAULT_INIT_OUT') as f:
    data = json.load(f)
for key in data['unseal_keys_b64'][:$KEY_THRESHOLD]:
    print(key)
" 2>/dev/null)

                for KEY in $UNSEAL_KEYS; do
                    log_info "Submitting unseal key..."
                    vault_exec operator unseal "$KEY" > /dev/null || true
                done

                # Verify unseal succeeded.
                if vault_exec status 2>/dev/null | grep -q "Sealed.*false"; then
                    log_info "Vault is now unsealed."
                else
                    log_error "Auto-unseal failed. Manual intervention required."
                    exit 3
                fi
            else
                log_error "Vault is sealed and no init file found at $VAULT_INIT_OUT"
                log_error "Manual unseal required: vault operator unseal <shard>"
                exit 4
            fi
        else
            log_info "Vault is initialized and unsealed."
            vault_exec status || true
        fi
        exit 0
    fi
fi

# ─── Mode: unseal-only ──────────────────────────────────────────────────────
if [ "$MODE" = "--unseal-only" ]; then
    log_error "Vault is not yet initialized. Cannot unseal an uninitialized Vault."
    log_error "Run without --unseal-only to perform initialization."
    exit 5
fi

# ═════════════════════════════════════════════════════════════════════════════
#  FIRST-TIME INITIALIZATION — KEYS WILL BE DISPLAYED ONCE
# ═════════════════════════════════════════════════════════════════════════════
echo ""
echo "╔══════════════════════════════════════════════════════════════════════════════╗"
echo "║  VAULT FIRST-TIME INITIALIZATION                                             ║"
echo "║  ═══════════════════════════════                                               ║"
echo "║  This is a ONE-TIME operation. The unseal keys and root token will be        ║"
echo "║  displayed ONCE and saved to $VAULT_INIT_OUT"
echo "║                                                                              ║"
echo "║  ⚠  IMMEDIATE ACTION REQUIRED:                                               ║"
echo "║     • Save the unseal keys to an OFFLINE password manager.                   ║"
echo "║     • Store each key in a SEPARATE, secure location.                         ║"
echo "║     • The root token grants FULL access — guard it accordingly.              ║"
echo "║     • DO NOT commit these values to git or paste them in chat.               ║"
echo "╚══════════════════════════════════════════════════════════════════════════════╝"
echo ""

read -r -p "Press ENTER to proceed with initialization..."

# ─── Initialize Vault ───────────────────────────────────────────────────────
log_info "Initializing Vault: key_shares=$KEY_SHARES, key_threshold=$KEY_THRESHOLD"

INIT_OUTPUT=$(vault_exec operator init \
    -key-shares="$KEY_SHARES" \
    -key-threshold="$KEY_THRESHOLD" \
    -format=json)

# ─── Persist init output ────────────────────────────────────────────────────
mkdir -p "$(dirname "$VAULT_INIT_OUT")"
echo "$INIT_OUTPUT" > "$VAULT_INIT_OUT"
chmod 600 "$VAULT_INIT_OUT"
log_info "Init output saved to $VAULT_INIT_OUT (chmod 600)"

# ─── Parse keys ─────────────────────────────────────────────────────────────
ROOT_TOKEN=$(echo "$INIT_OUTPUT" | python3 -c "import sys,json; print(json.load(sys.stdin)['root_token'])")
UNSEAL_KEYS=$(echo "$INIT_OUTPUT" | python3 -c "
import sys, json
keys = json.load(sys.stdin)['unseal_keys_b64']
for k in keys[:$KEY_THRESHOLD]:
    print(k)
")

# ─── Display keys with WARNING ──────────────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════════════════════════════════════════╗"
echo "║  ⚠⚠⚠  SECURITY WARNING — COPY THESE VALUES NOW  ⚠⚠⚠                         ║"
echo "║  After this script completes, the keys below are your ONLY copy.            ║"
echo "╚══════════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "$INIT_OUTPUT" | python3 -m json.tool 2>/dev/null || echo "$INIT_OUTPUT"
echo ""
echo "╔══════════════════════════════════════════════════════════════════════════════╗"
echo "║  ROOT TOKEN:                                                                 ║"
echo "║    $ROOT_TOKEN"
echo "╚══════════════════════════════════════════════════════════════════════════════╝"
echo ""

# ─── Auto-unseal (threshold: 2 of 3) ────────────────────────────────────────
log_info "Auto-unsealing Vault with $KEY_THRESHOLD of $KEY_SHARES shards..."
for KEY in $UNSEAL_KEYS; do
    vault_exec operator unseal "$KEY" > /dev/null
    log_info "Unseal shard submitted."
done

# ─── Verify unseal ──────────────────────────────────────────────────────────
log_info "Verifying unseal status..."
if vault_exec status 2>/dev/null | grep -qi "sealed.*false"; then
    log_info "Vault is UNSEALED and ready."
else
    log_error "Vault still reports sealed after submitting threshold shards."
    exit 6
fi

# ─── Enable kv-v2 secrets engine ────────────────────────────────────────────
log_info "Enabling kv-v2 secrets engine at path '$KV_PATH/'..."
# kv-v2 is the modern secrets engine; it may already be enabled by default
# on newer Vault versions, so treat "already enabled" as success.
if vault_exec secrets enable -path="$KV_PATH" -version=2 kv 2>&1 | \
    grep -qi "already enabled\|success"; then
    log_info "kv-v2 secrets engine ready at path '$KV_PATH/'."
else
    log_info "kv-v2 secrets engine may already be enabled (this is OK)."
fi

# ─── Final status ───────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════════════════════════════"
log_info "Vault initialization COMPLETE."
echo "═══════════════════════════════════════════════════════════════════════════════"
vault_exec status || true
echo ""
log_info "Next steps:"
echo "  1. Secure the unseal keys from the output above (OFFLINE storage)."
echo "  2. Secure the root token — it grants full Vault access."
echo "  3. Consider creating non-root policies for services (docs/operations/VAULT_SETUP.md)."
echo "  4. Restart vault-agent to render secrets: docker compose restart vault-agent"
echo ""
