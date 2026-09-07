/**
 * WO-03 (2026-09-06) — unit tests for the service_credentials envelope
 * encryption module. Pure functions + a scripted FakePool (same convention as
 * projection.test.ts): no live PG, no fabricated production data — every
 * "secret" here is a synthetic test vector.
 *
 * Guards:
 *   - Row-key derivation parity vector (locked against the SQL derivation in
 *     migration 120 — byte-identical contract, design §1.2).
 *   - maskHint parity with the historical maskSuffix on every reachable path.
 *   - Master-key resolution: legacy / fail-fast half-config / version parse.
 *   - decryptSecret version routing (current / PREV window / unsupported /
 *     key-required / pg-error wrapping).
 *   - Boot backfill: conversion phases with rowCount guards, roundtrip
 *     verify-before-scrub, crash recovery, rotation, invariant assert.
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Pool } from "pg";

import {
  backfillCredentialEncryption,
  CredentialsBackfillError,
  CredentialsDecryptError,
  CredentialsKeyRequiredError,
  decryptSecret,
  deriveRowKeyHex,
  encryptSecret,
  maskHint,
  resolveMasterKeys,
  resetMasterKeyCacheForTests,
  CREDENTIALS_PGP_OPTIONS,
} from "./crypto.js";

const TEST_MASTER = "unit-test-master-key-32-chars-ok!!"; // 34 chars — synthetic vector
const noopLogger = {
  warn: () => {},
  info: () => {},
  error: () => {},
};

/** Scripted pool: records every (sql, params) and answers via responder. */
class FakePool {
  calls: Array<{ sql: string; params: unknown[] }> = [];
  constructor(private respond: (sql: string, params: unknown[]) => { rows: unknown[]; rowCount?: number }) {}
  async query(sql: string, params: unknown[] = []): Promise<{ rows: any[]; rowCount: number }> {
    this.calls.push({ sql, params });
    const r = this.respond(sql, params);
    return { rows: r.rows as any[], rowCount: r.rowCount ?? r.rows.length };
  }
}

beforeEach(() => {
  resetMasterKeyCacheForTests();
  delete process.env["ARBX_CREDENTIALS_MASTER_KEY"];
  delete process.env["ARBX_CREDENTIALS_MASTER_KEY_PREV"];
  delete process.env["ARBX_CREDENTIALS_KEY_VERSION"];
  delete process.env["ARBX_CREDENTIALS_MASTER_KEY_FILE"];
});

afterEach(() => {
  resetMasterKeyCacheForTests();
  delete process.env["ARBX_CREDENTIALS_MASTER_KEY"];
  delete process.env["ARBX_CREDENTIALS_MASTER_KEY_PREV"];
  delete process.env["ARBX_CREDENTIALS_KEY_VERSION"];
  delete process.env["ARBX_CREDENTIALS_MASTER_KEY_FILE"];
});

describe("deriveRowKeyHex — parity vector (locked vs migration 120 SQL)", () => {
  it("matches the fixed HMAC-SHA256 vector for the documented master/salt pair", () => {
    // Vector computed independently of the implementation under test:
    // master = TEST_MASTER, salt = 000102...0f, message = "arbx-svc-cred-v1:" || salt.
    expect(deriveRowKeyHex(TEST_MASTER, Buffer.from("000102030405060708090a0b0c0d0e0f", "hex")))
      .toBe("b9a03be3109b0b76991d9d6750ae3410f0926f73208da1b53eac7c59f0ccb6ef");
  });

  it("differs per salt (per-row key separation)", () => {
    const a = deriveRowKeyHex(TEST_MASTER, Buffer.alloc(16, 1));
    const b = deriveRowKeyHex(TEST_MASTER, Buffer.alloc(16, 2));
    expect(a).not.toBe(b);
  });
});

describe("maskHint — public suffix parity with historical maskSuffix", () => {
  it.each([
    ["abcd", "****"],            // short → fully masked
    ["abcde", "…bcde"],          // long → last-4
    ["  abcde  ", "…bcde"],      // trimmed first
    ["   ", "****"],             // whitespace-only (SQL Path A parity)
  ])("%j → %j", (input, expected) => {
    expect(maskHint(input)).toBe(expected);
  });
});

