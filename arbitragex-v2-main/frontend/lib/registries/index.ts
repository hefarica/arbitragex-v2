/**
 * =============================================================================
 * OMEGA Entity Registry — Export Barrel
 * =============================================================================
 *
 * Import from this module to access all canonical types and validation schemas.
 *
 * @example
 * ```typescript
 * import { Chain, ChainSchema, validateEntity } from "@/lib/registries";
 *
 * const raw = await fetchChain(42161);
 * const chain: Chain = validateEntity("chain", raw);
 * ```
 *
 * @module registries
 */

// --- Canonical TypeScript interfaces (zero-any, strict) ---
export * from "./types";

// --- Zod runtime-validation schemas (mirrors every interface above) ---
export * from "./schemas";
