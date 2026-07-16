/**
 * live-testnet — operator-facing testnet controls (read-only + config staging).
 *
 * Replaces the hardcoded Rust stub at `routes/live_testnet.rs` (G-SIM-1 cleanup
 * pending — this TS route is the real implementation; the .rs file is dead code).
 *
 * Invariants:
 *   - This surface NEVER flips `paper_mode`. That knob lives in
 *     `configs/app.toml` and requires explicit operator edit + redeploy.
 *   - This surface NEVER enables broadcast/submit. It only validates that the
 *     requested chain is in the testnet allowlist and reports readiness blockers.
 *   - Mainnet (chain_id 1) is rejected structurally.
 *   - All productive values (allowlist, upstream URLs) come from env/config.
 *
 * Endpoints:
 *   GET  /api/v1/live-testnet/config
 *   POST /admin/config/live-testnet   (admin token required)
 *   GET  /api/live-testnet/events     (public SSE telemetry stream)
 */

import type { Application, Request, Response } from "express";
import { z } from "zod";

interface Deps {
  logger: { warn: (obj: object, msg?: string) => void };
  requireAdminToken: (token: string) => (req: Request, res: Response, next: () => void) => void;
  adminToken: string;
  readiness: () => Promise<{ flip_blocked: boolean; blockers?: Array<{ id: string; title: string; severity: string }> }>;
}

// TESTNET_CHAIN_IDS is the canonical allowlist. Populate from a typed env var
// so the operator controls which testnets are exposed without code changes.
// Format: comma-separated decimal chain IDs, e.g. "11155111,421614,11155420".
function parseTestnetChainIds(): number[] {
  const raw = process.env["LIVE_TESTNET_CHAIN_IDS"] ?? "11155111,421614,11155420";
  const ids = raw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => Number(s))
    .filter((n) => Number.isFinite(n) && n > 1 && Number.isInteger(n));
  // Deduplicate preserving order.
  return Array.from(new Set(ids));
}

const TESTNET_CHAIN_IDS = parseTestnetChainIds();

const PostConfigSchema = z.object({
  enabled: z.boolean(),
  chain_id: z.number().int().positive(),
});

interface LiveTestnetConfig {
  mode: "LIVE_TESTNET";
  enabled: boolean;
  chain_id: number;
  allowed_chain_ids: number[];
  mainnet_blocked: boolean;
  can_execute: false;
  paper_mode: true;
  blockers: string[];
  generated_at: string;
}

async function buildConfig(enabled: boolean, chainId: number, readiness: Deps["readiness"]): Promise<LiveTestnetConfig> {
  const allowed = TESTNET_CHAIN_IDS;
  let blockers: string[] = [];
  try {
    const r = await readiness();
    if (r.flip_blocked && Array.isArray(r.blockers)) {
      blockers = r.blockers.filter((b) => b.severity === "critical").map((b) => b.title).slice(0, 8);
    }
  } catch (e) {
    blockers = ["readiness_decision_unreachable"];
  }
  return {
    mode: "LIVE_TESTNET",
    enabled,
    chain_id: chainId,
    allowed_chain_ids: allowed,
    mainnet_blocked: true,
    can_execute: false,
    paper_mode: true,
    blockers,
    generated_at: new Date().toISOString(),
  };
}

function sendSse(res: Response, event: string, data: unknown): void {
  res.write(`event: ${event}\n`);
  res.write(`data: ${JSON.stringify(data)}\n\n`);
}

export function mountLiveTestnet(app: Application, deps: Deps): void {
  app.get("/api/v1/live-testnet/config", async (_req: Request, res: Response) => {
    const cfg = await buildConfig(true, TESTNET_CHAIN_IDS[0] ?? 11155111, deps.readiness);
    res.status(200).json(cfg);
  });

  // Public SSE telemetry stream. Emits config snapshot + periodic pings.
  // No admin token required — all data is read-only and non-secret.
  app.get("/api/live-testnet/events", async (req: Request, res: Response) => {
    const requestedChainId = Number(req.query["chain_id"]);
    const chainId = Number.isFinite(requestedChainId) && requestedChainId > 1 ? requestedChainId : (TESTNET_CHAIN_IDS[0] ?? 11155111);

    res.setHeader("Content-Type", "text/event-stream");
    res.setHeader("Cache-Control", "no-cache");
    res.setHeader("Connection", "keep-alive");
    res.setHeader("X-Accel-Buffering", "no");
    res.flushHeaders();

    const cfg = await buildConfig(true, chainId, deps.readiness);
    sendSse(res, "connected", { mode: cfg.mode, chain_id: cfg.chain_id, ts: Date.now() });

    const intervalMs = Number(process.env["LIVE_TESTNET_SSE_INTERVAL_MS"] ?? 5000);
    const intervalHandle = setInterval(() => {
      sendSse(res, "ping", { type: "ping", chain_id: chainId, ts: Date.now() });
    }, intervalMs);

    req.on("close", () => {
      clearInterval(intervalHandle);
      res.end();
    });

    req.on("error", () => {
      clearInterval(intervalHandle);
      res.end();
    });
  });

  app.post(
    "/admin/config/live-testnet",
    deps.requireAdminToken(deps.adminToken),
    async (req: Request, res: Response) => {
      const parsed = PostConfigSchema.safeParse(req.body);
      if (!parsed.success) {
        res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
        return;
      }

      const { enabled, chain_id } = parsed.data;

      if (chain_id === 1) {
        res.status(403).json({ error: "MAINNET_BLOCKED" });
        return;
      }

      if (!TESTNET_CHAIN_IDS.includes(chain_id)) {
        res.status(400).json({
          error: "UNSUPPORTED_CHAIN",
          chain_id,
          allowed_chain_ids: TESTNET_CHAIN_IDS,
        });
        return;
      }

      const cfg = await buildConfig(enabled, chain_id, deps.readiness);
      res.status(200).json(cfg);
    },
  );
}
