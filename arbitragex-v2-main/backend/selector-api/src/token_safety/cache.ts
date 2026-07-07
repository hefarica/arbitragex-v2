/**
 * Token safety cache — Postgres-backed TTL cache over `token_safety_cache` table.
 *
 * Contract:
 *   PK: (chain_id, token_address).
 *   `safety_score`: 0..100 integer.
 *   `flags`: JSONB freeform.
 *   `source`: "goplus" | "honeypot_is" | "internal" | "unknown".
 *   `ttl_expires_at`: absolute expiry (TIMESTAMPTZ).
 *
 * Normalize addresses to lowercase hex at the boundary.
 */

import type pg from "pg";

export type SafetySource = "goplus" | "honeypot_is" | "internal" | "unknown";

export type TokenSafetyRecord = {
  chain_id: number;
  token_address: string;          // lowercase hex
  safety_score: number;           // 0..100
  flags: Record<string, unknown>;
  source: SafetySource;
  ttl_expires_at: Date;
  updated_at: Date;
};

export function normalizeAddress(addr: string): string {
  const s = addr.trim().toLowerCase();
  if (!/^0x[0-9a-f]{40}$/.test(s)) {
    throw new Error(`invalid eth address: ${addr}`);
  }
  return s;
}

export async function getCached(
  pool: pg.Pool, chainId: number, addr: string, now: Date = new Date(),
): Promise<TokenSafetyRecord | null> {
  const a = normalizeAddress(addr);
  const r = await pool.query(
    `SELECT chain_id, token_address, safety_score, flags, source, ttl_expires_at, updated_at
       FROM token_safety_cache
      WHERE chain_id = $1 AND token_address = $2 AND ttl_expires_at > $3
      LIMIT 1`,
    [chainId, a, now],
  );
  if (r.rowCount === 0) return null;
  const row = r.rows[0]!;
  return {
    chain_id: row.chain_id,
    token_address: row.token_address,
    safety_score: row.safety_score,
    flags: row.flags ?? {},
    source: row.source as SafetySource,
    ttl_expires_at: row.ttl_expires_at,
    updated_at: row.updated_at,
  };
}

export async function upsertCached(
  pool: pg.Pool, rec: Omit<TokenSafetyRecord, "updated_at">,
): Promise<void> {
  const a = normalizeAddress(rec.token_address);
  await pool.query(
    `INSERT INTO token_safety_cache
       (chain_id, token_address, safety_score, flags, source, ttl_expires_at, updated_at)
     VALUES ($1, $2, $3, $4::jsonb, $5, $6, NOW())
     ON CONFLICT (chain_id, token_address) DO UPDATE
       SET safety_score = EXCLUDED.safety_score,
           flags        = EXCLUDED.flags,
           source       = EXCLUDED.source,
           ttl_expires_at = EXCLUDED.ttl_expires_at,
           updated_at   = NOW()`,
    [rec.chain_id, a, rec.safety_score, JSON.stringify(rec.flags), rec.source, rec.ttl_expires_at],
  );
}
