/**
 * OMEGA-102 · CI/CD VPS · static delivery manifest.
 *
 * Materializes the executable artifacts of the OMEGA-102 PR
 * (containment → unblock CI → deploy production → branch protection final).
 *
 * Data is intentionally static here: this view is a runbook console for
 * the operator. It mirrors the canonical PR body so the operator can
 * walk the four phases without re-reading markdown. No fabricated data,
 * no synthetic metrics — every value is grounded in the PR contents.
 */

export type PhaseStatus = "ready" | "pending" | "blocked" | "done";

export interface PhaseStep {
  label: string;
  cmd?: string;
  note?: string;
}

export interface Phase {
  num: number;
  id: string;
  title: string;
  scope: string;
  status: PhaseStatus;
  steps: PhaseStep[];
}

export const PHASES: Phase[] = [
  {
    num: 0,
    id: "containment",
    title: "Contención",
    scope: "Antes de cualquier merge — políticas GitHub (no commits)",
    status: "ready",
    steps: [
      {
        label: "Privatizar repo (P01 crítico)",
        cmd: "gh repo edit hefarica/arbitragex-v2 --visibility private --accept-visibility-change-consequences",
        note: "El código de Asimetría Topológica NO puede ser público — docs/REPO_VISIBILITY_POLICY.md",
      },
      {
        label: "Secret scanning + push protection",
        cmd: "gh api -X PATCH repos/hefarica/arbitragex-v2 -f security_and_analysis[secret_scanning][status]=enabled -f security_and_analysis[secret_scanning_push_protection][status]=enabled",
      },
      {
        label: "Aplicar workflows del PR (auto via merge)",
        note: "codeql.yml · dependabot.yml · policy-check.yml",
      },
      {
        label: "Branch protection · 14 required checks",
        cmd: "bash scripts/setup_branch_protection.sh",
        note: "Idempotente · requiere GITHUB_TOKEN con admin:repo",
      },
      {
        label: "Pre-commit hooks locales",
        cmd: "pip install pre-commit && cp config/pre-commit-config.yaml .pre-commit-config.yaml && pre-commit install",
      },
    ],
  },
  {
    num: 1,
    id: "unblock-ci",
    title: "Unblock CI",
    scope: "Resuelve las 5 causas raíz documentadas en OMEGA-101",
    status: "ready",
    steps: [
      {
        label: "Aplicar fixes idempotentes",
        cmd: "bash scripts/apply_omega102_fixes.sh",
      },
      {
        label: "Commit unblock",
        cmd: 'git add -A && git commit -m "fix(omega102): unblock CI · resolve 5 root causes [P01-P05]"',
      },
    ],
  },
  {
    num: 2,
    id: "deploy",
    title: "Deploy production pipeline",
    scope: "Workflow deploy-production.yml — 7 gates secuenciales",
    status: "pending",
    steps: [
      {
        label: "Trigger workflow",
        cmd: "gh workflow run deploy-production.yml --ref main -f confirm_sha=<full_sha> -f confirm_environment=production-vps",
        note: "NO paths alternativos · NO --admin · NO push directo · NO SSH manual",
      },
    ],
  },
  {
    num: 3,
    id: "branch-protection-final",
    title: "Branch protection final",
    scope: "Después del primer deploy verde — los 14 checks deben pasar al menos una vez",
    status: "pending",
    steps: [
      {
        label: "Enforce required checks",
        cmd: "bash scripts/setup_branch_protection.sh --enforce-required-checks",
        note: 'Doctrina "POR EL HAZ DE LUZ SOLO PASA QUIEN ES VISTO" materializada',
      },
    ],
  },
];

export interface RootCause {
  cause: string;
  artifact: string;
  verification: string;
  cve?: string;
}

export const ROOT_CAUSES: RootCause[] = [
  {
    cause: "lru 0.12.5 — IterMut unsound",
    artifact: "patches/01_lru_bump.patch (bump → 0.13)",
    verification: "cargo audit verde",
    cve: "RUSTSEC-2026-0002",
  },
  {
    cause: "postcss <8.5.10 via Next 14.2.3",
    artifact: "patches/02_postcss_override.patch (overrides ^8.5.10)",
    verification: "npm audit --audit-level=high verde",
    cve: "GHSA-qx2v-qp2m-jg93",
  },
  {
    cause: "rustfmt drift v2_shadow_replay.rs:436",
    artifact: "cargo fmt --all aplicado en este PR",
    verification: "cargo fmt --all -- --check PASS",
  },
  {
    cause: "Constantes fuera de chains.rs",
    artifact: "Refactor + byte tests (incluido en patches)",
    verification: "scripts/lint_no_hardcode.sh PASS",
  },
  {
    cause: "ANVIL_FORK_URL blank + reporter path",
    artifact: "Documentado en docs/DEPLOY_PRODUCTION.md § 1",
    verification: "Secret seteado por operador",
  },
];

