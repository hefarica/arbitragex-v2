// Idea 2 — Strategy Forge injection form types.
// Mirrors backend/api-server/src/routes/cartridge-forge.ts GET /api/v1/cartridges rows.
// Cartridges run in SHADOW eval (admin-gated, no capital): injecting publishes
// arbx:cartridge:injection → searcher-rs CartridgeSubscriber compiles + hot-loads.

export interface CartridgeSummary {
  id: string;
  slug: string;
  name: string;
  version: string;
  author: string;
  description: string;
  category: string;
  target_chains: number[];
  state: string;
  min_eval_interval_ms: number;
  total_evaluations: number;
  total_opportunities: number;
  total_errors: number;
  created_at: string;
  updated_at?: string;
  last_evaluation_at?: string | null;
}

export interface InjectCartridgeInput {
  slug: string;
  source_code: string;
  target_chains: number[];
}

export const SLUG_RE = /^[a-z][a-z0-9_]{2,48}$/;
