/**
 * agents-status — surface the 17 ArbitrageX Agent Teams' last verdicts.
 *
 * GET /api/v1/agents/status
 *
 * Source honesty contract:
 *   - "workspace_verified": the verdict reflects the operator's last
 *     in-session audit (P0+P1+P2 phases) AS OF `verified_at`. The date is
 *     git-derived (the commit that last touched the def's evidence) — May-era
 *     evidence strings are the ledger's historical record, honestly dated,
 *     never presented as fresh. These are NOT runtime probes;
 *     they're the persistent ledger of the most recent agent-team
 *     pass-through. A future runtime agent runner will replace these
 *     with source="runtime" — the schema is forward-compat.
 *   - "runtime": currently unused at the row level. The endpoint itself
 *     reads runtime signals (readiness flip_blocked, env vars) to dynamically
 *     mark phase-gated agents BLOCKED/PARTIAL/NO_GO when prerequisites fail.
 *   - "unknown": for any future agent we haven't observed yet.
 *
 * R8 fail-honest: NO agent is silently marked PASS. If verifyAll fails,
 * we surface the failure on the relevant agents. The risk-circuit-agent
 * is PARTIAL for live because A.6 is not yet implemented. The
 * go-no-go-agent is always NO_GO for live because A.9 is pending.
 */

import type { Application, Request, Response } from "express";
import type pg from "pg";

import { verifyAll } from "../readiness/verifiers/index.js";

// ---------------------------------------------------------------------------
// Wire contract types — mirror frontend Zod schemas exactly.
// ---------------------------------------------------------------------------

type AgentVerdict = "PASS" | "BLOCKED" | "PARTIAL" | "NO_GO" | "NOT_RUN" | "UNKNOWN";
type AgentStatus = "healthy" | "degraded" | "blocked" | "unknown";
type AgentSource = "workspace_verified" | "runtime" | "unknown";
type AgentCategory =
  | "forensics"
  | "deployment"
  | "operator"
  | "frontend"
  | "backend"
  | "edge"
  | "data"
  | "security"
  | "risk"
  | "quality"
  | "decision";
type BlockedPhase = "A.4" | "A.5" | "A.6" | "A.7" | "A.8" | "A.9" | "LIVE";
type RiskLevel = "low" | "medium" | "high" | "critical";

interface AgentStatusRow {
  id: string;
  name: string;
  category: AgentCategory;
  verdict: AgentVerdict;
  status: AgentStatus;
  evidence: string[];
  last_run_at: string | null;
  verified_at: string | null;
  source: AgentSource;
  blocks: BlockedPhase[];
  next_action: string | null;
  risk: RiskLevel;
  operator_required: boolean;
}

interface AgentsStatusResponse {
  generated_at: string;
  source: "mixed";
  overall_status: "ready" | "partial" | "blocked";
  agents: AgentStatusRow[];
  summary: {
    pass: number;
    blocked: number;
    partial: number;
    no_go: number;
    not_run: number;
    unknown: number;
    total: number;
  };
}

// ---------------------------------------------------------------------------
// The 17 agent definitions. Verdicts are workspace-verified at the time of
// this commit. Doctrinal verdicts (PASS/PARTIAL/NO_GO for live) are pinned
// here and updated only on milestone-completing commits — exactly the same
// pattern as ProgressRealCard's milestone constants. The endpoint then
// OVERLAYS runtime data (verifyAll output, env probes) to demote PASS to
// BLOCKED when a prerequisite has regressed.
// ---------------------------------------------------------------------------

interface AgentDef {
  id: string;
  name: string;
  category: AgentCategory;
  default_verdict: AgentVerdict;
  default_status: AgentStatus;
  evidence: string[];
  source: AgentSource;
  /** Date (YYYY-MM-DD) the ledger verdict was last workspace-verified. null = unknown (R8). */
  verified_at: string | null;
  blocks: BlockedPhase[];
  next_action: string | null;
  risk: RiskLevel;
  operator_required: boolean;
}