export interface DeployGate {
  num: number;
  name: string;
  detail: string;
  kind: "human" | "ci" | "build" | "snapshot" | "deploy" | "smoke" | "cutover";
}

export const DEPLOY_GATES: DeployGate[] = [
  {
    num: 1,
    name: "Confirm environment",
    detail: "Aprobación humana @hefarica · GitHub Environment production-vps",
    kind: "human",
  },
  {
    num: 2,
    name: "CI green",
    detail: "14 required status checks PASS",
    kind: "ci",
  },
  {
    num: 3,
    name: "Build images",
    detail: "buildx multi-arch → GHCR + cosign keyless + SBOM Syft + Trivy",
    kind: "build",
  },
  {
    num: 4,
    name: "Pre-deploy snapshot",
    detail: "pg_dump + Redis BGSAVE + nginx state + vault state on-chain",
    kind: "snapshot",
  },
  {
    num: 5,
    name: "Deploy green slot",
    detail: "SSH al VPS · docker compose up · sqlx migrate",
    kind: "deploy",
  },
  {
    num: 6,
    name: "Smoke tests",
    detail: "7 checks obligatorios · scripts/smoke-tests.sh",
    kind: "smoke",
  },
  {
    num: 7,
    name: "Cutover OR rollback",
    detail: "nginx swap blue→green · o scripts/rollback.sh automático",
    kind: "cutover",
  },
];

export interface ArtifactItem {
  path: string;
  desc: string;
}

export interface ArtifactCategory {
  category: string;
  count: number;
  tone: "success" | "warning" | "info" | "secondary";
  items: ArtifactItem[];
}

export const ARTIFACTS: ArtifactCategory[] = [
  {
    category: "Workflows GitHub Actions",
    count: 4,
    tone: "success",
    items: [
      { path: ".github/workflows/codeql.yml", desc: "SAST security-extended Rust+TS" },
      { path: ".github/workflows/deploy-production.yml", desc: "pipeline 7 gates VPS" },
      {
        path: ".github/workflows/dependabot.yml",
        desc: "updates semanales cargo/npm/actions (lunes 09:00 Bogotá)",
      },
      { path: ".github/workflows/policy-check.yml", desc: "verificación horaria policies repo" },
    ],
  },
  {
    category: "Patches reproducibles",
    count: 2,
    tone: "warning",
    items: [
      { path: "patches/01_lru_bump.patch", desc: "bump lru 0.12 → 0.13 (RUSTSEC-2026-0002)" },
      {
        path: "patches/02_postcss_override.patch",
        desc: "overrides postcss ^8.5.10 (GHSA-qx2v-qp2m-jg93)",
      },
    ],
  },
  {
    category: "Scripts operativos",
    count: 4,
    tone: "info",
    items: [
      { path: "scripts/apply_omega102_fixes.sh", desc: "aplicador idempotente 5 fixes CI" },
      { path: "scripts/smoke-tests.sh", desc: "7 smoke checks post-deploy" },
      { path: "scripts/rollback.sh", desc: "rollback determinista (DB + nginx + slot)" },
      {
        path: "scripts/setup_branch_protection.sh",
        desc: "PUT branches/main/protection con 14 contexts",
      },
    ],
  },
  {
    category: "Configuración",
    count: 2,
    tone: "secondary",
    items: [
      {
        path: "config/CODEOWNERS",
        desc: "paths backend/contracts/frontend/edge/workflows → @hefarica",
      },
      {
        path: "config/pre-commit-config.yaml",
        desc: "cargo fmt+clippy, tsc, forge build, gitleaks, block .env",
      },
    ],
  },
  {
    category: "Documentación operativa",
    count: 4,
    tone: "success",
    items: [
      {
        path: "docs/INCIDENT_RUNBOOK.md",
        desc: "matriz INC-01 a INC-10 + killswitch HMAC + rollback + Tribunal",
      },
      {
        path: "docs/ROTATION_POLICY.md",
        desc: "inventario secrets + protocolo rotación + calendario 2026",
      },
      {
        path: "docs/DEPLOY_PRODUCTION.md",
        desc: "anatomía 7 gates + métricas éxito + R8 límites",
      },
      {
        path: "docs/REPO_VISIBILITY_POLICY.md",
        desc: "regla absoluta PRIVATE + monitoring + forks",
      },
    ],
  },
];

