/**
 * wallet — READ-ONLY / PAPER wallet surface for the operator console.
 *
 *   GET  /api/wallet/status   → wallet runtime posture (safe, honest)
 *   GET  /api/wallet/safety   → the safety gate matrix (HARDCODED-safe)
 *   POST /api/wallet/intent   → register a transaction INTENT (never broadcasts)
 *   POST /api/wallet/simulate → simulate an intent (never broadcasts)
 *
 * SECURITY DOCTRINE (HARD INVARIANTS — never violated):
 *   • live_enabled is STRUCTURALLY false. capital_exposed is STRUCTURALLY 0.
 *     broadcast is STRUCTURALLY false. These are compile-time literals, not
 *     runtime-tunable values — no env, query, or body can flip them.
 *   • There is NO signer, NO private key, NO mnemonic anywhere in this module.
 *   • An intent ALWAYS terminates at state BROADCAST_DISABLED with
 *     broadcast_allowed:false and reason:"broadcast_disabled_by_policy".
 *   • Simulation NEVER leads to a broadcast. The intent state machine has no
 *     transition out of BROADCAST_DISABLED toward any send.
 *
 * Fail-honest (RULE 00 / R8): where a real wallet runtime isn't configured we
 * return status:"unavailable" with reason:"wallet_runtime_not_configured" — we
 * never fabricate balances, allowances, or a "ready to execute" posture.
 */

import type { Application, Request, Response } from "express";

// ---------------------------------------------------------------------------
// Compile-time safe posture. `as const` makes these literals; TypeScript will
// reject any code that tries to assign `true` / non-zero here.
// ---------------------------------------------------------------------------

const SAFE_POSTURE = {
  live_enabled: false as const,
  capital_exposed: 0 as const,
  broadcast: false as const,
};

const BROADCAST_DISABLED_REASON = "broadcast_disabled_by_policy" as const;

/** The terminal intent state. No transition leaves it toward a send. */
const TERMINAL_STATE = "BROADCAST_DISABLED" as const;

/** All legal intent states (documented for the frontend state machine). */
export const INTENT_STATES = [
  "DRAFT",
  "SIMULATION_REQUIRED",
  "SIMULATION_FAILED",
  "SIMULATION_PASSED",
  "PAPER_ONLY",
  "BLOCKED_BY_RISK",
  "BLOCKED_BY_LIVE_OFF",
  "READY_FOR_OPERATOR_APPROVAL",
  "BROADCAST_DISABLED",
] as const;
export type IntentState = (typeof INTENT_STATES)[number];

// ---------------------------------------------------------------------------
// Safety gate matrix — every gate is a hard policy assertion that PASSES at the
// safe value. This is the institutional "why you cannot broadcast" view.
// ---------------------------------------------------------------------------

interface SafetyGate {
  name: string;
  status: "PASS";
  value: string | number | boolean;
  reason: string;
}

function safetyGates(): SafetyGate[] {
  return [
    { name: "live_enabled", status: "PASS", value: false, reason: "Live execution is disabled by policy (read-only/paper)." },
    { name: "capital_exposure", status: "PASS", value: 0, reason: "Capital exposed is structurally 0 — no funds are at risk." },
    { name: "broadcast", status: "PASS", value: false, reason: "Transaction broadcast is disabled by policy." },
    { name: "signer_present", status: "PASS", value: false, reason: "No server-side signer / private key exists in this surface." },
    { name: "blind_signing", status: "PASS", value: false, reason: "Blind signing is forbidden — every intent requires simulation first." },
    { name: "simulation_required", status: "PASS", value: true, reason: "Every transaction intent must be simulated before any operator review." },
    { name: "mode", status: "PASS", value: "paper", reason: "Trade mode is paper/read-only; no network send path is wired." },
  ];
}

// ---------------------------------------------------------------------------
// Intent body — hand-validated. We accept a high-level description of what the
// operator WOULD do; we never act on it.
// ---------------------------------------------------------------------------

interface IntentInput {
  chain_id?: number | undefined;
  kind?: string | undefined; // e.g. "swap" | "approve" | "transfer" — descriptive only.
  from?: string | undefined;
  to?: string | undefined;
  value?: string | undefined; // string to avoid float capital math.
  data?: string | undefined;
  note?: string | undefined;
}

function readIntent(body: unknown): IntentInput {
  if (!body || typeof body !== "object") return {};
  const b = body as Record<string, unknown>;
  const str = (v: unknown): string | undefined => (typeof v === "string" ? v : undefined);
  const num = (v: unknown): number | undefined => (typeof v === "number" && Number.isFinite(v) ? v : undefined);
  return {
    chain_id: num(b.chain_id),
    kind: str(b.kind),
    from: str(b.from),
    to: str(b.to),
    value: str(b.value),
    data: str(b.data),
    note: str(b.note),
  };
}

