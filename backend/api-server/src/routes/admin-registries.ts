/**
 * =============================================================================
 * OMEGA admin-registries — mounts the generic Registry Engine for 6 entities
 * =============================================================================
 *
 * Wires the canonical 7-Layer fabric for:
 *   - rpcs            /rpcs        (chain-scoped)
 *   - contracts       /contracts   (chain-scoped)
 *   - risk-gates      /risk-gates  (global)
 *   - capital-gates   /capital-gates (global)
 *   - relays          /relays      (chain-scoped)
 *   - agents          /agents      (global)
 *
 * Chains/dexes/pools/tokens/wallets/strategies retain their pre-existing
 * routes (admin-chains.ts etc.) — they are already 7-Layer compliant.
 *
 * @module routes/admin-registries
 */

import { Router, type Request } from "express";
import type { Pool } from "pg";
import type { Redis } from "ioredis";
import {
  buildRegistryRouter,
  ALL_DESCRIPTORS,
  type RegistryDeps,
} from "../lib/registry-engine.js";

export function mountAdminRegistries(db: Pool, redis: Redis): Router {
  const router = Router();
  const deps: RegistryDeps = {
    db,
    redis,
    actorOf: (req: Request) => {
      const actor = (req.header("x-omega-actor") as string) ?? (req.ip ?? "anonymous");
      return actor;
    },
  };
  for (const desc of ALL_DESCRIPTORS) buildRegistryRouter(desc, deps, router);
  return router;
}
