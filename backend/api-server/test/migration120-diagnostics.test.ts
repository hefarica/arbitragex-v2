import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { GenericContainer, Wait, type StartedTestContainer } from "testcontainers";

// Executes the real migration only in newly created containers, never DATABASE_URL.
// No production secrets or signing material are read or injected.
const migration = readFileSync(new URL("../../../database/migrations/120_service_credentials_envelope_encryption.sql", import.meta.url), "utf8");
const previousEcho = "\\set arbx_credentials_master_key_version 1\n" +
  "\\echo '120: decrypt roundtrip verified for version :'arbx_credentials_master_key_version'\n";

for (const version of [15, 16]) {
  describe(`migration 120 psql diagnostics on PostgreSQL ${version}`, () => {
    let container: StartedTestContainer;
    const args = ["psql", "-X", "-U", "postgres", "-d", "migration_fixture", "-v", "ON_ERROR_STOP=1"];
    beforeAll(async () => {
      container = await new GenericContainer(`postgres:${version}`)
        .withEnvironment({ POSTGRES_PASSWORD: "disposable-fixture", POSTGRES_DB: "migration_fixture" })
        .withWaitStrategy(Wait.forLogMessage("database system is ready to accept connections", 2))
        .start();
      await container.copyContentToContainer([
        { content: migration, target: "/tmp/migration120.sql" },
        { content: previousEcho, target: "/tmp/previous-echo.sql" },
      ]);
      const setup = await container.exec([...args, "-c", "CREATE EXTENSION pgcrypto; CREATE TABLE service_credentials (secret_value text); INSERT INTO service_credentials VALUES ('laboratory-fixture');"]);
      expect(setup.exitCode).toBe(0);
    }, 90000);
    afterAll(async () => { await container?.stop(); });

    it("reproduces the previous client-side unterminated quote error", async () => {
      const result = await container.exec([...args, "-f", "/tmp/previous-echo.sql"]);
      expect(result.stderr).toContain("unterminated quoted string");
    });
    it("applies the complete keyless migration twice with no psql errors", async () => {
      for (let attempt = 0; attempt < 2; attempt++) {
        const result = await container.exec([...args, "-f", "/tmp/migration120.sql"]);
        expect(result.exitCode, result.output).toBe(0);
        expect(result.stderr).not.toMatch(/(?:error|fatal|unterminated)/i);
        expect(result.stdout).toContain("120: decrypt roundtrip verified for version 1");
      }
      const result = await container.exec([...args, "-At", "-c", "SELECT count(*) FROM service_credentials WHERE secret_value='laboratory-fixture' AND secret_ciphertext IS NULL AND secret_salt IS NULL AND secret_key_version IS NULL;"]);
      expect(result.exitCode).toBe(0);
      expect(result.stdout.trim()).toBe("1");
    });
  });
}
