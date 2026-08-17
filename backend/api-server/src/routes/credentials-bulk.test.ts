/**
 * RunFullSyncCycle FASE 1 — unit tests for the bulk row processor.
 *
 * The processor is dependency-injected (BulkRowContext) so these tests run
 * without PG: fakes record calls and return canned stored rows. What they
 * guard (operator contract):
 *   - Idempotencia estricta: same secret + same metadata ⇒ `noop`, NO write
 *     (updated_at never rotates on re-runs).
 *   - dry_run validates everything and persists NOTHING.
 *   - A row without any secret (provided or stored) is `invalid`, not a crash.
 *   - Metadata-only refresh: absent secret_value reuses the stored one.
 *   - Sanitization: `_validation.providers` carries name/ok/detail only —
 *     never the raw URL (API keys live there).
 *   - The `_validation` server-managed key NEVER breaks idempotency (it is
 *     excluded from the metadata comparison).
 */
import { describe, it, expect } from "vitest";
import type { CredentialBulkItem, CredentialTestResult } from "@arbx/shared";

import {
  processCredentialBulkRow,
  type BulkRowContext,
} from "./credentials.js";
import type { StoredCredentialRow } from "../credentials/store.js";

const noopLogger = { warn: () => {}, info: () => {} };

function makeCtx(stored: StoredCredentialRow | null) {
  const calls = { read: 0, validate: 0, upsert: 0 };
  const ctx: BulkRowContext = {
    readStored: async () => {
      calls.read += 1;
      return stored;
    },
    validate: async (provider, _scope, secret, _metadata) => {
      calls.validate += 1;
      const r: CredentialTestResult = {
        status: "valid",
        message: `${secret.length} chars ok`,
        tested_at: new Date().toISOString(),
        details:
          provider === "rpc_http"
            ? {
                providers: [
                  { name: "alpha", url: "https://alpha.example/v2/SECRETKEY", ok: true, detail: "chain 1" },
                  { name: "beta", url: "https://beta.example/v2/SECRETKEY", ok: false, detail: "http 429" },
                ],
              }
            : undefined,
      };
      return r;
    },
    upsert: async (input) => {
      calls.upsert += 1;
      return {
        id: "00000000-0000-4000-8000-000000000001",
        provider: input.provider,
        scope: input.scope,
        display_name: input.display_name ?? `${input.provider} (${input.scope})`,
        has_value: true,
        value_suffix: "…tail",
        status: input.status,
        last_validated_at: new Date().toISOString(),
        last_validation_error: input.validation_error,
        metadata: input.metadata,
        updated_at: new Date().toISOString(),
        updated_by: input.actor,
      };
    },
    logger: noopLogger,
  };
  return { ctx, calls };
}

const item = (over: Partial<CredentialBulkItem> = {}): CredentialBulkItem => ({
  provider: "coingecko_pro",
  scope: "global",
  secret_value: "CG-abcdef0123456789",
  metadata: {},
  ...over,
});

const storedRow = (over: Partial<StoredCredentialRow> = {}): StoredCredentialRow => ({
  secret: "CG-abcdef0123456789",
  metadata: {},
  status: "valid",
  display_name: "Coingecko Pro",
  updated_at: "2026-08-17T00:00:00.000Z",
  ...over,
});

describe("processCredentialBulkRow — idempotencia estricta", () => {
  it("reports noop and writes NOTHING when secret + metadata are unchanged", async () => {
    const stored = storedRow({
      metadata: { _validation: { message: null, providers: [{ name: "x", ok: true, detail: "d" }] } },
    });
    const { ctx, calls } = makeCtx(stored);
    const r = await processCredentialBulkRow(ctx, item(), { dryRun: false, actor: "operator:macro" });
    expect(r.action).toBe("noop");
    expect(r.status).toBe("valid");
    expect(calls.upsert).toBe(0); // updated_at does not rotate
    expect(calls.validate).toBe(0); // nothing to re-prove
  });

  it("re-validates and writes when the secret CHANGED", async () => {
    const { ctx, calls } = makeCtx(storedRow({ secret: "CG-OLD" }));
    const r = await processCredentialBulkRow(ctx, item(), { dryRun: false, actor: "operator:macro" });
    expect(r.action).toBe("updated");
    expect(r.status).toBe("valid");
    expect(calls.upsert).toBe(1);
    expect(calls.validate).toBe(1);
  });

  it("writes when metadata changed even with the same secret", async () => {
    const { ctx, calls } = makeCtx(storedRow({ metadata: { tier: "free" } }));
    const r = await processCredentialBulkRow(
      ctx,
      item({ metadata: { tier: "pro" } }),
      { dryRun: false, actor: "operator:macro" },
    );
    expect(r.action).toBe("updated");
    expect(calls.upsert).toBe(1);
  });
});

describe("processCredentialBulkRow — dry_run (homologación)", () => {
  it("validates but persists NOTHING", async () => {
    const { ctx, calls } = makeCtx(null);
    const r = await processCredentialBulkRow(ctx, item(), { dryRun: true, actor: "operator:macro" });
    expect(r.action).toBe("validated");
    expect(calls.validate).toBe(1);
    expect(calls.upsert).toBe(0);
  });
});

describe("processCredentialBulkRow — granular fail-honest", () => {
  it("row with no secret anywhere is invalid, not a crash", async () => {
    const { ctx } = makeCtx(null);
    const r = await processCredentialBulkRow(
      ctx,
      item({ secret_value: null }),
      { dryRun: false, actor: "operator:macro" },
    );
    expect(r.action).toBe("invalid");
    expect(r.error).toContain("no secret");
  });

  it("absent secret_value reuses the STORED secret (metadata-only refresh)", async () => {
    const { ctx, calls } = makeCtx(storedRow({ secret: "CG-stored" }));
    const r = await processCredentialBulkRow(
      ctx,
      item({ secret_value: null, metadata: { tier: "pro" } }),
      { dryRun: false, actor: "operator:macro" },
    );
    expect(r.action).toBe("updated");
    expect(calls.validate).toBe(1); // validated against the stored secret
  });
});

describe("processCredentialBulkRow — sanitización", () => {
  it("_validation.providers carries name/ok/detail but NEVER the raw URL", async () => {
    const { ctx, calls } = makeCtx(null);
    const r = await processCredentialBulkRow(
      ctx,
      item({ provider: "rpc_http", scope: "chain:1" }),
      { dryRun: false, actor: "operator:macro" },
    );
    expect(r.action).toBe("updated");
    const providers = r.providers!;
    expect(providers).toHaveLength(2);
    expect(providers[0]).toEqual({ name: "alpha", ok: true, detail: "chain 1" });
    const serialized = JSON.stringify(r);
    expect(serialized).not.toContain("SECRETKEY");
    expect(serialized).not.toContain("alpha.example");
    expect(calls.upsert).toBe(1);
  });
});