function intentId(): string {
  // Non-cryptographic correlation id; safe to expose. No secret material.
  return `intent_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 10)}`;
}

// ---------------------------------------------------------------------------
// /api/wallet/simulate contract. DENY-BY-DEFAULT: `allow` requires EVERY gate
// green. `live_gate_open` is STRUCTURALLY false, so `allow` is always false from
// this surface — the endpoint INFORMS the client policy engine; it authorizes
// nothing. When no fork-simulation runtime is wired we are fail-closed:
// simulation_passed=false, calldata_hash_matches_sim=false, reason=runtime_not_configured.
// ---------------------------------------------------------------------------

const RISK_SCORE_MIN = 60;

export interface SimulateGates {
  simulation_passed: boolean;
  calldata_hash_matches_sim: boolean;
  net_profit_usd: number | null;
  risk_score: number | null;
  gas_estimate: string | null; // wei string
  routeHash: string | null;
  calldataHash: string | null;
  policyId: string | null;
  readiness_green: boolean;
  kill_switch_off: boolean;
  live_gate_open: false; // structural — never true from this surface
}

/** The result a real fork-simulation runtime would return (injected via deps). */
export interface SimResult {
  passed: boolean;
  calldataHash: string;
  routeHash: string;
  netProfitUsd: number;
  riskScore: number;
  gasEstimate: string; // wei
}

/** Deny-by-default verdict: allow only if every gate is green. */
export function simulateVerdict(g: SimulateGates): { allow: boolean; denied: string[] } {
  const checks: Array<[string, boolean]> = [
    ["simulation_passed", g.simulation_passed === true],
    ["calldata_hash_matches_sim", g.calldata_hash_matches_sim === true],
    ["net_profit_usd_positive", (g.net_profit_usd ?? 0) > 0],
    ["risk_score_minimum", g.risk_score !== null && g.risk_score >= RISK_SCORE_MIN],
    ["readiness_green", g.readiness_green === true],
    ["kill_switch_off", g.kill_switch_off === true],
    ["live_gate_open", g.live_gate_open === true],
  ];
  const denied = checks.filter(([, ok]) => !ok).map(([n]) => n);
  return { allow: denied.length === 0, denied };
}

interface SimulateInput extends IntentInput {
  routeHash?: string | undefined;
  calldataHash?: string | undefined;
  policyId?: string | undefined;
}

function readSimulateInput(body: unknown): SimulateInput {
  const base = readIntent(body);
  if (!body || typeof body !== "object") return base;
  const b = body as Record<string, unknown>;
  const str = (v: unknown): string | undefined => (typeof v === "string" ? v : undefined);
  return { ...base, routeHash: str(b.routeHash), calldataHash: str(b.calldataHash), policyId: str(b.policyId) };
}

/** Route deps. The runtime providers are OPTIONAL — absent → fail-closed (deny). */
export interface WalletRoutesDeps {
  logger: { warn: (obj: object, msg?: string) => void; info?: (obj: object, msg?: string) => void };
  readiness?: () => Promise<{ green: boolean }>;
  killSwitch?: () => Promise<{ off: boolean }>;
  forkSimulator?: (input: SimulateInput) => Promise<SimResult | null>;
}

// ---------------------------------------------------------------------------
// Route mounting.
// ---------------------------------------------------------------------------