describe("resolveMasterKeys", () => {
  it("returns null (legacy mode) when nothing is configured", () => {
    expect(resolveMasterKeys({})).toBeNull();
  });

  it("returns null for an empty env value (compose default `${VAR:-}`)", () => {
    expect(resolveMasterKeys({ ARBX_CREDENTIALS_MASTER_KEY: "" })).toBeNull();
  });

  it("THROWS on a half-configured short key (fail-fast, R8)", () => {
    expect(() => resolveMasterKeys({ ARBX_CREDENTIALS_MASTER_KEY: "too-short" }))
      .toThrowError(CredentialsKeyRequiredError);
  });

  it("parses current key, version default 1 and absent previous", () => {
    const k = resolveMasterKeys({ ARBX_CREDENTIALS_MASTER_KEY: TEST_MASTER });
    expect(k).toEqual({ currentVersion: 1, current: TEST_MASTER, previous: null });
  });

  it("parses ARBX_CREDENTIALS_KEY_VERSION and _PREV (rotation window)", () => {
    const k = resolveMasterKeys({
      ARBX_CREDENTIALS_MASTER_KEY: TEST_MASTER,
      ARBX_CREDENTIALS_MASTER_KEY_PREV: "previous-master-key-32-chars-ok!!!",
      ARBX_CREDENTIALS_KEY_VERSION: "2",
    });
    expect(k).toEqual({
      currentVersion: 2,
      current: TEST_MASTER,
      previous: "previous-master-key-32-chars-ok!!!",
    });
  });

  it("rejects malformed version (int >= 1 required)", () => {
    expect(() => resolveMasterKeys({ ARBX_CREDENTIALS_MASTER_KEY: TEST_MASTER, ARBX_CREDENTIALS_KEY_VERSION: "abc" }))
      .toThrowError(CredentialsKeyRequiredError);
    expect(() => resolveMasterKeys({ ARBX_CREDENTIALS_MASTER_KEY: TEST_MASTER, ARBX_CREDENTIALS_KEY_VERSION: "0" }))
      .toThrowError(CredentialsKeyRequiredError);
  });

  it("reads the key from ARBX_CREDENTIALS_MASTER_KEY_FILE (Vault agent sink)", () => {
    const dir = mkdtempSync(join(tmpdir(), "arbx-wo03-"));
    try {
      const f = join(dir, "master-key");
      writeFileSync(f, `${TEST_MASTER}\n`, "utf8");
      const k = resolveMasterKeys({ ARBX_CREDENTIALS_MASTER_KEY_FILE: f });
      expect(k?.current).toBe(TEST_MASTER);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("THROWS when the configured key file is missing (misconfig is never silent)", () => {
    expect(() =>
      resolveMasterKeys({ ARBX_CREDENTIALS_MASTER_KEY_FILE: join(tmpdir(), "definitely-absent-arbx-wo03") }),
    ).toThrowError(CredentialsKeyRequiredError);
  });
});

describe("encryptSecret / decryptSecret", () => {
  it("encrypts via pgcrypto with the derived row key and returns a complete envelope", async () => {
    process.env["ARBX_CREDENTIALS_MASTER_KEY"] = TEST_MASTER;
    const pool = new FakePool(() => ({ rows: [{ ct: Buffer.from("ct-bytes") }] }));
    const env = await encryptSecret(pool as unknown as Pool, "CG-plain-secret", resolveMasterKeys()!);
    expect(env.ciphertext.toString()).toBe("ct-bytes");
    expect(env.salt.length).toBe(16);
    expect(env.keyVersion).toBe(1);
    expect(env.hint).toBe("…cret");
    const sql = (pool as FakePool).calls[0]!;
    expect(sql.sql).toContain("pgp_sym_encrypt");
    expect(sql.params[0]).toBe("CG-plain-secret");
    expect(sql.params[2]).toBe(CREDENTIALS_PGP_OPTIONS);
    // The MASTER key never crosses the wire — only the derived row key does.
    expect(sql.params[1]).toBe(deriveRowKeyHex(TEST_MASTER, env.salt));
    expect(JSON.stringify(sql.params)).not.toContain(TEST_MASTER);
  });

  it("decrypts the current version with the current key", async () => {
    process.env["ARBX_CREDENTIALS_MASTER_KEY"] = TEST_MASTER;
    const keys = resolveMasterKeys()!;
    const pool = new FakePool(() => ({ rows: [{ pt: "plain" }] }));
    const out = await decryptSecret(
      pool as unknown as Pool,
      { ciphertext: Buffer.from("ct"), salt: Buffer.alloc(16, 7), keyVersion: 1 },
      keys,
      "coingecko_pro/global",
    );
    expect(out).toBe("plain");
    expect((pool as FakePool).calls[0]!.params[1]).toBe(deriveRowKeyHex(TEST_MASTER, Buffer.alloc(16, 7)));
  });

  it("decrypts version current-1 with the PREV key during the rotation window", async () => {
    const PREV = "previous-master-key-32-chars-ok!!!";
    process.env["ARBX_CREDENTIALS_MASTER_KEY"] = TEST_MASTER;
    process.env["ARBX_CREDENTIALS_MASTER_KEY_PREV"] = PREV;
    process.env["ARBX_CREDENTIALS_KEY_VERSION"] = "2";
    const keys = resolveMasterKeys()!;
    const pool = new FakePool(() => ({ rows: [{ pt: "plain" }] }));
    const salt = Buffer.alloc(16, 3);
    await decryptSecret(pool as unknown as Pool, { ciphertext: Buffer.from("ct"), salt, keyVersion: 1 }, keys, "ctx");
    expect((pool as FakePool).calls[0]!.params[1]).toBe(deriveRowKeyHex(PREV, salt));
  });

  it("rejects version current-1 without PREV, and anything older (fail-honest)", async () => {
    process.env["ARBX_CREDENTIALS_MASTER_KEY"] = TEST_MASTER;
    process.env["ARBX_CREDENTIALS_KEY_VERSION"] = "2";
    const keys = resolveMasterKeys()!;
    const pool = new FakePool(() => ({ rows: [{ pt: "x" }] }));
    // decryptSecret is async: the rejection must be AWAITED, not asserted as a
    // synchronous throw (the pre-verification draft used `expect(() => …)` here
    // and leaked an unhandled rejection — caught by vitest on first run).
    await expect(
      decryptSecret(pool as unknown as Pool, { ciphertext: Buffer.from("ct"), salt: Buffer.alloc(16), keyVersion: 1 }, keys, "ctx"),
    ).rejects.toThrowError(CredentialsDecryptError);
    process.env["ARBX_CREDENTIALS_MASTER_KEY_PREV"] = "previous-master-key-32-chars-ok!!!";
    resetMasterKeyCacheForTests();
    const keys2 = resolveMasterKeys()!;
    await expect(
      decryptSecret(pool as unknown as Pool, { ciphertext: Buffer.from("ct"), salt: Buffer.alloc(16), keyVersion: 0 }, keys2, "ctx"),
    ).rejects.toThrowError(CredentialsDecryptError);
  });

  it("fails fast when an envelope exists but no key is configured", async () => {
    const pool = new FakePool(() => ({ rows: [{ pt: "x" }] }));
    await expect(
      decryptSecret(pool as unknown as Pool, { ciphertext: Buffer.from("ct"), salt: Buffer.alloc(16), keyVersion: 1 }, null, "ctx"),
    ).rejects.toThrowError(CredentialsKeyRequiredError);
  });

  it("wraps pgcrypto failures without leaking key material", async () => {
    process.env["ARBX_CREDENTIALS_MASTER_KEY"] = TEST_MASTER;
    const keys = resolveMasterKeys()!;
    const pool = new FakePool(() => {
      throw new Error("pg_sym_decrypt: Wrong key or corrupted data");
    });
    await expect(
      decryptSecret(pool as unknown as Pool, { ciphertext: Buffer.from("ct"), salt: Buffer.alloc(16), keyVersion: 1 }, keys, "binance/global"),
    ).rejects.toThrowError(CredentialsDecryptError);
  });
});

describe("backfillCredentialEncryption (boot sweep)", () => {
  const PLAIN = "CG-abcdef0123456789";
  const CT = Buffer.from("envelope-ct");

  /** Responder for the happy-path conversion of ONE pending row. */
  function conversionPool(opts?: { roundtrip?: string; phase1RowCount?: number }) {
    return new FakePool((sql) => {
      if (sql.includes("SELECT id, provider, scope, secret_value")) {
        return { rows: [{ id: "r1", provider: "coingecko_pro", scope: "global", secret_value: PLAIN }] };
      }
      if (sql.includes("pgp_sym_encrypt")) return { rows: [{ ct: CT }] };
      if (sql.includes("pgp_sym_decrypt")) return { rows: [{ pt: opts?.roundtrip ?? PLAIN }] };
      if (sql.includes("SET secret_ciphertext = $2, secret_salt = $3, secret_key_version = $4, secret_hint = $5")) {
        return { rows: [], rowCount: opts?.phase1RowCount ?? 1 };
      }
      if (sql.includes("SET secret_value = NULL")) return { rows: [], rowCount: 1 };
      if (sql.includes("WHERE secret_value IS NOT NULL AND secret_value <> ''\n        AND secret_ciphertext IS NOT NULL") || (sql.includes("secret_ciphertext IS NOT NULL") && sql.includes("SELECT id, provider, scope, secret_value"))) {
        return { rows: [] };
      }
      if (sql.includes("count(*)")) return { rows: [{ n: "0" }] };
      return { rows: [] };
    });
  }

  it("legacy mode: no key + zero ciphertext rows → no-op with warn", async () => {
    const pool = new FakePool((sql) => {
      if (sql.includes("count(*)")) return { rows: [{ n: "0" }] };
      return { rows: [] };
    });
    const sum = await backfillCredentialEncryption(pool as unknown as Pool, noopLogger);
    expect(sum).toEqual({ converted: 0, rotated: 0, scrubbed: 0, mode: "legacy" });
  });

  it("legacy mode + ciphertext rows present → CredentialsKeyRequiredError (crash on boot, RULE 02)", async () => {
    const pool = new FakePool((sql) => {
      if (sql.includes("count(*)")) return { rows: [{ n: "3" }] };
      return { rows: [] };
    });
    await expect(backfillCredentialEncryption(pool as unknown as Pool, noopLogger))
      .rejects.toThrowError(CredentialsKeyRequiredError);
  });

  it("converts a plaintext row through encrypt → verify → scrub (rowCount guards)", async () => {
    process.env["ARBX_CREDENTIALS_MASTER_KEY"] = TEST_MASTER;
    const pool = conversionPool();
    const sum = await backfillCredentialEncryption(pool as unknown as Pool, noopLogger);
    expect(sum).toEqual({ converted: 1, rotated: 0, scrubbed: 1, mode: "envelope" });
    const scrub = (pool as FakePool).calls.filter((c) => c.sql.includes("SET secret_value = NULL"));
    expect(scrub).toHaveLength(1);
    // The scrub ONLY runs after the roundtrip verify query.
    const order = (pool as FakePool).calls.map((c) =>
      c.sql.includes("pgp_sym_decrypt") ? "verify" : c.sql.includes("SET secret_value = NULL") ? "scrub" : c.sql.includes("pgp_sym_encrypt") ? "encrypt" : "",
    );
    expect(order.indexOf("verify")).toBeLessThan(order.indexOf("scrub"));
    expect(order.indexOf("encrypt")).toBeLessThan(order.indexOf("verify"));
  });

  it("roundtrip mismatch aborts BEFORE scrubbing (plaintext stays intact)", async () => {
    process.env["ARBX_CREDENTIALS_MASTER_KEY"] = TEST_MASTER;
    const pool = conversionPool({ roundtrip: "NOT-THE-PLAINTEXT" });
    await expect(backfillCredentialEncryption(pool as unknown as Pool, noopLogger))
      .rejects.toThrowError(CredentialsBackfillError);
    expect((pool as FakePool).calls.filter((c) => c.sql.includes("SET secret_value = NULL"))).toHaveLength(0);
  });

  it("phase-1 rowCount != 1 aborts (concurrent writer guard)", async () => {
    process.env["ARBX_CREDENTIALS_MASTER_KEY"] = TEST_MASTER;
    const pool = conversionPool({ phase1RowCount: 0 });
    await expect(backfillCredentialEncryption(pool as unknown as Pool, noopLogger))
      .rejects.toThrowError(CredentialsBackfillError);
  });

  it("repairs crash-recovery rows (both sources set → verify + scrub)", async () => {
    process.env["ARBX_CREDENTIALS_MASTER_KEY"] = TEST_MASTER;
    const pool = new FakePool((sql) => {
      if (sql.includes("SELECT id, provider, scope, secret_value") && sql.includes("secret_ciphertext IS NULL")) {
        return { rows: [] }; // no pending
      }
      if (sql.includes("secret_ciphertext IS NOT NULL") && sql.includes("SELECT id, provider, scope, secret_value")) {
        return { rows: [{ id: "r1", provider: "coingecko_pro", scope: "global", secret_value: PLAIN }] };
      }
      if (sql.includes("SELECT secret_ciphertext, secret_salt, secret_key_version")) {
        return { rows: [{ secret_ciphertext: CT, secret_salt: Buffer.alloc(16, 9), secret_key_version: 1 }] };
      }
      if (sql.includes("pgp_sym_decrypt")) return { rows: [{ pt: PLAIN }] };
      if (sql.includes("SET secret_value = NULL")) return { rows: [], rowCount: 1 };
      if (sql.includes("count(*)")) return { rows: [{ n: "0" }] };
      return { rows: [] };
    });
    const sum = await backfillCredentialEncryption(pool as unknown as Pool, noopLogger);
    expect(sum.scrubbed).toBe(1);
    expect(sum.converted).toBe(0);
  });

  it("rotates stale-version rows when PREV is configured (v1 → v2)", async () => {
    process.env["ARBX_CREDENTIALS_MASTER_KEY"] = TEST_MASTER;
    process.env["ARBX_CREDENTIALS_MASTER_KEY_PREV"] = "previous-master-key-32-chars-ok!!!";
    process.env["ARBX_CREDENTIALS_KEY_VERSION"] = "2";
    const pool = new FakePool((sql) => {
      if (sql.includes("secret_key_version < $1")) {
        return { rows: [{ id: "r1", provider: "coingecko_pro", scope: "global", secret_ciphertext: CT, secret_salt: Buffer.alloc(16, 4), secret_key_version: 1 }] };
      }
      if (sql.includes("pgp_sym_encrypt")) return { rows: [{ ct: Buffer.from("ct-v2") }] };
      if (sql.includes("pgp_sym_decrypt")) return { rows: [{ pt: PLAIN }] };
      if (sql.includes("WHERE id = $1 AND secret_key_version = $6")) return { rows: [], rowCount: 1 };
      if (sql.includes("count(*)")) return { rows: [{ n: "0" }] };
      return { rows: [] };
    });
    const sum = await backfillCredentialEncryption(pool as unknown as Pool, noopLogger);
    expect(sum.rotated).toBe(1);
    // Rotation decrypt uses the PREV key for the v1 envelope.
    const decryptCall = (pool as FakePool).calls.find((c) => c.sql.includes("pgp_sym_decrypt"))!;
    expect(decryptCall.params[1]).toBe(deriveRowKeyHex("previous-master-key-32-chars-ok!!!", Buffer.alloc(16, 4)));
  });
});
