/**
 * WO-03 (2026-09-06) — Envelope encryption for service_credentials.
 *
 * Design: audits/omniscience-integration-2026-09-06/WO-03-DESIGN.md.
 *
 * Envelope scheme (pgcrypto, master key EXTERNAL to the DB):
 *   salt(row)  = 16 random bytes
 *   RK(row)    = hex( HMAC-SHA256(master_key_version, "arbx-svc-cred-v1:" || salt) )
 *   ciphertext = pgp_sym_encrypt(secret, RK_hex, 'cipher-algo=aes256, compress-algo=0')
 *
 * Only the per-row RK crosses to Postgres as a bind parameter — the master
 * key NEVER does. The SQL-side derivation (migration 120, Path A) is locked
 * byte-identical to this one; runtime parity is asserted by the boot
 * backfill's decrypt-verify phase (a mismatch aborts boot).
 *
 * Key material sources (in order, arbx-no-hardcode-doctrine — placeholders
 * only in repo, real values only in VPS .env / Vault-agent sink):
 *   1. ARBX_CREDENTIALS_MASTER_KEY_FILE  (Vault agent template sink)
 *   2. ARBX_CREDENTIALS_MASTER_KEY       (env)
 *   3. absent/empty ⇒ legacy plaintext mode (honest degradation; FAIL-FAST
 *      via CredentialsKeyRequiredError the moment a ciphertext row exists
 *      without a key — RULE 02: missing security env = crash, not degrade).
 *
 * Rotation: ARBX_CREDENTIALS_MASTER_KEY_PREV holds the previous-version key
 * during the rotation window; ARBX_CREDENTIALS_KEY_VERSION (default 1) is the
 * version of the CURRENT key. Rows encrypted at version < current are
 * re-encrypted at boot by backfillCredentialEncryption().
 */

import { createHmac, randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import type { Pool } from "pg";

// WO-03 (2026-09-06) — derivation domain separator + pgcrypto options.
// MUST stay byte-identical with migration 120's SQL derivation.
export const CREDENTIALS_ENVELOPE_DOMAIN = "arbx-svc-cred-v1:";
export const CREDENTIALS_PGP_OPTIONS = "cipher-algo=aes256, compress-algo=0";

export const CREDENTIALS_KEY_MIN_LENGTH = 32;
const SALT_BYTES = 16;

export class CredentialsKeyRequiredError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CredentialsKeyRequiredError";
  }
}

export class CredentialsDecryptError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CredentialsDecryptError";
  }
}

export class CredentialsBackfillError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CredentialsBackfillError";
  }
}

export interface MasterKeyMaterial {
  /** Version of `current` (rows written now get this number). */
  currentVersion: number;
  current: string;
  /** Key of version currentVersion-1 during a rotation window, else null. */
  previous: string | null;
}

interface EnvLike {
  [k: string]: string | undefined;
}

let cachedKeys: MasterKeyMaterial | null | undefined;

/**
 * Resolve the master key material from the environment (memoized per
 * process — env is stable for the container lifetime).
 *
 * - returns null when NO key is configured (legacy plaintext mode);
 * - THROWS on half-configuration (file configured but missing/empty, or a
 *   non-empty key shorter than CREDENTIALS_KEY_MIN_LENGTH) — a half-configured
 *   key is a security incident, not a fallback case (fail-fast, R8).
 */
export function resolveMasterKeys(env: EnvLike = process.env): MasterKeyMaterial | null {
  if (cachedKeys !== undefined) return cachedKeys;

  let current: string | null = null;
  const file = (env["ARBX_CREDENTIALS_MASTER_KEY_FILE"] ?? "").trim();
  if (file) {
    let content: string;
    try {
      content = readFileSync(file, "utf8").trim();
    } catch (e) {
      throw new CredentialsKeyRequiredError(
        `credentials master key file configured but unreadable (${file}): ${(e as Error).message}`,
      );
    }
    if (!content) {
      throw new CredentialsKeyRequiredError(
        `credentials master key file configured but empty (${file})`,
      );
    }
    current = content;
  } else {
    const fromEnv = (env["ARBX_CREDENTIALS_MASTER_KEY"] ?? "").trim();
    current = fromEnv || null;
  }

  if (current === null) {
    cachedKeys = null; // legacy mode — no key, no ciphertext writes
    return cachedKeys;
  }
  if (current.length < CREDENTIALS_KEY_MIN_LENGTH) {
    throw new CredentialsKeyRequiredError(
      `credentials master key too short (${current.length} < ${CREDENTIALS_KEY_MIN_LENGTH} chars) — refusing to boot half-configured`,
    );
  }

  const prev = (env["ARBX_CREDENTIALS_MASTER_KEY_PREV"] ?? "").trim() || null;
  const versionRaw = (env["ARBX_CREDENTIALS_KEY_VERSION"] ?? "1").trim();
  const currentVersion = Number.parseInt(versionRaw, 10);
  if (!Number.isFinite(currentVersion) || currentVersion < 1) {
    throw new CredentialsKeyRequiredError(
      `ARBX_CREDENTIALS_KEY_VERSION must be an integer >= 1 (got "${versionRaw}")`,
    );
  }

  cachedKeys = { currentVersion, current, previous: prev };
  return cachedKeys;
}