export function mountWalletRoutes(app: Application, deps: WalletRoutesDeps): void {
  // ----- GET /api/wallet/status -------------------------------------------
  app.get("/api/wallet/status", (_req: Request, res: Response) => {
    res.status(200).json({
      status: "ok",
      mode: "paper",
      read_only: true,
      runtime: "wallet_identity_only",
      ...SAFE_POSTURE,
      broadcast_allowed: false,
      simulation_required: true,
      message: "Wallet surface is read-only/paper. No signer, no capital, no broadcast.",
      ts: new Date().toISOString(),
    });
  });

  // ----- GET /api/wallet/safety -------------------------------------------
  app.get("/api/wallet/safety", (_req: Request, res: Response) => {
    res.status(200).json({
      status: "ok",
      ...SAFE_POSTURE,
      broadcast_allowed: false,
      submit_enabled: false,
      private_relay_enabled: false,
      blind_signing_enabled: false,
      simulation_required: true,
      session_mode: "wallet_identity_only",
      gates: safetyGates(),
      ts: new Date().toISOString(),
    });
  });

  // ----- POST /api/wallet/simulate ----------------------------------------
  // Simulation is the ONLY pre-flight; it can pass or fail but NEVER unlocks a
  // broadcast. Deny-by-default: `allow` needs every gate green, and live_gate_open
  // is structurally false, so allow is always false here. Without a fork-sim
  // runtime we are fail-closed (reason=runtime_not_configured) — never a fabricated
  // "passed". Runtime providers (forkSimulator/readiness/killSwitch) are optional;
  // absent or throwing → the corresponding gate is denied.
  app.post("/api/wallet/simulate", async (req: Request, res: Response) => {
    const input = readSimulateInput(req.body);
    const id = intentId();
    deps.logger.info?.({ event: "wallet.simulate", id, kind: input.kind ?? null, chain_id: input.chain_id ?? null });

    let sim: SimResult | null = null;
    if (deps.forkSimulator) {
      try {
        sim = await deps.forkSimulator(input);
      } catch {
        sim = null; // fail-closed on simulator error
      }
    }

    let readiness_green = false;
    if (deps.readiness) {
      try {
        readiness_green = (await deps.readiness()).green === true;
      } catch {
        readiness_green = false;
      }
    }

    let kill_switch_off = false;
    if (deps.killSwitch) {
      try {
        kill_switch_off = (await deps.killSwitch()).off === true;
      } catch {
        kill_switch_off = false;
      }
    }

    const gates: SimulateGates = {
      simulation_passed: sim?.passed === true,
      calldata_hash_matches_sim:
        !!sim && !!input.calldataHash && sim.calldataHash.toLowerCase() === input.calldataHash.toLowerCase(),
      net_profit_usd: sim ? sim.netProfitUsd : null,
      risk_score: sim ? sim.riskScore : null,
      gas_estimate: sim ? sim.gasEstimate : null,
      routeHash: sim ? sim.routeHash : null,
      calldataHash: sim ? sim.calldataHash : null,
      policyId: input.policyId ?? null,
      readiness_green,
      kill_switch_off,
      live_gate_open: false,
    };

    const { allow, denied } = simulateVerdict(gates);
    const reason = sim ? (allow ? null : "policy_denied") : "runtime_not_configured";

    res.status(200).json({
      status: sim ? "ok" : "unavailable",
      reason,
      intent_id: id,
      state: (allow ? "SIMULATION_PASSED" : "SIMULATION_REQUIRED") as IntentState,
      simulated: sim !== null,
      ...gates,
      allow, // deny-by-default; always false from this surface (live_gate_open structural false)
      denied,
      ...SAFE_POSTURE,
      broadcast_allowed: false,
      broadcast_reason: BROADCAST_DISABLED_REASON,
      message: sim
        ? "Simulation ran. A passing simulation does NOT enable broadcast — live_gate_open is structurally false."
        : "No fork-simulation runtime is wired. Even a passing simulation would NOT enable broadcast.",
      ts: new Date().toISOString(),
    });
  });

  // ----- POST /api/wallet/intent ------------------------------------------
  // Registers an intent and runs it through the safe state machine. It ALWAYS
  // terminates at BROADCAST_DISABLED. An intent that has not been simulated is
  // SIMULATION_REQUIRED first; either way broadcast_allowed is false.
  app.post("/api/wallet/intent", (req: Request, res: Response) => {
    const intent = readIntent(req.body);
    const id = intentId();

    // State derivation — deterministic, safe. `simulated` may be claimed in the
    // body but we DO NOT trust it to enable anything; it only affects the
    // pre-terminal label shown to the operator. The terminal state is fixed.
    const claimedSimulated =
      typeof (req.body as { simulated?: unknown })?.simulated === "boolean"
        ? Boolean((req.body as { simulated?: boolean }).simulated)
        : false;

    const preTerminal: IntentState = claimedSimulated ? "BLOCKED_BY_LIVE_OFF" : "SIMULATION_REQUIRED";

    deps.logger.info?.({
      event: "wallet.intent",
      id,
      kind: intent.kind ?? null,
      chain_id: intent.chain_id ?? null,
      pre_terminal: preTerminal,
    });

    res.status(200).json({
      status: "ok",
      intent_id: id,
      // The lifecycle the intent traversed; it ALWAYS ends at BROADCAST_DISABLED.
      lifecycle: ["DRAFT", preTerminal, TERMINAL_STATE] as IntentState[],
      state: TERMINAL_STATE as IntentState,
      ...SAFE_POSTURE,
      broadcast_allowed: false,
      reason: BROADCAST_DISABLED_REASON,
      simulation_required: true,
      operator_approval_required: true,
      echo: {
        chain_id: intent.chain_id ?? null,
        kind: intent.kind ?? null,
        // Addresses echoed for the operator's review only; never acted upon.
        to: intent.to ?? null,
        note: intent.note ?? null,
      },
      message:
        "Intent recorded for review. Broadcast is disabled by policy. " +
        "There is no code path from this surface to a network send.",
      ts: new Date().toISOString(),
    });
  });
}

// Test-only exports.
export const __forTesting = {
  SAFE_POSTURE,
  BROADCAST_DISABLED_REASON,
  TERMINAL_STATE,
  INTENT_STATES,
  safetyGates,
  readIntent,
  simulateVerdict,
  readSimulateInput,
  RISK_SCORE_MIN,
};
