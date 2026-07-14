import { test, expect } from "vitest";
import { Pool } from "pg";

test("018b PII helper functions exist after migration", async () => {
  const pool = new Pool({
    connectionString: process.env["DATABASE_URL"] ||
      "postgres://postgres:postgres@localhost:5432/arbitragex"
  });

  const result = await pool.query(`
    SELECT p.proname as function_name
    FROM pg_proc p
    JOIN pg_namespace n ON p.pronamespace = n.oid
    WHERE n.nspname = 'public'
      AND p.proname IN ('arbx_anonymize_ip', 'arbx_hash_user_agent')
  `);

  const functionNames = result.rows.map(r => r.function_name);
  expect(functionNames).toContain("arbx_anonymize_ip");
  expect(functionNames).toContain("arbx_hash_user_agent");

  await pool.end();
});