/** Test-only escape hatch for the per-process memo. */
export function resetMasterKeyCacheForTests(): void {
  cachedKeys = undefined;
}

/**
 * Per-row key derivation. Parity vector locked in crypto.test.ts and in
 * migration 120's SQL (encode(hmac(convert_to(DOMAIN,'UTF8') || salt,
 * key::bytea, 'sha256'), 'hex')).
 */
export function deriveRowKeyHex(master: string, salt: Buffer): string {
  return createHmac("sha256", master).update(CREDENTIALS_ENVELOPE_DOMAIN).update(salt).digest("hex");
}

/**
 * Public masked suffix for a plaintext secret — EXACT parity with the
 * historical maskSuffix() (store.ts pre-WO-03) and with the SQL CASE in
 * migration 120's Path-A backfill. This is display-only, already part of the
 * public API contract (value_suffix, shared-ts contracts/credentials.ts:49).
 */
export function maskHint(plaintext: string): string {
  const t = plaintext.trim();
  return t.length <= 4 ? "****" : `…${t.slice(-4)}`;
}

export interface SecretEnvelope {
  ciphertext: Buffer;
  salt: Buffer;
  keyVersion: number;
  hint: string;
}

/**
 * Encrypt ONE secret into an envelope. The PGP encryption itself runs inside
 * Postgres (pgcrypto); only the per-row derived key crosses the wire.
 */
export async function encryptSecret(
  pool: Pool,
  plaintext: string,
  keys: MasterKeyMaterial,
): Promise<SecretEnvelope> {
  const salt = randomBytes(SALT_BYTES);
  const rowKey = deriveRowKeyHex(keys.current, salt);
  const q = await pool.query<{ ct: Buffer }>(
    `SELECT pgp_sym_encrypt($1::text, $2::text, $3::text) AS ct`,
    [plaintext, rowKey, CREDENTIALS_PGP_OPTIONS],
  );
  const ct = q.rows[0]?.ct;
  if (!ct || ct.length === 0) {
    throw new CredentialsDecryptError("pgp_sym_encrypt returned empty ciphertext");
  }
  return { ciphertext: ct, salt, keyVersion: keys.currentVersion, hint: maskHint(plaintext) };
}

function masterKeyForVersion(keys: MasterKeyMaterial, version: number): string {
  if (version === keys.currentVersion) return keys.current;
  if (version === keys.currentVersion - 1 && keys.previous) return keys.previous;
  throw new CredentialsDecryptError(
    `unsupported credential key version ${version} (current ${keys.currentVersion}, previous ${keys.previous ? keys.currentVersion - 1 : "none"})`,
  );
}

/**
 * Decrypt ONE envelope row. Wrong key / tampered ciphertext raises inside
 * pgcrypto and is wrapped (no key material in the message). Missing master
 * key with ciphertext present is CredentialsKeyRequiredError (fail-fast).
 */
export async function decryptSecret(
  pool: Pool,
  envelope: { ciphertext: Buffer; salt: Buffer; keyVersion: number },
  keys: MasterKeyMaterial | null,
  context: string,
): Promise<string> {
  if (!keys) {
    throw new CredentialsKeyRequiredError(
      `encrypted credential present but ARBX_CREDENTIALS_MASTER_KEY[_FILE] is not configured (${context})`,
    );
  }
  const rowKey = deriveRowKeyHex(masterKeyForVersion(keys, envelope.keyVersion), envelope.salt);
  try {
    const q = await pool.query<{ pt: string }>(
      `SELECT pgp_sym_decrypt($1::bytea, $2::text) AS pt`,
      [envelope.ciphertext, rowKey],
    );
    const pt = q.rows[0]?.pt;
    if (pt === undefined || pt === null) {
      throw new CredentialsDecryptError(`pgp_sym_decrypt returned NULL (${context})`);
    }
    return pt;
  } catch (e) {
    if (e instanceof CredentialsDecryptError || e instanceof CredentialsKeyRequiredError) throw e;
    throw new CredentialsDecryptError(
      `pgp_sym_decrypt failed for ${context}: ${(e as Error).message.slice(0, 120)}`,
    );
  }
}

