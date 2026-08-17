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
  CredentialBulkRequestSchema,
  CredentialProvider,
  type CredentialBulkItem,
  type CredentialBulkRowResult,
  type CredentialRowPublic,
  type CredentialStatus,
  type CredentialTestResult,
} from "@arbx/shared";
import { runValidator } from "../credentials/validators.js";
import {
  listCredentials,
  upsertCredential,
  deleteCredential,
  readCredentialForBulk,
  type StoredCredentialRow,
  type UpsertInput,
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

/**
 * MC-RPC-1 shared by PUT and bulk: merge the sanitized per-provider breakdown
 * (name/ok/detail — URLs with keys stripped) into the metadata that gets
 * persisted, under the namespaced `_validation` key.
 */
function buildPersistMetadata(
  itemMetadata: Record<string, unknown>,
  validationProviders: Array<{ name: string; ok: boolean; detail: string }> | undefined,
  validationError: string | null,
): Record<string, unknown> {
  return {
    ...itemMetadata,
    ...(validationProviders
      ? { _validation: { message: validationError, providers: validationProviders } }
      : {}),
  };
}

// ── RunFullSyncCycle FASE 1: bulk row processor (DI for tests) ─────────────

export interface BulkRowContext {
  readStored: (
    provider: CredentialProvider,
    scope: string,
  ) => Promise<StoredCredentialRow | null>;
  validate: typeof runValidator;
  upsert: (input: UpsertInput) => Promise<CredentialRowPublic>;
  logger: Deps["logger"];
}

/** Canonical JSON (sorted keys, recursive) for metadata equality checks. */
function canonicalJson(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(canonicalJson).join(",")}]`;
  if (v !== null && typeof v === "object") {
    const o = v as Record<string, unknown>;
    return `{${Object.keys(o)
      .filter((k) => k !== "_validation") // server-managed — never user-compared
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canonicalJson(o[k])}`)
      .join(",")}}`;
  }
  return JSON.stringify(v) ?? "null";
}

/**
 * Process ONE bulk row through the EXACT manual pipeline (runValidator →
 * upsertCredential). Idempotency: when the stored secret is byte-identical and
 * the metadata (minus `_validation`) is unchanged, the row is reported `noop`
 * and NOTHING is written — updated_at does not rotate on re-runs.
 */
export async function processCredentialBulkRow(
  ctx: BulkRowContext,
  item: CredentialBulkItem,
  opts: { dryRun: boolean; actor: string },
): Promise<CredentialBulkRowResult> {
  const stored = await ctx.readStored(item.provider, item.scope);

  // Effective secret: the item's, else the stored one (metadata-only refresh).
  const secret = item.secret_value ?? stored?.secret ?? null;
  if (!secret) {
    return {
      provider: item.provider,
      scope: item.scope,
      action: "invalid",
      error: "no secret provided and none stored",
    };
  }

  const secretChanged = !stored?.secret || stored.secret !== secret;
  const metaChanged =
    !stored || canonicalJson(item.metadata) !== canonicalJson(stored.metadata);

  // Strict idempotency (apply path only): nothing changed ⇒ no write at all.
  if (!opts.dryRun && stored && !secretChanged && !metaChanged) {
    return { provider: item.provider, scope: item.scope, action: "noop", status: stored.status };
  }

  let test: CredentialTestResult;
  try {
    test = await ctx.validate(item.provider, item.scope, secret, item.metadata);
  } catch (e) {
    test = {
      status: "invalid",
      message: `validator_threw: ${(e as Error).message.slice(0, 200)}`,
      tested_at: new Date().toISOString(),
    };
  }
  const providers = sanitizeProviderBreakdown(test.details);
  const status: CredentialStatus = test.status === "valid" ? "valid" : "invalid";

  if (opts.dryRun) {
    // Homologation pass: report exactly what WOULD happen. No persistence.
    return {
      provider: item.provider,
      scope: item.scope,
      action: status === "valid" ? "validated" : "invalid",
      status,
      message: test.message,
      providers,
    };
  }

  const row = await ctx.upsert({
    provider: item.provider,
    scope: item.scope,
    display_name: item.display_name,
    secret_value: secret,
    metadata: buildPersistMetadata(item.metadata, providers, status === "valid" ? null : test.message),
    status,
    validation_error: status === "valid" ? null : test.message,
    actor: opts.actor,
  });
  ctx.logger.info({
    event: "credentials.bulk_row",
    provider: item.provider,
    scope: item.scope,
    action: "updated",
    status,
    actor: opts.actor,
  });
  return {
    provider: item.provider,
    scope: item.scope,
    action: "updated",
    status: row.status,
    message: status === "valid" ? test.message : (row.last_validation_error ?? test.message),
    providers,
  };
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
        metadata: buildPersistMetadata(metadata, validationProviders, validationError),
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

  // ── POST bulk (RunFullSyncCycle FASE 1 — operator macro entrance) ──────
  // Same validator, same upsert, same persistence contract as the manual PUT:
  // the macro is the manual path run in a batch. One bad row NEVER blocks the
  // rest — results are per-row and fail-honest. dry_run validates everything
  // and persists nothing (homologation pass).
  r.post("/admin/credentials/bulk", deps.requireAdminToken(deps.adminToken), async (req, res) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    const parsed = CredentialBulkRequestSchema.safeParse(req.body);
    if (!parsed.success) {
      res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
      return;
    }
    const { items, dry_run } = parsed.data;
    const actor = (req.header("x-arbx-actor") ?? "operator:macro").slice(0, 64);

    const ctx: BulkRowContext = {
      readStored: (provider, scope) => readCredentialForBulk(deps.pool!, provider, scope),
      validate: runValidator,
      upsert: (input) => upsertCredential(deps.pool!, input),
      logger: deps.logger,
    };

    const results: CredentialBulkRowResult[] = [];
    for (const item of items) {
      try {
        results.push(await processCredentialBulkRow(ctx, item, { dryRun: dry_run, actor }));
      } catch (e) {
        // Granular fail-honest: a row-level crash is reported on that row only.
        results.push({
          provider: item.provider,
          scope: item.scope,
          action: "error",
          error: (e as Error).message.slice(0, 200),
        });
      }
    }

    const summary = {
      total: results.length,
      updated: results.filter((x) => x.action === "updated").length,
      noop: results.filter((x) => x.action === "noop").length,
      invalid: results.filter((x) => x.action === "invalid").length,
      error: results.filter((x) => x.action === "error").length,
    };
    deps.logger.info({
      event: "credentials.bulk",
      dry_run,
      actor,
      ...summary,
    });
    res.status(200).json({ dry_run, summary, items: results });
  });

  return r;
}
