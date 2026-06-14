#!/bin/bash
set -e

# ArbitrageX v2 - Environment Encryptor
# Encripta el archivo .env actual usando la Master Key derivada del Excel.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
EXCEL_PATH="$PROJECT_ROOT/ArbitrageX_Secrets_Config.xlsx"
ENV_PATH="$PROJECT_ROOT/.env"
ENV_ENC_PATH="$PROJECT_ROOT/.env.enc"

echo "ArbitrageX v2 Environment Encryptor"
echo "==================================="

# Validate inputs
if [ ! -f "$EXCEL_PATH" ]; then
  echo "Error: Excel secrets file not found at $EXCEL_PATH"
  exit 1
fi

if [ ! -f "$ENV_PATH" ]; then
  echo "Error: Plaintext .env file not found at $ENV_PATH"
  echo "You must have a .env file to encrypt."
  exit 1
fi

echo "1. Deriving Master Key from Excel..."
# Verify key can be generated
python3 "$SCRIPT_DIR/vault_crypto.py" get-key-hash --excel "$EXCEL_PATH"

echo "2. Encrypting .env -> .env.enc..."
python3 "$SCRIPT_DIR/vault_crypto.py" encrypt \
  --excel "$EXCEL_PATH" \
  --in-file "$ENV_PATH" \
  --out-file "$ENV_ENC_PATH"

echo "==================================="
echo "Encryption successful."
echo ""
echo "CRITICAL: To fully secure the system, you must delete the plaintext .env file:"
echo "  shred -u $ENV_PATH"
echo ""
echo "From now on, start the system using:"
echo "  ./scripts/security/start_secure.sh"
