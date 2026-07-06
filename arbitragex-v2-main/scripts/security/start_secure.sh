#!/bin/bash
set -e

# ArbitrageX v2 - Secure Startup Wrapper
# Desencripta el .env en memoria temporalmente para levantar Docker Compose
# sin dejar los secrets en el disco físico.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
EXCEL_PATH="$PROJECT_ROOT/ArbitrageX_Secrets_Config.xlsx"
ENV_ENC_PATH="$PROJECT_ROOT/.env.enc"
ENV_TMP_PATH="/dev/shm/.env.arbx.tmp"

# Check if running as root (needed for /dev/shm security)
if [ "$EUID" -ne 0 ]; then
  echo "Warning: Running without root privileges. Make sure /dev/shm is secure."
fi

# Validate inputs
if [ ! -f "$EXCEL_PATH" ]; then
  echo "Error: Excel secrets file not found at $EXCEL_PATH"
  echo "This file is required as the Master Key to decrypt the environment."
  exit 1
fi

if [ ! -f "$ENV_ENC_PATH" ]; then
  echo "Error: Encrypted env file not found at $ENV_ENC_PATH"
  echo "Run encrypt_env.sh first to encrypt your .env file."
  exit 1
fi

# Ensure cleanup happens even if the script fails or is interrupted
cleanup() {
  if [ -f "$ENV_TMP_PATH" ]; then
    echo "Shredding temporary decrypted secrets from memory..."
    # Try to use shred if available, otherwise rm
    if command -v shred >/dev/null 2>&1; then
      shred -u "$ENV_TMP_PATH"
    else
      rm -P "$ENV_TMP_PATH" 2>/dev/null || rm -f "$ENV_TMP_PATH"
    fi
  fi
}
trap cleanup EXIT INT TERM

echo "ArbitrageX v2 Secure Startup"
echo "============================"
echo "1. Deriving Master Key from Excel..."

# Ensure the python script is executable
chmod +x "$SCRIPT_DIR/vault_crypto.py"

# Decrypt to /dev/shm (tmpfs - RAM only)
echo "2. Decrypting .env to RAM (/dev/shm)..."
python3 "$SCRIPT_DIR/vault_crypto.py" decrypt \
  --excel "$EXCEL_PATH" \
  --in-file "$ENV_ENC_PATH" \
  --out-file "$ENV_TMP_PATH"

# Ensure the file has strict permissions
chmod 600 "$ENV_TMP_PATH"

echo "3. Starting Docker Compose with decrypted secrets..."
cd "$PROJECT_ROOT/docker"

# Pass the temporary file to docker compose
docker compose -f compose.prod.yml --env-file "$ENV_TMP_PATH" up -d "$@"

echo "============================"
echo "Services started successfully."
echo "Temporary secrets will be shredded now."
# The trap will run cleanup() automatically here
