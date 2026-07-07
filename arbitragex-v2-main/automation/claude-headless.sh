#!/bin/bash
# OMEGA CORTEX — Headless Mode Scripts
# Run Claude Code as non-interactive agent for CI/CD automation
# Usage: bash automation/claude-headless.sh <task>

set -euo pipefail

TASK="${1:-}"
PROJECT_DIR="/opt/arbitragex-v2"
OUTPUT_FORMAT="json"

if [ -z "$TASK" ]; then
  echo "Usage: $0 <task>"
  echo ""
  echo "Available tasks:"
  echo "  audit       — Full 4-layer audit"
  echo "  typecheck   — TypeScript + Rust compilation check"
  echo "  deploy      — Full deploy to VPS"
  echo "  status      — System health check"
  echo "  security    — Security audit with findings report"
  echo "  validate    — Math + CS + Economics validation"
  exit 1
fi

case "$TASK" in
  audit)
    echo "🔍 Running OMEGA audit (headless)..."
    claude -p "Read and execute .claude/commands/audit.md. Output findings as structured JSON." \
      --output-format json \
      --allowedTools "Read,Bash,Grep,Glob" \
      > "reports/audit-$(date +%Y%m%d-%H%M%S).json"
    ;;

  typecheck)
    echo "🔧 Running typecheck (headless)..."
    claude -p "Run: cd frontend && npx tsc --noEmit. Then: cd backend && cargo check --workspace. Report results." \
      --output-format json \
      --allowedTools "Bash,Read" \
      > "reports/typecheck-$(date +%Y%m%d-%H%M%S).json"
    ;;

  deploy)
    echo "🚀 Running deploy (headless)..."
    claude -p "Read and execute .claude/commands/deploy.md. Follow RULE 01 exactly. Report all 5 verification gates." \
      --output-format json \
      --allowedTools "Read,Bash,Grep" \
      > "reports/deploy-$(date +%Y%m%d-%H%M%S).json"
    ;;

  status)
    echo "📊 Running status check (headless)..."
    claude -p "Read and execute .claude/commands/status.md. Report component health as JSON." \
      --output-format json \
      --allowedTools "Bash,Read" \
      > "reports/status-$(date +%Y%m%d-%H%M%S).json"
    ;;

  security)
    echo "🛡️ Running security audit (headless)..."
    claude -p "Adopt the security-auditor agent from .claude/agents/security-auditor.md. Audit the entire project. Report all findings with severity, location, and remediation." \
      --output-format json \
      --allowedTools "Read,Bash,Grep,Glob" \
      > "reports/security-$(date +%Y%m%d-%H%M%S).json"
    ;;

  validate)
    echo "🧪 Running scientific validation (headless)..."
    claude -p "Run 3 validators sequentially: (1) math-validator from .claude/agents/math-validator.md — validate Bellman-Ford and numerical precision, (2) cs-validator from .claude/agents/cs-validator.md — verify concurrency and invariants, (3) economics-validator from .claude/agents/economics-validator.md — validate P&L with full cost analysis. Report all findings." \
      --output-format json \
      --allowedTools "Read,Bash,Grep,Glob" \
      > "reports/validation-$(date +%Y%m%d-%H%M%S).json"
    ;;

  *)
    echo "❌ Unknown task: $TASK"
    exit 1
    ;;
esac

echo "✅ Report saved to reports/"
