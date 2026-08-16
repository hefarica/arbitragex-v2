/**
 * Operator credentials surface — REST API behind /admin/credentials and
 * /api/v1/credentials.
 *
 *   GET  /api/v1/credentials          list (masked)
 *   POST /admin/credentials/test      run validator without persisting
 *   PUT  /admin/credentials           upsert (runs validator + persists)
 *   DELETE /admin/credentials/:provider/:scope
 *
 * Auth:
 *   - GET list is admin-protected (token in cookie or header).
 *   - All mutations require admin token.
 *   - The raw secret_value is NEVER returned. List/get return last-4 suffix.
 */

import { Router, type Request, type Response } from "express";
import type { Pool } from "pg";
import {
  CredentialTestSchema,
  CredentialUpsertSchema,
  CredentialProvider,
} from "@arbx/shared";
import { runValidator } from "../credentials/validators.js";
import {
  listCredentials,
  upsertCredential,
  deleteCredential,
} from "../credentials/store.js";

interface Deps {
  pool: Pool | null;
  requireAdminToken: (token: string) => (req: Request, res: Response, next: () => void) => void;
  adminToken: string;
  logger: { warn: (obj: object, msg?: string) => void; info: (obj: object, msg?: string) => void };
}

/**
 * MC-RPC-1: reduce the rpc_http/rpc_ws validator breakdown to a maskable
 * {name, ok, detail} list. The raw `details.providers` rows carry the full
 * provider URL — which embeds API keys — and must NEVER be persisted or
 * returned by the masked list endpoint.
 */
function sanitizeProviderBreakdown(
  details: Record<string, unknown> | undefined,
): Array<{ name: string; ok: boolean; detail: string }> | undefined {
  const providers = details?.["providers"];
  if (!Array.isArray(providers)) return undefined;
  const rows = providers
    .filter((p): p is Record<string, unknown> => typeof p === "object" && p !== null)
    .map((p) => ({
      name: typeof p["name"] === "string" ? p["name"] : "?",
      ok: p["ok"] === true,
      detail: typeof p["detail"] === "string" ? p["detail"].slice(0, 80) : "",
    }));
  return rows.length > 0 ? rows : undefined;
}

