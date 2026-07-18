import type { Application, Request, Response } from "express";
import type { Redis } from "ioredis";
import { resolvePaperModeState } from "../readiness/paper-mode-state.js";

export interface MountPaperModeStateDeps {
  redis: Redis | null;
  env: NodeJS.ProcessEnv;
  enabledChainIds: number[];
  logger: { warn: (obj: object, msg?: string) => void };
}

export function mountPaperModeState(app: Application, deps: MountPaperModeStateDeps): void {
  app.get("/api/paper-mode/state", async (req: Request, res: Response) => {
    const chainIdParam = req.query["chain_id"];
    let chainId: number | undefined;
    if (chainIdParam !== undefined) {
      const parsed = Number(chainIdParam);
      if (!Number.isFinite(parsed) || parsed < 1 || !Number.isInteger(parsed)) {
        res.status(400).json({ error: "invalid_chain_id", detail: "chain_id must be a positive integer" });
        return;
      }
      chainId = parsed;
    }

    try {
      const state = await resolvePaperModeState({
        redis: deps.redis,
        env: deps.env,
        enabledChainIds: deps.enabledChainIds,
        chainId: chainId ?? null,
        logger: deps.logger,
      });

      res.setHeader("Cache-Control", "no-store");
      res.setHeader("Pragma", "no-cache");
      res.status(200).json(state);
    } catch (e) {
      deps.logger.warn({ event: "paper_mode_state.failed", err: (e as Error).message });
      res.status(503).json({ error: "paper_mode_state_failed", detail: (e as Error).message });
    }
  });
}
