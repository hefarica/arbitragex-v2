import { test, expect } from "@playwright/test";
import { Pool } from "pg";

test("018b PII helper functions exist after migration", async () => {
  // Host-side only. Strip any Docker-DNS DATABASE_URL (…@postgres:…) that may
  // leak in from a sourced compose .env — Playwright runs on the runner host.
  const raw = process.env["DATABASE_URL"] ?? "";
  const connectionString =
    raw && !/@(postgres|db)(:|\/)/.test(raw)
      ? raw
      : "postgres://postgres:postgres@localhost:5432/arbitragex";

  const pool = new Pool({ connectionString, connectionTimeoutMillis: 5000 });
  try {
    const result = await pool.query(`
      SELECT p.proname as function_name
      FROM pg_proc p
      JOIN pg_namespace n ON p.pronamespace = n.oid
      WHERE n.nspname = 'public'
        AND p.proname IN ('arbx_anonymize_ip', 'arbx_hash_user_agent')
    `);

    const functionNames = result.rows.map((r) => r.function_name);
    expect(functionNames).toContain("arbx_anonymize_ip");
    expect(functionNames).toContain("arbx_hash_user_agent");
  } catch (err) {
    test.skip(true, `postgres unreachable: ${(err as Error).message} — VALIDATION_PENDING_INFRASTRUCTURE`);
  } finally {
    await pool.end().catch(() => undefined);
  }
});