export function buildCredentialsRouter(deps: Deps): Router {
  const r = Router();

  // ── GET list (admin) ──────────────────────────────────────────────────
  r.get("/api/v1/credentials", deps.requireAdminToken(deps.adminToken), async (_req, res) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    try {
      const items = await listCredentials(deps.pool);
      res.status(200).json({ items, ts: new Date().toISOString() });
    } catch (e) {
      deps.logger.warn({ event: "credentials.list_failed", err: (e as Error).message });
      res.status(500).json({ error: "db_error", detail: (e as Error).message });
    }
  });

  // ── GET summary (counts only — powers the sidebar "needs attention" badge) ──
  // Returns ONLY aggregate counts (no provider, scope, value or metadata), so it
  // is intentionally NOT admin-gated; the detailed list above stays gated.
  // needs_attention = invalid + untested rows. NOTE: reflects rows in the store
  // (tracked credentials), not the frontend's expected catalog — a fresh/empty
  // store reports 0.
  r.get("/api/v1/credentials/summary", async (_req, res) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    try {
      const items = await listCredentials(deps.pool);
      let valid = 0;
      let invalid = 0;
      let untested = 0;
      for (const it of items) {
        if (it.status === "valid") valid += 1;
        else if (it.status === "invalid") invalid += 1;
        else untested += 1;
      }
      res.status(200).json({
        total: items.length,
        valid,
        invalid,
        untested,
        needs_attention: invalid + untested,
        generated_at: new Date().toISOString(),
      });
    } catch (e) {
      deps.logger.warn({ event: "credentials.summary_failed", err: (e as Error).message });
      res.status(500).json({ error: "db_error", detail: (e as Error).message });
    }
  });

  // ── POST test (run validator without persisting) ──────────────────────
  r.post("/admin/credentials/test", deps.requireAdminToken(deps.adminToken), async (req, res) => {
    const parsed = CredentialTestSchema.safeParse(req.body);
    if (!parsed.success) {
      res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
      return;
    }
    const { provider, scope, secret_value, metadata } = parsed.data;
    try {
      const result = await runValidator(provider, scope, secret_value, metadata);
      deps.logger.info({
        event: "credentials.test",
        provider,
        scope,
        status: result.status,
      });
      res.status(200).json(result);
    } catch (e) {
      res.status(500).json({ error: "validator_error", detail: (e as Error).message });
    }
  });

  // ── PUT upsert (runs validator + persists) ────────────────────────────
  r.put("/admin/credentials", deps.requireAdminToken(deps.adminToken), async (req, res) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    const parsed = CredentialUpsertSchema.safeParse(req.body);
    if (!parsed.success) {
      res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
      return;
    }
    const { provider, scope, display_name, secret_value, metadata } = parsed.data;
    const actor = (req.header("x-arbx-actor") ?? "admin").slice(0, 64);

    let validationStatus: "valid" | "invalid" | "untested" = "untested";
    let validationError: string | null = null;
    let validationProviders: Array<{ name: string; ok: boolean; detail: string }> | undefined;

    if (secret_value && secret_value.length > 0) {
      try {
        const test = await runValidator(provider, scope, secret_value, metadata);
        validationStatus = test.status === "valid" ? "valid" : "invalid";
        validationError = test.status === "valid" ? null : test.message;
        validationProviders = sanitizeProviderBreakdown(test.details);
      } catch (e) {
        validationStatus = "invalid";
        validationError = `validator_threw: ${(e as Error).message.slice(0, 200)}`;
      }
    }

    try {
      const row = await upsertCredential(deps.pool, {
        provider,
        scope,
        display_name: display_name,
        secret_value,
        metadata: {
          ...metadata,
          // MC-RPC-1: persist the sanitized per-provider fallback-pool
          // breakdown so the UI renders every provider's state without a
          // live re-test. URLs (API keys) are stripped server-side — the
          // masked-list contract must hold for metadata too.
          ...(validationProviders
            ? { _validation: { message: validationError, providers: validationProviders } }
            : {}),
        },
        status: validationStatus,
        validation_error: validationError,
        actor,
      });
      deps.logger.info({
        event: "credentials.upsert",
        provider,
        scope,
        status: validationStatus,
        actor,
      });
      res.status(200).json(row);
    } catch (e) {
      deps.logger.warn({ event: "credentials.upsert_failed", err: (e as Error).message });
      res.status(500).json({ error: "db_error", detail: (e as Error).message });
    }
  });

  // ── DELETE ─────────────────────────────────────────────────────────────
  r.delete(
    "/admin/credentials/:provider/:scope",
    deps.requireAdminToken(deps.adminToken),
    async (req, res) => {
      if (!deps.pool) {
        res.status(503).json({ error: "db_unavailable" });
        return;
      }
      const provider = CredentialProvider.safeParse(req.params.provider);
      if (!provider.success) {
        res.status(400).json({ error: "invalid_provider" });
        return;
      }
      const scope = req.params.scope ?? "";
      if (!/^(global|chain:\d+)$/.test(scope)) {
        res.status(400).json({ error: "invalid_scope" });
        return;
      }
      try {
        const removed = await deleteCredential(deps.pool, provider.data, scope);
        const actor = (req.header("x-arbx-actor") ?? "admin").slice(0, 64);
        deps.logger.info({
          event: "credentials.delete",
          provider: provider.data,
          scope,
          removed,
          actor,
        });
        res.status(200).json({ ok: true, removed });
      } catch (e) {
        res.status(500).json({ error: "db_error", detail: (e as Error).message });
      }
    },
  );

  return r;
}
