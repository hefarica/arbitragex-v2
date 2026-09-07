/**
 * RunFullSyncCycle FASE 3 — PG→Redis credential projection.
 *
 * Mirrors the sanctioned trading-config pattern (routes/trading-config.ts):
 * admin writes SET the mirror and publish a reload channel; a boot
 * re-hydration replays every PG row so a Redis restart/flush can never leave
 * the mirror empty. Precedence for runtime consumers is documented as
 * projection → .env fallback (never the other way).
 *
 * SECURITY: Redis here is the internal docker network behind the socket-proxy
 * ACL — the projected value carries the RAW secret by design (that is its
 * purpose). Public/edge endpoints NEVER read this projection; the masked-list
 * contract (last-4 suffix only) is enforced on the HTTP surface and its tests.
 *
 * WO-03 (2026-09-06): the boot re-hydration now (a) runs the envelope
 * backfill sweep FIRST (idempotent, crash-safe — see credentials/crypto.ts)
 * and (b) decrypts envelope rows before mirroring. Contract change: the
 * function now THROWS CredentialsKeyRequiredError when ciphertext rows exist
 * but no master key is configured — boot fail-fast (RULE 02) instead of
 * silently projecting an empty/partial mirror. All other failures stay
 * non-throwing as before.
 */
import type { Pool } from "pg";
import type { Redis } from "ioredis";
import type { CredentialStatus } from "@arbx/shared";
// WO-03 (2026-09-06)
import { backfillCredentialEncryption, decryptSecret, resolveMasterKeys } from "./crypto.js";

export const SVC_CRED_KEY_PREFIX = "arbx:svc_cred:";
export const SVC_CRED_CHANNEL = "arbx:svc_cred:reload";

export function svcCredKey(provider: string, scope: string): string {
  return `${SVC_CRED_KEY_PREFIX}${provider}:${scope}`;
}

/** What a runtime consumer reads from the projection (raw secret included). */
export interface ProjectedCredential {
  provider: string;
  scope: string;
  secret_value: string;
  metadata: Record<string, unknown>;
  status: CredentialStatus;
  updated_at: string;
  updated_by: string | null;
}

interface Logger {
  warn: (o: object, m?: string) => void;
  info: (o: object, m?: string) => void;
}

/**
 * Mirror ONE credential row to Redis + publish the reload channel.
 * Fail-honest: a Redis failure logs a warn and returns false — it must never
 * break the admin write path (PG remains the source of truth).
 */
export async function mirrorCredential(
  deps: { redis: Redis; logger: Logger },
  row: ProjectedCredential,
): Promise<boolean> {
  const json = JSON.stringify(row);
  try {
    await deps.redis.set(svcCredKey(row.provider, row.scope), json);
    await deps.redis.publish(SVC_CRED_CHANNEL, json);
    return true;
  } catch (e) {
    deps.logger.warn({
      event: "credentials.projection_mirror_failed",
      provider: row.provider,
      scope: row.scope,
      err: (e as Error).message,
    });
    return false;
  }
}

/** Remove a credential from the projection (DELETE path) + tombstone publish. */
export async function unmirrorCredential(
  deps: { redis: Redis; logger: Logger },
  provider: string,
  scope: string,
): Promise<void> {
  try {
    await deps.redis.del(svcCredKey(provider, scope));
    await deps.redis.publish(SVC_CRED_CHANNEL, JSON.stringify({ provider, scope, deleted: true }));
  } catch (e) {
    deps.logger.warn({
      event: "credentials.projection_unmirror_failed",
      provider,
      scope,
      err: (e as Error).message,
    });
  }
}

/**
 * Boot re-hydration (exact analogue of rehydrateTradingConfigMirror): replay
 * every row that HAS a secret from PG into the mirror. Idempotent SETs; never
 * throws (must not break boot); logs precisely what was mirrored.
 *
 * WO-03 (2026-09-06): runs the envelope backfill sweep first, then decrypts
 * envelope rows before mirroring. A missing master key WITH ciphertext rows
 * present rethrows CredentialsKeyRequiredError AFTER logging (boot fail-fast,
 * RULE 02 — the caller at index.ts fire-and-forgets, so the rejection aborts
 * the process under Node 20's default unhandled-rejections=throw).
 */
export async function rehydrateSvcCredMirror(deps: {
  pool: Pool | null;
  redis: Redis;
  logger: Logger;
}): Promise<void> {
  if (!deps.pool) {
    deps.logger.warn({ event: "credentials.projection_rehydrate_skipped", reason: "db_unavailable" });
    return;
  }
  // WO-03 (2026-09-06): convert/verify/rotate rows before projecting. Throws
  // only for the key-required / backfill-integrity cases (documented above);
  // those MUST not be swallowed — serving a partial mirror would be silent
  // corruption.
  await backfillCredentialEncryption(deps.pool, deps.logger);
  try {
    const q = await deps.pool.query<{
      provider: string;
      scope: string;
      secret_value: string | null;
      secret_ciphertext: Buffer | null;
      secret_salt: Buffer | null;
      secret_key_version: number | null;
      metadata: Record<string, unknown> | null;
      status: string;
      updated_at: Date;
      updated_by: string | null;
    }>(
      `SELECT provider, scope, secret_value, secret_ciphertext, secret_salt, secret_key_version,
              metadata, status, updated_at, updated_by
         FROM service_credentials
        WHERE (secret_value IS NOT NULL AND secret_value <> '')
           OR secret_ciphertext IS NOT NULL`,
    );
    const keys = resolveMasterKeys();
    let mirrored = 0;
    for (const r of q.rows) {
      // WO-03 (2026-09-06): envelope rows decrypt to the RAW secret (the
      // projection's whole purpose); legacy plaintext rows pass through.
      let secret: string;
      if (r.secret_ciphertext) {
        if (!r.secret_salt || r.secret_key_version === null) continue; // incomplete envelope — never fabricate (R8)
        secret = await decryptSecret(
          deps.pool,
          { ciphertext: r.secret_ciphertext, salt: r.secret_salt, keyVersion: r.secret_key_version },
          keys,
          `${r.provider}/${r.scope}`,
        );
      } else if (r.secret_value) {
        secret = r.secret_value;
      } else {
        continue;
      }
      const ok = await mirrorCredential({ redis: deps.redis, logger: deps.logger }, {
        provider: r.provider,
        scope: r.scope,
        secret_value: secret,
        metadata: r.metadata ?? {},
        status: r.status as CredentialStatus,
        updated_at: r.updated_at.toISOString(),
        updated_by: r.updated_by,
      });
      if (ok) mirrored += 1;
    }
    deps.logger.info(
      {
        event: "credentials.projection_rehydrated",
        rows: q.rows.length,
        mirrored,
        channel: SVC_CRED_CHANNEL,
        key_prefix: SVC_CRED_KEY_PREFIX,
      },
      "service_credentials Redis projection re-hydrated from Postgres at boot",
    );
  } catch (e) {
    if (e instanceof Error && e.name === "CredentialsKeyRequiredError") {
      deps.logger.warn(
        { event: "credentials.projection_rehydrate_blocked", err: e.message },
        "svc_cred re-hydration blocked: encrypted rows without master key — aborting boot (RULE 02)",
      );
      throw e;
    }
    deps.logger.warn(
      { event: "credentials.projection_rehydrate_failed", err: (e as Error).message },
      "svc_cred boot re-hydration failed (non-fatal)",
    );
  }
}