export interface R8Limit {
  topic: string;
  detail: string;
}

export const R8_LIMITS: R8Limit[] = [
  {
    topic: "Cosign keyless",
    detail: "Requiere OIDC GitHub válido — si falla, no hay alternativa local firmada",
  },
  {
    topic: "TEE attestation",
    detail: "Asume signer en SGX/SEV-SNP real — en VPS sin hardware TEE el stage no es ejecutable y se declara como limitación (fail-honest; no existe implementación sustituta)",
  },
  {
    topic: "Blue/green",
    detail: "Asume ~8GB RAM extra disponibles en VPS — degradar a recreate strategy si insuficiente",
  },
  {
    topic: "Smoke tests",
    detail: "Validan superficie HTTP — no garantizan correctness de Asimetría Topológica (requiere shadow-replay continuo, separado)",
  },
  {
    topic: "Rollback DB",
    detail: "RPO=15min — transacciones post-snapshot se pierden",
  },
];

export interface ChecklistItem {
  id: string;
  label: string;
  group: "policy" | "artifacts" | "ci" | "deploy";
}

export const CHECKLIST: ChecklistItem[] = [
  {
    id: "c1",
    label: "Repo PRIVATE confirmado via gh repo view --json visibility",
    group: "policy",
  },
  { id: "c2", label: "Secret scanning + push protection ON via API", group: "policy" },
  { id: "c3", label: "Workflows 4/4 presentes en .github/workflows/", group: "artifacts" },
  { id: "c4", label: "Patches 2/2 aplicados y commiteados", group: "artifacts" },
  { id: "c5", label: "Scripts 4/4 ejecutables (chmod +x)", group: "artifacts" },
  { id: "c6", label: "Branch protection con 14 required checks activos", group: "ci" },
  { id: "c7", label: "Pre-commit hooks instalados localmente", group: "ci" },
  {
    id: "c8",
    label: "Primer gh workflow run deploy-production.yml en staging-mode",
    group: "deploy",
  },
  { id: "c9", label: "7/7 smoke tests PASS en staging", group: "deploy" },
  {
    id: "c10",
    label: "Operador aprueba environment production-vps para primer deploy real",
    group: "deploy",
  },
];

export interface MergeBranch {
  branch: string;
  order: number;
  scope: string[];
  effect: string;
  blocker: string;
}

export const MERGE_BRANCHES: MergeBranch[] = [
  {
    branch: "fix/omega102-ci-unblock",
    order: 1,
    scope: ["patches/*", "scripts/apply_omega102_fixes.sh", "config/pre-commit-config.yaml"],
    effect: "Resuelve causas raíz CI → enables los 5 checks rojos a verdes",
    blocker: "Sin esto, el segundo PR no puede mergearse",
  },
  {
    branch: "feat/omega102-deploy-production-pipeline",
    order: 2,
    scope: [
      ".github/workflows/*",
      "scripts/{smoke-tests,rollback,setup_branch_protection}.sh",
      "config/CODEOWNERS",
      "docs/*",
    ],
    effect: "Activa el pipeline completo",
    blocker: "Depende del primer merge",
  },
];

export interface Signature {
  name: string;
  status: "PASS" | "PENDING" | "FAIL";
  note: string;
}

export const SIGNATURES: Signature[] = [
  {
    name: "Tribunal R7 (rigor científico)",
    status: "PASS",
    note: "artefactos materializados, sin teoría",
  },
  {
    name: "Tribunal R8 (fail-honest)",
    status: "PASS",
    note: 'límites declarados explícitos en § "Doctrina R8"',
  },
  {
    name: "Operador @hefarica",
    status: "PENDING",
    note: "review + merge fase por fase",
  },
];

export const META = {
  ticket: "OMEGA-102",
  title: "CI/CD VPS · Entrega ejecutable",
  doctrine: [
    "OMEGA AGENT TEAMS",
    "Ley de Lexicón Absoluto",
    "Ghost Protocol",
    "Zero-Mocks",
    "Fail-Honest R8",
  ],
  baseAudit: "OMEGA-101 sistémica E2E (27 problemas catalogados R7+R8)",
  objective:
    "llevar arbitragex-v2 del estado actual (CI 100% rojo, repo PUBLIC, branch protection sin status checks) a producción operativa en VPS via GitHub Actions",
};
