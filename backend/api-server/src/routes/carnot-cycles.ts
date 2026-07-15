import type { Request, Response } from "express";
import { CarnotStore } from "../services/carnotStore.js";

interface Deps {
  store: CarnotStore;
  logger: { warn: (obj: object, msg?: string) => void };
}

export function mountCarnotCycles(app: import("express").Express, deps: Deps): void {
  app.get("/api/v1/carnot/cycles", (req: Request, res: Response) => {
    const limit = Math.min(Math.max(Number(req.query["limit"] ?? 50), 1), 200);
    res.json({ ok: true, data: deps.store.recent(limit) });
  });

  app.get("/api/v1/carnot/snapshot", (_req: Request, res: Response) => {
    res.json({ ok: true, data: deps.store.snapshot() });
  });
}