export interface BackfillSummary {
  converted: number;
  rotated: number;
  scrubbed: number;
  mode: "envelope" | "legacy";
}

interface BackfillLogger {
  warn: (o: object, m?: string) => void;
  info: (o: object, m?: string) => void;
  error?: (o: object, m?: string) => void;
}

/**
 * Boot sweep (WO-03 design §5.2). Idempotent + crash-safe:
 *
 *  phase 1  encrypt plaintext rows (KEEPING secret_value)   — rowCount guard
 *  phase 2  verify decrypt(ciphertext) === stored plaintext — mismatch aborts
 *  phase 3  scrub secret_value (second UPDATE)               — rowCount guard
 *
 * A crash between phases leaves a both-set row which the NEXT boot repairs
 * (verify + scrub) — that transient is why there is NO "single-source" CHECK
 * constraint in migration 120 (design D5).
 *
 * Stale-version rows (rotation window, PREV key configured) are decrypted
 * with PREV and re-encrypted at the current version in one guarded UPDATE.
 *
 * Throws CredentialsKeyRequiredError when ciphertext rows exist without a
 * configured key (boot fail-fast, RULE 02) and CredentialsBackfillError on
 * any rowCount/roundtrip mismatch. Never fabricates or rewrites data (R8).
 */
export async function backfillCredentialEncryption(
  pool: Pool,
  logger: BackfillLogger,
): Promise<BackfillSummary> {
  const keys = resolveMasterKeys();

  if (!keys) {
    const q = await pool.query<{ n: string }>(
      `SELECT count(*)::text AS n FROM service_credentials WHERE secret_ciphertext IS NOT NULL`,
    );
    const n = Number(q.rows[0]?.n ?? "0");
    if (n > 0) {
      const msg = `credentials.encryption_key_missing: ${n} encrypted rows but no ARBX_CREDENTIALS_MASTER_KEY[_FILE] — refusing to boot (RULE 02)`;
      (logger.error ?? logger.warn)({ event: "credentials.encryption_key_missing", encrypted_rows: n }, msg);
      throw new CredentialsKeyRequiredError(msg);
    }
    logger.warn(
      { event: "credentials.encryption_disabled", reason: "master_key_not_configured" },
      "service_credentials running in LEGACY plaintext mode (identical to pre-WO-03 behavior)",
    );
    return { converted: 0, rotated: 0, scrubbed: 0, mode: "legacy" };
  }

  let converted = 0;
  let rotated = 0;
  let scrubbed = 0;

  // Rows needing conversion: plaintext without envelope.
  const pending = await pool.query<{
    id: string;
    provider: string;
    scope: string;
    secret_value: string;
  }>(
    `SELECT id, provider, scope, secret_value
       FROM service_credentials
      WHERE secret_value IS NOT NULL AND secret_value <> ''
        AND secret_ciphertext IS NULL`,
  );
  for (const r of pending.rows) {
    const env = await encryptSecret(pool, r.secret_value, keys);
    const u = await pool.query(
      `UPDATE service_credentials
          SET secret_ciphertext = $2, secret_salt = $3, secret_key_version = $4, secret_hint = $5
        WHERE id = $1 AND secret_ciphertext IS NULL`,
      [r.id, env.ciphertext, env.salt, env.keyVersion, env.hint],
    );
    if ((u.rowCount ?? 0) !== 1) {
      throw new CredentialsBackfillError(
        `phase-1 rowCount ${(u.rowCount ?? 0)} != 1 for ${r.provider}/${r.scope} (concurrent writer?)`,
      );
    }
    const roundtrip = await decryptSecret(
      pool,
      { ciphertext: env.ciphertext, salt: env.salt, keyVersion: env.keyVersion },
      keys,
      `${r.provider}/${r.scope}`,
    );
    if (roundtrip !== r.secret_value) {
      throw new CredentialsBackfillError(
        `phase-2 roundtrip mismatch for ${r.provider}/${r.scope} — plaintext left INTACT, aborting`,
      );
    }
    await scrubPlaintext(pool, r.id);
    scrubbed += 1;
    converted += 1;
  }

  // Crash recovery: rows with BOTH sources set (crash between phase 1 and 3).
  const both = await pool.query<{ id: string; provider: string; scope: string; secret_value: string }>(
    `SELECT id, provider, scope, secret_value
       FROM service_credentials
      WHERE secret_value IS NOT NULL AND secret_value <> ''
        AND secret_ciphertext IS NOT NULL`,
  );
  for (const r of both.rows) {
    // The plaintext is still present — verify the envelope against it, then scrub.
    const cur = await pool.query<{
      secret_ciphertext: Buffer;
      secret_salt: Buffer;
      secret_key_version: number;
    }>(
      `SELECT secret_ciphertext, secret_salt, secret_key_version
         FROM service_credentials WHERE id = $1`,
      [r.id],
    );
    const row = cur.rows[0];
    if (!row) continue;
    const roundtrip = await decryptSecret(
      pool,
      { ciphertext: row.secret_ciphertext, salt: row.secret_salt, keyVersion: row.secret_key_version },
      keys,
      `${r.provider}/${r.scope}`,
    );
    if (roundtrip !== r.secret_value) {
      throw new CredentialsBackfillError(
        `crash-recovery roundtrip mismatch for ${r.provider}/${r.scope} — plaintext INTACT, aborting`,
      );
    }
    await scrubPlaintext(pool, r.id);
    scrubbed += 1;
  }

  // Rotation: rows encrypted at a previous version, PREV key available.
  if (keys.previous) {
    const stale = await pool.query<{
      id: string;
      provider: string;
      scope: string;
      secret_ciphertext: Buffer;
      secret_salt: Buffer;
      secret_key_version: number;
    }>(
      `SELECT id, provider, scope, secret_ciphertext, secret_salt, secret_key_version
         FROM service_credentials
        WHERE secret_ciphertext IS NOT NULL
          AND secret_key_version < $1`,
      [keys.currentVersion],
    );
    for (const r of stale.rows) {
      const plaintext = await decryptSecret(
        pool,
        { ciphertext: r.secret_ciphertext, salt: r.secret_salt, keyVersion: r.secret_key_version },
        keys,
        `${r.provider}/${r.scope}@v${r.secret_key_version}`,
      );
      const env = await encryptSecret(pool, plaintext, keys);
      const u = await pool.query(
        `UPDATE service_credentials
            SET secret_ciphertext = $2, secret_salt = $3, secret_key_version = $4, secret_hint = $5
          WHERE id = $1 AND secret_key_version = $6`,
        [r.id, env.ciphertext, env.salt, env.keyVersion, env.hint, r.secret_key_version],
      );
      if ((u.rowCount ?? 0) !== 1) {
        throw new CredentialsBackfillError(
          `rotation rowCount ${(u.rowCount ?? 0)} != 1 for ${r.provider}/${r.scope}`,
        );
      }
      const roundtrip = await decryptSecret(
        pool,
        { ciphertext: env.ciphertext, salt: env.salt, keyVersion: env.keyVersion },
        keys,
        `${r.provider}/${r.scope}@v${env.keyVersion}`,
      );
      if (roundtrip !== plaintext) {
        throw new CredentialsBackfillError(
          `rotation roundtrip mismatch for ${r.provider}/${r.scope} — restore from the pre-rotation pg_dump`,
        );
      }
      rotated += 1;
    }
  }

  // Final invariant assert (design D5): no row may carry both sources.
  const inv = await pool.query<{ n: string }>(
    `SELECT count(*)::text AS n FROM service_credentials
      WHERE secret_value IS NOT NULL AND secret_value <> ''
        AND secret_ciphertext IS NOT NULL`,
  );
  if (Number(inv.rows[0]?.n ?? "0") !== 0) {
    throw new CredentialsBackfillError("invariant violated: rows with both plaintext and ciphertext remain");
  }

  logger.info(
    {
      event: "credentials.backfill_complete",
      converted,
      rotated,
      scrubbed,
      mode: "envelope",
      key_version: keys.currentVersion,
    },
    "service_credentials envelope-encryption sweep complete",
  );
  return { converted, rotated, scrubbed, mode: "envelope" };
}

async function scrubPlaintext(pool: Pool, id: string): Promise<void> {
  const u = await pool.query(
    `UPDATE service_credentials SET secret_value = NULL
      WHERE id = $1 AND secret_ciphertext IS NOT NULL`,
    [id],
  );
  if ((u.rowCount ?? 0) !== 1) {
    throw new CredentialsBackfillError(`phase-3 scrub rowCount ${(u.rowCount ?? 0)} != 1 for id ${id}`);
  }
}
