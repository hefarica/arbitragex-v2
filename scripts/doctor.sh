#!/usr/bin/env bash
set -euo pipefail

# doctor.sh — Health check local del repositorio

REPO="${1:-.}"
FAILED=0

check() {
  local desc="$1" cmd="$2"
  if eval "$cmd" > /dev/null 2>&1; then
    echo "  ✅ $desc"
    return 0
  else
    echo "  ❌ $desc"
    FAILED=$((FAILED + 1))
    return 1
  fi
}

echo "═══════════════════════════════════════════════════════════════"
echo "  OMEGA DOCTOR — Repo Health Check"
echo "═══════════════════════════════════════════════════════════════"

cd "$REPO"

check "Git repo initialized"            "git rev-parse --git-dir"
check "Git status clean"                "git diff-index --quiet HEAD"
check "Node.js 20+ installed"           "node -e 'process.exit(process.version.split(\".\")[0] < \"v20\" ? 1 : 0)'"
check "npm installed"                   "which npm"
check "Rust installed"                  "which rustc"
check "Docker running"                  "docker info"
check "Docker Compose available"        "docker compose version"
check ".env file exists"                "test -f .env"
check "Required ports free (:8080)"     "! lsof -i :8080"
check "Required ports free (:5173)"     "! lsof -i :5173"
check "Required ports free (:5432)"     "! lsof -i :5432"
check "Required ports free (:6379)"     "! lsof -i :6379"
check "Backend builds"                  "cd backend && cargo check"
check "Frontend builds"                 "cd frontend && npm run build"
check "Tests pass (backend)"            "cd backend && cargo test"
check "Tests pass (frontend)"           "cd frontend && npm test"

echo ""
echo "═══════════════════════════════════════════════════════════════"
if [ $FAILED -eq 0 ]; then
  echo "  ✅ ALL CHECKS PASSED — Repo is healthy"
  exit 0
else
  echo "  ❌ $FAILED check(s) failed — See details above"
  exit 1
fi
