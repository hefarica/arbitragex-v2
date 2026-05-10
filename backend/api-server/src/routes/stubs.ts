/**
 * A8 — Frontend/API endpoint stubs (audit 2026-05-10).
 *
 * The frontend was calling several endpoints that did not exist on the
 * api-server, producing generic Express 404s in production. That is operator-
 * hostile: a 404 from a missing route looks identical to a 404 from a typo
 * or from a real "no such resource" condition.
 *
 * This module mounts proper stubs that return:
 *   - 501 Not Implemented for endpoints whose backing logic requires an
 *     operator strategy decision (wallet sourcing, service control plane,
 *     alertmanager wiring, sim-ctl integration).
 *   - structured JSON: { error, message, feature, roadmap }.
 *
 * R8 fail-honest: never fabricates a fake response body. The stubs never
 * synthesize data; they declare honestly that the feature is scaffolded
 * but not wired.
 *
 * Endpoints reconciled against the audit 2026-05-10 frontend ↔ API matrix:
 *   - GET   /api/v1/wallets                          → 501 (FE-2 wallets list)
 *   - GET   /api/v1/wallets/:addr/balances           → 501 (FE-2 balances)
 *   - GET   /api/v1/wallets/:addr/allowances         → 501 (FE-2 allowances)
 *   - POST  /api/v1/opportunities/:id/simulate       → 501 (sim-ctl wiring)
 *   - POST  /api/v1/admin/services/:name/start       → 501 (control-plane TBD)
 *   - POST  /api/v1/admin/services/:name/stop        → 501 (control-plane TBD)
 *   - POST  /admin/alertmanager/webhook              → 501 (alertmanager handler)
 *   - PUT   /api/v1/dexes/:id/active                 → 501 (DEX toggle persistence)
 *
 * NOTE: real implementations should replace the stub by deleting the matching
 * route here and registering the canonical handler elsewhere. Express dispatches
 * the FIRST matching route, so removing the stub is sufficient.
 */

import type { Express, Request, Response, RequestHandler } from "express";

interface StubDeps {
  /** Admin-token middleware factory from @arbx/shared. */
  requireAdminToken: (expected: string) => RequestHandler;
  /** Resolved admin token (do NOT log). */
  adminToken: string;
}

interface StubBody {
  error: "not_implemented";
  message: string;
  feature: string;
  roadmap: string;
}

/** Builds a uniform 501 handler for read-only stub endpoints. */
function notImplemented(body: Omit<StubBody, "error">): RequestHandler {
  return (_req: Request, res: Response) => {
    const payload: StubBody = { error: "not_implemented", ...body };
    res.status(501).type("application/json").json(payload);
  };
}

export function mountStubs(app: Express, deps: StubDeps): void {
  const { requireAdminToken, adminToken } = deps;

  // ── FE-2 Wallets ─────────────────────────────────────────────────────────
  // Operator must decide wallet sourcing strategy (multisig vs hot signer
  // vs Vault-backed key derivation) before this can return real data.
  app.get(
    "/api/v1/wallets",
    notImplemented({
      message:
        "GET /api/v1/wallets is scaffolded but the operator must wire wallet sourcing logic",
      feature: "FE-2 wallets page",
      roadmap:
        "Sprint 4+ — operator decides wallet management strategy (multisig vs hot signer vs Vault)",
    }),
  );

  app.get(
    "/api/v1/wallets/:addr/balances",
    notImplemented({
      message:
        "GET /api/v1/wallets/:addr/balances is scaffolded; balance enrichment via on-chain provider not wired",
      feature: "FE-2 wallet balances",
      roadmap:
        "Sprint 4+ — depends on wallet sourcing + RPC provider selection (alloy vs ethers-rs)",
    }),
  );

  app.get(
    "/api/v1/wallets/:addr/allowances",
    notImplemented({
      message:
        "GET /api/v1/wallets/:addr/allowances is scaffolded; ERC-20 allowance scanner not wired",
      feature: "FE-2 wallet allowances",
      roadmap:
        "Sprint 4+ — depends on token universe + per-router approval index",
    }),
  );

  // ── Opportunity simulation (sim-ctl bridge) ──────────────────────────────
  // The api-server currently aggregates upstream services but does not proxy
  // simulation to sim-ctl per opportunity. Real wiring needs a discriminated
  // body schema (Zod) describing the expected SwapStep[] payload.
  app.post(
    "/api/v1/opportunities/:id/simulate",
    notImplemented({
      message:
        "POST /api/v1/opportunities/:id/simulate is scaffolded; sim-ctl bridge not wired",
      feature: "FE simulate-on-demand",
      roadmap:
        "Sprint 4 — REVM bridge through sim-ctl with Bellman-Ford route reconstruction",
    }),
  );

  // ── Service control plane (admin-gated) ──────────────────────────────────
  // start/stop is admin-token gated even as a stub so that probing the
  // surface area does not leak which services exist.
  app.post(
    "/api/v1/admin/services/:name/start",
    requireAdminToken(adminToken),
    notImplemented({
      message:
        "POST /api/v1/admin/services/:name/start is scaffolded; control-plane (docker / systemd) not wired",
      feature: "FE service control panel",
      roadmap:
        "Sprint 5+ — operator decides control-plane (docker socket vs systemd unit vs k8s)",
    }),
  );

  app.post(
    "/api/v1/admin/services/:name/stop",
    requireAdminToken(adminToken),
    notImplemented({
      message:
        "POST /api/v1/admin/services/:name/stop is scaffolded; control-plane (docker / systemd) not wired",
      feature: "FE service control panel",
      roadmap:
        "Sprint 5+ — operator decides control-plane (docker socket vs systemd unit vs k8s)",
    }),
  );

  // ── Alertmanager webhook ─────────────────────────────────────────────────
  // Receives Prometheus Alertmanager POSTs (silenced/firing). Currently no
  // sink (no notifier wiring, no DB persistence). Stub returns 501 so
  // Alertmanager surfaces the misconfiguration in its own UI.
  app.post(
    "/admin/alertmanager/webhook",
    requireAdminToken(adminToken),
    notImplemented({
      message:
        "POST /admin/alertmanager/webhook is scaffolded; Alertmanager → audit/notify pipeline not wired",
      feature: "Observability alert ingestion",
      roadmap:
        "Sprint 3+ — wire notifier (Slack/PagerDuty) + persist to alert_events table",
    }),
  );

  // ── DEX active toggle ────────────────────────────────────────────────────
  // The frontend strategies tab assumes PUT /api/v1/dexes/:id/active to flip
  // a single DEX. The canonical write path is PUT /admin/trading-config/:chain_id
  // (already implemented) which mutates enabled_dex_ids[]. Until the per-DEX
  // shortcut route is implemented, return 501 with explicit pointer.
  app.put(
    "/api/v1/dexes/:id/active",
    requireAdminToken(adminToken),
    notImplemented({
      message:
        "PUT /api/v1/dexes/:id/active is scaffolded; use PUT /admin/trading-config/:chain_id to mutate enabled_dex_ids[]",
      feature: "FE strategies → DEX toggle",
      roadmap:
        "Sprint 2+ — add per-DEX shortcut that delegates to trading_config writer",
    }),
  );
}
