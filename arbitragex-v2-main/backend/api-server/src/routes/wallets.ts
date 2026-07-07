/**
 * Operational Wallets endpoints — `/wallets` page consumer.
 *
 * Three routes, all R8 fail-honest (empty data when not yet seeded):
 *
 *   GET /api/v1/wallets
 *     Lists operational wallets registered in the `wallets` table
 *     (migration 011 / pre-existing schema). The table can be empty —
 *     in paper-trade mode the operator has no real signers, and the
 *     page renders its "No wallets returned by the API" empty state.
 *     The dual signer-ish entry from env (SIM_SIGNER_ADDRESS) is NOT
 *     surfaced here because that address is the dev sentinel
 *     (`0x1234…7890`) — surfacing it as a wallet would imply real
 *     capital sits behind it, which is the opposite of fail-honest.
 *     Operator seeds rows via PSQL or future admin endpoint.
 *
 *   GET /api/v1/wallets/:address/balances
 *     Reads cached balances from on-chain queries. NOT yet wired to a
 *     live source (no balance worker exists). Returns `{ address,
 *     balances: [] }` with a `pending_source` hint so the dashboard
 *     surfaces the gap honestly rather than 404-ing.
 *
 *   GET /api/v1/wallets/:address/allowances
 *     Reads from `wallet_allowances` JOINed with `tokens` for symbol
 *     resolution. Currently empty (operator has not seeded). Returns
 *     `{ address, allowances: [] }` for unknown / unseeded addresses.
 */

import type { Request, Response } from "express";
import type { Pool } from "pg";

interface Deps {
  pool: Pool | null;
  logger: { warn: (obj: object, msg?: string) => void };
}

// Address validation — match the DB CHECK constraint (lowercase hex 0x... 42 chars).
const ADDR_RE = /^0x[0-9a-fA-F]{40}$/;

export function mountWallets(app: import("express").Express, deps: Deps): void {
  // ── GET /api/v1/wallets ───────────────────────────────────────────────
  app.get("/api/v1/wallets", async (_req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    try {
      const q = await deps.pool.query<{
        address: string;
        label: string | null;
        is_active: boolean;
      }>(
        `SELECT address, label, is_active
           FROM wallets
          WHERE is_active = TRUE
          ORDER BY label NULLS LAST, address`,
      );
      res.status(200).json({
        wallets: q.rows.map((r) => ({
          address: r.address.toLowerCase(),
          label:   r.label ?? r.address,
          // No `role` column in the schema yet — surface null per R8 rather
          // than inventing a role from `label`. Future migration adds it.
          role:    null as string | null,
        })),
      });
    } catch (e) {
      deps.logger.warn({ event: "wallets.list_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });

  // ── GET /api/v1/wallets/:address/balances ─────────────────────────────
  // Honest empty response. Once a balance worker (per-chain ERC-20 + native
  // balanceOf via the alloy RPC pool) lands, this becomes a SELECT from a
  // `wallet_balances_cache` table. For now we emit an explicit gap hint
  // alongside the empty array so the dashboard surfaces "no balance data"
  // rather than hiding the missing source.
  app.get("/api/v1/wallets/:address/balances", async (req: Request, res: Response) => {
    const address = String(req.params["address"] ?? "").toLowerCase();
    if (!ADDR_RE.test(address)) {
      res.status(400).json({ error: "invalid_address" });
      return;
    }
    res.status(200).json({
      address,
      balances: [],
      // R8 fail-honest: explicit pending hint so the WalletDetailDialog
      // doesn't render "empty" as "verified zero balance".
      data_source: "pending",
      pending_reason:
        "balance worker not yet wired — operator can spot-check via etherscan.io until a live source lands",
    });
  });

  // ── GET /api/v1/wallets/:address/allowances ────────────────────────────
  app.get("/api/v1/wallets/:address/allowances", async (req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    const address = String(req.params["address"] ?? "").toLowerCase();
    if (!ADDR_RE.test(address)) {
      res.status(400).json({ error: "invalid_address" });
      return;
    }
    try {
      const q = await deps.pool.query<{
        spender_address: string;
        amount: string;
        token_address: string | null;
        token_symbol: string | null;
        token_decimals: number | null;
      }>(
        `SELECT wa.spender_address,
                wa.amount::text         AS amount,
                t.address               AS token_address,
                t.symbol                AS token_symbol,
                t.decimals              AS token_decimals
           FROM wallet_allowances wa
           JOIN wallets w        ON w.id  = wa.wallet_id
           LEFT JOIN tokens  t   ON t.id  = wa.token_id
          WHERE LOWER(w.address) = $1
          ORDER BY t.symbol NULLS LAST, wa.spender_address`,
        [address],
      );
      // The DB stores `amount` as a NUMERIC string. Treat 2^256-1 (canonical
      // max-uint256) as "unlimited" for UI flag — exact-string compare avoids
      // float precision drift.
      const UINT256_MAX = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
      res.status(200).json({
        address,
        allowances: q.rows.map((r) => {
          const isUnlimited = r.amount === UINT256_MAX;
          let formatted: string | null = null;
          if (r.token_decimals != null) {
            // Best-effort human-readable; full-precision compare still uses raw.
            try {
              const big = BigInt(r.amount);
              const base = BigInt(10) ** BigInt(r.token_decimals);
              const whole = big / base;
              const frac = big - whole * base;
              const fracStr = frac
                .toString()
                .padStart(r.token_decimals, "0")
                .replace(/0+$/, "")
                .slice(0, 6);
              formatted = fracStr ? `${whole}.${fracStr}` : `${whole}`;
            } catch {
              formatted = null;
            }
          }
          return {
            token_address:        r.token_address ?? "",
            token_symbol:         r.token_symbol,
            spender_address:      r.spender_address,
            spender_label:        null as string | null, // not in schema yet
            allowance_raw:        r.amount,
            allowance_formatted:  formatted,
            is_unlimited:         isUnlimited,
          };
        }),
      });
    } catch (e) {
      deps.logger.warn({ event: "wallets.allowances_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });
}
