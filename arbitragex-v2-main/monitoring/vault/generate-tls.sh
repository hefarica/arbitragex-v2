#!/usr/bin/env bash
# Generate self-signed CA + server cert for Vault TLS (intra-network use).
#
# Audit N4 (2026-05-10): Vault must serve HTTPS even on the internal Docker
# bridge network. Network isolation alone is insufficient — a container-level
# RCE on any sibling service can sniff secrets transported in plaintext over
# the shared bridge. TLS eliminates that attack surface.
#
# Usage (run once on the VPS before first `docker compose up`):
#   bash monitoring/vault/generate-tls.sh
#
# To rotate (e.g., approaching expiry):
#   rm monitoring/vault/tls/vault-{cert,key,ca}.pem
#   bash monitoring/vault/generate-tls.sh
#   docker compose -f docker/compose.prod.yml restart vault
#
# The tls/ directory is gitignored. NEVER commit cert or key files.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TLS_DIR="${SCRIPT_DIR}/tls"

mkdir -p "${TLS_DIR}"

if [ -f "${TLS_DIR}/vault-cert.pem" ]; then
    echo "Vault TLS cert already exists at ${TLS_DIR}/vault-cert.pem"
    echo "To rotate: rm ${TLS_DIR}/vault-{cert,key,ca}.pem and re-run this script."
    exit 0
fi

echo "Generating Vault TLS self-signed CA and server cert..."

# ── Step 1: Generate CA key + self-signed CA cert ──────────────────────────
openssl genrsa -out "${TLS_DIR}/vault-ca-key.pem" 2048

openssl req -new -x509 -days 365 \
    -subj "/CN=arbx-vault-ca/O=ArbitrageX/C=US" \
    -key "${TLS_DIR}/vault-ca-key.pem" \
    -out "${TLS_DIR}/vault-ca.pem"

# ── Step 2: Generate Vault server key + CSR ────────────────────────────────
openssl genrsa -out "${TLS_DIR}/vault-key.pem" 2048

openssl req -new \
    -subj "/CN=vault/O=ArbitrageX/C=US" \
    -key "${TLS_DIR}/vault-key.pem" \
    -out "${TLS_DIR}/vault-csr.pem"

# ── Step 3: SAN extension (required for modern TLS clients) ────────────────
cat > "${TLS_DIR}/vault-ext.cnf" <<EOF
[req]
req_extensions = v3_req
distinguished_name = req_distinguished_name

[req_distinguished_name]

[v3_req]
subjectAltName = DNS:vault, DNS:localhost, IP:127.0.0.1
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
EOF

# ── Step 4: Sign server cert with CA ──────────────────────────────────────
openssl x509 -req -days 365 \
    -in "${TLS_DIR}/vault-csr.pem" \
    -CA "${TLS_DIR}/vault-ca.pem" \
    -CAkey "${TLS_DIR}/vault-ca-key.pem" \
    -CAcreateserial \
    -out "${TLS_DIR}/vault-cert.pem" \
    -extensions v3_req \
    -extfile "${TLS_DIR}/vault-ext.cnf"

# ── Step 5: Cleanup intermediates ─────────────────────────────────────────
rm -f \
    "${TLS_DIR}/vault-csr.pem" \
    "${TLS_DIR}/vault-ext.cnf" \
    "${TLS_DIR}/vault-ca-key.pem" \
    "${TLS_DIR}/vault-ca.pem.srl"

# ── Step 6: Permissions ────────────────────────────────────────────────────
# vault-key.pem: readable only by owner (root inside Docker maps to uid 0).
# vault-cert.pem, vault-ca.pem: world-readable so Vault can load them after
# dropping privileges.
chmod 600 "${TLS_DIR}/vault-key.pem"
chmod 644 "${TLS_DIR}/vault-cert.pem" "${TLS_DIR}/vault-ca.pem"

echo ""
echo "Done. Files generated in ${TLS_DIR}:"
echo "  vault-ca.pem      — CA certificate (distribute to clients that verify Vault)"
echo "  vault-cert.pem    — Vault server certificate (365-day validity)"
echo "  vault-key.pem     — Vault server private key (mode 600)"
echo ""
echo "Rotation: delete the .pem files above and re-run this script."
echo "After rotation: docker compose -f docker/compose.prod.yml restart vault"
echo "Then re-run the unseal flow (Vault re-seals on restart)."
