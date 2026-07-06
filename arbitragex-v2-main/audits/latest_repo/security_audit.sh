#!/usr/bin/env bash
set -euo pipefail
cd /home/ubuntu/arbitragex-v2-latest-audit
OUT="audits/latest_repo/05_security_rpc_docker_ci.txt"
redact() {
  sed -E \
    -e 's#https?://[^[:space:]]+#https://<REDACTED_URL>#g' \
    -e 's#wss?://[^[:space:]]+#wss://<REDACTED_URL>#g' \
    -e 's#0x[A-Fa-f0-9]{64}#0x<REDACTED_PRIVATE_OR_HASH>#g' \
    -e 's#0x[A-Fa-f0-9]{32,}#0x<REDACTED_HEX>#g' \
    -e 's#([A-Za-z0-9_]*(KEY|TOKEN|SECRET|PASSWORD)[A-Za-z0-9_]*[[:space:]]*[:=][[:space:]]*)[^[:space:]]+#\1<REDACTED>#g'
}
{
  echo "# Auditoría seguridad / RPC / Docker / CI"
  echo "Fecha UTC: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
  echo
  echo "## Archivos env y ejemplos (nombres solamente)"
  find . -path './.git' -prune -o -type f \( -name '.env*' -o -name '*secret*' -o -name '*credential*' \) -print | sort | head -200 || true
  echo
  echo "## Variables críticas referenciadas (valores redactados)"
  grep -RIn --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.next -E '\b(RPC_HTTP_|RPC_WS_|ADMIN_|PRIVATE_KEY|EXECUTOR|DATABASE_URL|REDIS_URL|JWT|TOKEN|SECRET|INFURA|ALCHEMY|QUICKNODE|FLASHBOTS|WALLET)' . 2>/dev/null | redact | head -300 || true
  echo
  echo "## Posibles secretos hardcodeados por patrón (redactado)"
  grep -RIn --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.next -E '(alchemy|infura|quicknode|wss?://|https?://|0x[A-Fa-f0-9]{64}|[A-Za-z0-9_]{32,}\.[A-Za-z0-9_\-]{20,}\.[A-Za-z0-9_\-]{20,})' . 2>/dev/null | redact | head -200 || true
  echo
  echo "## RPC Fail-Honest: módulos y guards"
  grep -RIn --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.next -E 'fail.?honest|RPC_HTTP_1|RPC_WS_1|paper_mode|paper mode|live.?mode|mempool|provider.*pool|TopologyVault|topology|archive|executor' backend edge frontend docker database .github 2>/dev/null | redact | head -360 || true
  echo
  echo "## Docker Compose services y env_file"
  for f in docker/compose*.yml docker-compose*.yml; do
    if [ -f "$f" ]; then
      echo "--- $f"
      grep -nE '^[[:space:]]{0,4}[a-zA-Z0-9_-]+:|image:|build:|ports:|env_file:|environment:|depends_on:|healthcheck:|restart:' "$f" | head -220 || true
    fi
  done
  echo
  echo "## GitHub Actions: triggers y jobs"
  for f in .github/workflows/*; do
    if [ -f "$f" ]; then
      echo "--- $f"
      grep -nE '^(name:|on:|jobs:|[[:space:]]{2}[a-zA-Z0-9_-]+:|[[:space:]]+runs-on:|[[:space:]]+uses:|[[:space:]]+run:)' "$f" | head -180 || true
    fi
  done
} > "$OUT"
