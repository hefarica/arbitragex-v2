#!/usr/bin/env bash
set -euo pipefail

# omega-scoreboard.sh — Calcula bi-eje E1/E2 sobre el scaffold
# Uso: ./scripts/omega-scoreboard.sh [ruta_al_repo]

REPO="${1:-.}"
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

eje1_score=0
eje2_score=0

check() {
  local path="$1" weight="$2" desc="$3"
  if [ -e "$REPO/$path" ]; then
    printf "  ${GREEN}✓${NC} %-50s +%.2f  %s\n" "$path" "$weight" "$desc"
    return 0
  else
    printf "  ${RED}✗${NC} %-50s +0.00  %s\n" "$path" "$desc"
    return 1
  fi
}

echo "═══════════════════════════════════════════════════════════════"
echo "  OMEGA SCOREBOARD — Bi-Eje Doctrinal"
echo "═══════════════════════════════════════════════════════════════"

echo ""
echo "📊 EJE 1 — Salud Operativa del Runtime (15 puntos)"
echo "───────────────────────────────────────────────────────────────"

# Eje 1 checks (15 points)
check "docker-compose.yml"                  1.00 "Compose principal"          && eje1_score=$(awk "BEGIN{print $eje1_score+1.00}")
check "docker-compose.prod.yml"             1.00 "Compose producción"         && eje1_score=$(awk "BEGIN{print $eje1_score+1.00}")
check "frontend/Dockerfile"                 1.00 "Dockerfile frontend"        && eje1_score=$(awk "BEGIN{print $eje1_score+1.00}")
check "infra/docker/Dockerfile.backend"     1.00 "Dockerfile backend"         && eje1_score=$(awk "BEGIN{print $eje1_score+1.00}")
check "infra/docker/Dockerfile.searcher"    1.00 "Dockerfile searcher"        && eje1_score=$(awk "BEGIN{print $eje1_score+1.00}")
check "infra/nginx/nginx.conf"              1.00 "Nginx reverse proxy"        && eje1_score=$(awk "BEGIN{print $eje1_score+1.00}")
check "infra/prometheus/prometheus.yml"     1.00 "Prometheus config"          && eje1_score=$(awk "BEGIN{print $eje1_score+1.00}")
check "infra/prometheus/rules/red-business.yml" 1.00 "Alertas RED negocio"    && eje1_score=$(awk "BEGIN{print $eje1_score+1.00}")
check "monitoring/grafana/provisioning/datasources/datasources.yml" 1.00 "Grafana datasources"  && eje1_score=$(awk "BEGIN{print $eje1_score+1.00}")
check "scripts/doctor.sh"                   1.00 "Health check script"        && eje1_score=$(awk "BEGIN{print $eje1_score+1.00}")
check "scripts/snapshot.sh"                 0.50 "Snapshot script"            && eje1_score=$(awk "BEGIN{print $eje1_score+0.50}")
check "infra/vps/deploy.sh"                 1.50 "Deploy script idempotente"  && eje1_score=$(awk "BEGIN{print $eje1_score+1.50}")
check "runbooks/kill-switch.md"             0.50 "Runbook killswitch"         && eje1_score=$(awk "BEGIN{print $eje1_score+0.50}")
check "runbooks/dr-drill.md"                0.50 "Runbook DR drill"           && eje1_score=$(awk "BEGIN{print $eje1_score+0.50}")
check "runbooks/incident-response.md"       0.50 "Runbook incidentes"         && eje1_score=$(awk "BEGIN{print $eje1_score+0.50}")

echo ""
echo "📊 EJE 2 — Madurez de Plataforma (85 puntos)"
echo "───────────────────────────────────────────────────────────────"