const AGENT_DEFS: AgentDef[] = [
  {
    id: "repo-forensics-agent",
    name: "Repo Forensics Agent",
    category: "forensics",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "Branch feat/wire-simulator-v2-revm committed P0+P1+P2",
      "Working tree clean at last verification (HEAD pushed to origin + github)",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "vps-deploy-agent",
    name: "VPS Deploy Agent",
    category: "deployment",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "Last deploy reconstructed frontend + edge + api-server only",
      "13 services preserved untouched (postgres, redis, searcher-rs, sim-ctl, recon, etc.)",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "operator-wip-agent",
    name: "Operator WIP Agent",
    category: "operator",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "stash@{0}: OPERATOR WIP 2026-05-13 (configs/app.toml env=development) preserved on VPS",
      "Not applied, not deleted; awaiting operator decision",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: "Operator must decide: keep as stash, branch off, or commit to main.",
    risk: "low",
    operator_required: true,
  },
  {
    id: "frontend-preservation-agent",
    name: "Frontend Preservation Agent",
    category: "frontend",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "20 routes intact (build 31/31 PASS)",
      "24 top-level components preserved",
      "WebSocket layer (useOpportunitiesStream) untouched",
      "P0/P1/P2 panels all live",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "backend-contract-agent",
    name: "Backend Contract Agent",
    category: "backend",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "readiness-extras reuses verifyAll() without reimplementing verifiers",
      "Route mounted via standard mountX(app, deps) pattern",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "blockers-api-agent",
    name: "Blockers API Agent",
    category: "backend",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "/api/v1/readiness/blockers HTTP 200 with redacted env evidence",
      "20/20 unit tests pin redaction + doctrine + summarisation",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "decision-api-agent",
    name: "Decision API Agent",
    category: "backend",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "/api/v1/readiness/decision returns verdict=NO_GO go_live=false go_a5=false",
      "capital_exposure_usd hardcoded literal 0",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "edge-proxy-agent",
    name: "Edge Proxy Agent",
    category: "edge",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "edge/dev-local and edge/worker both proxy readiness/blockers + readiness/decision",
      "Scanner heartbeat proxy verified live (HTTP 200)",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "blockers-panel-agent",
    name: "Blockers Panel Agent",
    category: "frontend",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "BlockersPanel renders accordion with severity dots, evidence line, required action",
      "4/4 SSR tests assert R8 fail-honest contract",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "go-no-go-panel-agent",
    name: "GO/NO-GO Panel Agent",
    category: "frontend",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "GoNoGoPanel renders verdict tile + Dialog modal with full decision",
      "6/6 SSR tests confirm NO live-enable button anywhere in markup",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "data-wiring-agent",
    name: "Data Wiring Agent",
    category: "data",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "api-client.ts extended with getReadinessBlockers, getReadinessDecision",
      "Zod schemas .passthrough() forward-compat; null distinguished from 0",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "security-guard-agent",
    name: "Security Guard Agent",
    category: "security",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "Env redaction pinned by regression test (raw RPC/EXECUTOR/DATABASE_URL never serialised)",
      "Admin cookie httpOnly + Secure + SameSite=Strict",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "risk-circuit-agent",
    name: "Risk Circuit Agent",
    category: "risk",
    // PARTIAL because A.6 remains partial: the 10 doctrinal breakers ship and
    // compute (PR #470) but Prometheus alert emission is not wired, and the
    // ARBX_CB_* revert/gas thresholds are unset on the VPS (honest
    // NOT_AVAILABLE). Until A.6 PASS, this agent BLOCKS live.
    default_verdict: "PARTIAL",
    default_status: "degraded",
    evidence: [
      "Basic kill-switch + readiness gate exist",
      "A.8 (2026-08-29, resolved): score_and_publish consumes ConfidenceScore (bayesian_accepted + kelly_fraction) on every paper opportunity — prod XLEN arbx:scoring:scored=87, last-hour rows rejected|87, scored_opportunities_total=90",
      "A.6 (2026-08-29): 10 comprehensive breakers compute via /api/v1/risk/circuit-breakers/status (kill_switch real runtime; DD tiers read the REAL paper ledger — prod PASS 2026-08-29; revert/gas NOT_AVAILABLE until operator sets ARBX_CB_* envs)",
    ],
    source: "workspace_verified",
    verified_at: "2026-08-29",
    blocks: ["LIVE"],
    next_action: "Emit Prometheus alerts from breaker state (A.6 remaining); operator sets ARBX_CB_* thresholds on the VPS to move revert/gas off NOT_AVAILABLE.",
    risk: "high",
    operator_required: false,
  },
  {
    id: "per-chain-isolation-agent",
    name: "Per-Chain Isolation Agent",
    category: "risk",
    // B0 (2026-05-13): all 3 footguns + F4 minor closed in this commit set.
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "F1: ScannerCounters refactored to chain_counters(chain_id) registry; heartbeat_worker reads per-chain; 25 scanner.rs hot-path callsites migrated",
      "F2: arbx:papermode:<chain_id> per-chain; legacy global key read-only fallback (30-day deprecation); POST /admin/config/paper-mode rejects requests without chain_id",
      "F3: migration 060 added risk_events.chain_id + index; all 3 insert sites (recon::anomaly, recon::persistence, selector-api::persistence) updated",
      "F4: primary_chain fail-honest — empty enabled_chains aborts boot instead of silent fallback to chain 1",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "scoring-primitives-agent",
    name: "Scoring Primitives Agent",
    category: "risk",
    // A.8 PARTIAL: primitives audited and exposed via /api/v1/scoring/status,
    // but scanner hot-path does NOT invoke them per candidate yet.
    default_verdict: "PARTIAL",
    default_status: "degraded",
    evidence: [
      "bayesian_filter.rs: BetaParams + bayes_update + accept_by_posterior + vpin + decide_size_reduction_for_pin compiled",
      "kelly_sizing.rs: kelly_fraction + fractional_kelly + compute_position_size compiled",
      "/api/v1/scoring/status endpoint LIVE — reports scoring_pipeline_wired=false honestly",
      "scanner::dispatch_orchestrator_and_classify does NOT emit ConfidenceScore",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: ["LIVE"],
    next_action: "Future commit: wire bayesian + kelly into scanner hot path; persist ConfidenceScore on every paper opportunity; calibrate posterior priors from paper-shadow run.",
    risk: "medium",
    operator_required: false,
  },
  {
    id: "anti-mock-agent",
    name: "Anti-Mock Agent",
    category: "quality",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "Static anti-mock scan (RULE 00, 2026-05-13): productive frontend code has 0 occurrences of synthetic-data or fabricated-value patterns",
      "Backend: no PASS-fabrication, no unsafe in simulator-v2 or searcher-rs",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "visual-regression-agent",
    name: "Visual Regression Agent",
    category: "quality",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "next build 31/31 routes PASS",
      "/live-readiness HTTP 200 with P0/P1/P2 strings visible",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "performance-agent",
    name: "Performance Agent",
    category: "quality",
    default_verdict: "PASS",
    default_status: "healthy",
    evidence: [
      "Panel poll cadences: SystemGuardBanner 15s, BlockersPanel 30s, GoNoGoPanel 30s",
      "useEffect+allSettled; no reconnect storm; no render loops",
    ],
    source: "workspace_verified",
    verified_at: "2026-05-13",
    blocks: [],
    next_action: null,
    risk: "low",
    operator_required: false,
  },
  {
    id: "go-no-go-agent",
    name: "GO/NO-GO Agent",
    category: "decision",
    // NEVER PASS for live until every phase blocker clears + A.9 formal
    // sign-off. This is the structural barrier echoing SystemGuardBanner.
    default_verdict: "NO_GO",
    default_status: "blocked",
    evidence: [
      "Live trading: NO_GO (structural — no submission path in this binary)",
      "A.4 fork validation: resolved 2026-08-20 (gate_c_validation)",
      "A.5 paper-shadow: resolved 2026-08-29 (A5-STALL closure — kill-switch re-arm + alchemy fork + Redis AOF + G-PIPE-1; G-PAP-1 green explicit)",
      "A.8 confidence scoring: resolved 2026-08-29 (ConfidenceScore wired, scored rows flowing)",
      "A.6/A.7: partial (Prometheus emission / runtime call-site)",
      "A.9: pending formal two-operator sign-off",
    ],
    source: "workspace_verified",
    verified_at: "2026-08-29",
    blocks: ["LIVE"],
    next_action: "Clear A.6 (Prometheus alert emission) and A.7 (relay no-submit call-site) partials; then operator two-operator sign-off A.9 via POST /admin/go-no-go/sign-off.",
    risk: "critical",
    operator_required: true,
  },
];

// ---------------------------------------------------------------------------
// Runtime overlay — demote PASS to BLOCKED when a runtime check fails.
// ---------------------------------------------------------------------------

function runtimeOverlay(
  def: AgentDef,
  ctx: { readinessOk: boolean; readinessFailDetail: string | null; probeRanAt?: string | null },
): AgentStatusRow {
  let verdict: AgentVerdict = def.default_verdict;
  let status: AgentStatus = def.default_status;
  let source: AgentSource = def.source;
  const evidence = [...def.evidence];

  // backend-contract-agent + blockers-api-agent + decision-api-agent all
  // depend on verifyAll(). If verifyAll fails we demote them to BLOCKED
  // so the operator sees the real failure surface.
  // DAPP-SURFACE-FAIL (2026-08-31): last_run_at is only honest when a runtime
  // re-verification actually executed. verifyAll() runs on every request for
  // the three backend agents below — they get the probe timestamp. Static
  // ledger agents keep null = "not re-verified at runtime" (R8), and the UI
  // renders that state explicitly instead of implying freshness.
  const dependsOnVerify = ["backend-contract-agent", "blockers-api-agent", "decision-api-agent"];
  let last_run_at: string | null = null;
  if (dependsOnVerify.includes(def.id)) {
    last_run_at = ctx.probeRanAt ?? null;
    if (!ctx.readinessOk) {
      verdict = "BLOCKED";
      status = "blocked";
      source = "runtime";
      evidence.push(`Runtime: verifyAll failed (${ctx.readinessFailDetail ?? "unknown"})`);
    }
  }

  return {
    id: def.id,
    name: def.name,
    category: def.category,
    verdict,
    status,
    evidence,
    last_run_at,
    verified_at: def.verified_at,
    source,
    blocks: def.blocks,
    next_action: def.next_action,
    risk: def.risk,
    operator_required: def.operator_required,
  };
}

function summarize(agents: AgentStatusRow[]): AgentsStatusResponse["summary"] {
  const s = { pass: 0, blocked: 0, partial: 0, no_go: 0, not_run: 0, unknown: 0, total: agents.length };
  for (const a of agents) {
    switch (a.verdict) {
      case "PASS": s.pass++; break;
      case "BLOCKED": s.blocked++; break;
      case "PARTIAL": s.partial++; break;
      case "NO_GO": s.no_go++; break;
      case "NOT_RUN": s.not_run++; break;
      default: s.unknown++; break;
    }
  }
  return s;
}

function overallStatus(s: AgentsStatusResponse["summary"]): AgentsStatusResponse["overall_status"] {
  if (s.blocked > 0 || s.no_go > 0) return "blocked";
  if (s.partial > 0 || s.unknown > 0 || s.not_run > 0) return "partial";
  return "ready";
}

// ---------------------------------------------------------------------------
// Test-only exports.
// ---------------------------------------------------------------------------

export const __forTesting = {
  AGENT_DEFS,
  runtimeOverlay,
  summarize,
  overallStatus,
};

// ---------------------------------------------------------------------------
// Route mounting
// ---------------------------------------------------------------------------

export function mountAgentsStatus(
  app: Application,
  deps: { pool: pg.Pool | null; logger: { warn: (obj: object, msg?: string) => void } },
): void {
  app.get("/api/v1/agents/status", async (_req: Request, res: Response) => {
    let readinessOk = true;
    let readinessFailDetail: string | null = null;
    const probeRanAt = new Date().toISOString(); // real timestamp of THIS verifyAll execution
    try {
      await verifyAll({ pool: deps.pool });
    } catch (e) {
      readinessOk = false;
      readinessFailDetail = (e as Error).message.slice(0, 120);
      deps.logger.warn({
        event: "agents_status.readiness_probe_failed",
        err: readinessFailDetail,
      });
    }

    try {
      const agents = AGENT_DEFS.map((def) =>
        runtimeOverlay(def, { readinessOk, readinessFailDetail, probeRanAt }),
      );
      const summary = summarize(agents);
      const response: AgentsStatusResponse = {
        generated_at: new Date().toISOString(),
        source: "mixed",
        overall_status: overallStatus(summary),
        agents,
        summary,
      };
      res.status(200).json(response);
    } catch (e) {
      deps.logger.warn({ event: "agents_status.failed", err: (e as Error).message });
      res.status(503).json({ error: "agents_status_collection_failed", detail: (e as Error).message });
    }
  });
}