# Eje 2 checks (85 points)
check ".github/workflows/ci.yml"              6.00 "CI lint+build+test"         && eje2_score=$(awk "BEGIN{print $eje2_score+6.00}")
check ".github/workflows/security.yml"        6.00 "Security audit"             && eje2_score=$(awk "BEGIN{print $eje2_score+6.00}")
check ".github/workflows/docker-build.yml"    6.00 "Docker build+SBOM+cosign"  && eje2_score=$(awk "BEGIN{print $eje2_score+6.00}")
check ".github/workflows/deploy-vps.yml"      6.00 "Deploy VPS"                 && eje2_score=$(awk "BEGIN{print $eje2_score+6.00}")
check ".github/workflows/e2e.yml"             6.00 "E2E Playwright"             && eje2_score=$(awk "BEGIN{print $eje2_score+6.00}")
check ".github/workflows/codeql.yml"          6.00 "CodeQL SAST"                && eje2_score=$(awk "BEGIN{print $eje2_score+6.00}")
check ".github/CODEOWNERS"                    2.00 "CODEOWNERS"                 && eje2_score=$(awk "BEGIN{print $eje2_score+2.00}")
check ".github/PULL_REQUEST_TEMPLATE.md"      2.00 "PR template"                && eje2_score=$(awk "BEGIN{print $eje2_score+2.00}")
check ".github/dependabot.yml"                2.00 "Dependabot"                 && eje2_score=$(awk "BEGIN{print $eje2_score+2.00}")
check ".github/ISSUE_TEMPLATE/bug_report.yml"  1.00 "Bug template"               && eje2_score=$(awk "BEGIN{print $eje2_score+1.00}")
check "docs/index.md"                         3.00 "Docs index"                 && eje2_score=$(awk "BEGIN{print $eje2_score+3.00}")
check "docs/tutorials"                        4.00 "Tutorials"                  && eje2_score=$(awk "BEGIN{print $eje2_score+4.00}")
check "docs/how-to"                           4.00 "How-to guides"              && eje2_score=$(awk "BEGIN{print $eje2_score+4.00}")
check "docs/reference"                        4.00 "Reference docs"             && eje2_score=$(awk "BEGIN{print $eje2_score+4.00}")
check "docs/explanation"                      4.00 "Explanation docs"           && eje2_score=$(awk "BEGIN{print $eje2_score+4.00}")
check "docs/adrs"                             3.00 "ADRs MADR"                  && eje2_score=$(awk "BEGIN{print $eje2_score+3.00}")
check "docs/explanation/diagrams"             2.00 "Diagramas Mermaid"          && eje2_score=$(awk "BEGIN{print $eje2_score+2.00}")
check "apis/openapi.yaml"                     4.00 "OpenAPI spec"               && eje2_score=$(awk "BEGIN{print $eje2_score+4.00}")
check "apis/asyncapi.yaml"                    2.00 "AsyncAPI spec"              && eje2_score=$(awk "BEGIN{print $eje2_score+2.00}")
check "mkdocs.yml"                            2.00 "MkDocs config"              && eje2_score=$(awk "BEGIN{print $eje2_score+2.00}")
check ".editorconfig"                         1.00 "EditorConfig"               && eje2_score=$(awk "BEGIN{print $eje2_score+1.00}")
check ".gitattributes"                        1.00 "GitAttributes"              && eje2_score=$(awk "BEGIN{print $eje2_score+1.00}")
check ".nvmrc"                                1.00 "Node version"               && eje2_score=$(awk "BEGIN{print $eje2_score+1.00}")
check "rust-toolchain.toml"                   1.00 "Rust toolchain"             && eje2_score=$(awk "BEGIN{print $eje2_score+1.00}")

echo ""
echo "═══════════════════════════════════════════════════════════════"
printf "  ${YELLOW}EJE 1${NC} (Runtime):     %.2f / 15.00  =  %5.1f%%\n" "$eje1_score" "$(awk "BEGIN{print ($eje1_score/15)*100}")"
printf "  ${YELLOW}EJE 2${NC} (Madurez):    %.2f / 85.00  =  %5.1f%%\n" "$eje2_score" "$(awk "BEGIN{print ($eje2_score/85)*100}")"
printf "  ${YELLOW}TOTAL${NC} Doctrinal:    %.2f / 100.00 =  %5.1f%%\n" "$(awk "BEGIN{print $eje1_score+$eje2_score}")" "$(awk "BEGIN{print (($eje1_score+$eje2_score)/100)*100}")"
echo "═══════════════════════════════════════════════════════════════"

# Hito
TOTAL=$(awk "BEGIN{print $eje1_score+$eje2_score}")
if awk "BEGIN{exit !($TOTAL >= 70)}"; then
  printf "  ${GREEN}HITO: Paper Shadow ≥70%% ALCANZADO${NC}\n"
elif awk "BEGIN{exit !($TOTAL >= 22)}"; then
  printf "  ${GREEN}HITO: Pre-Paper Shadow ≥22%% ALCANZADO${NC}\n"
else
  printf "  ${RED}HITO: Ninguno alcanzado${NC}\n"
fi

exit 0
