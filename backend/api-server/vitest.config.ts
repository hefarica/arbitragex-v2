import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // test/ contains testcontainer integration tests that require Docker and
    // a full Postgres lifecycle (Docker postgres:15 init restarts mid-sequence).
    // They run under the dedicated TypeScript integration CI job (which provides
    // a real Postgres service + Docker). Excluding them here keeps `npm test`
    // (used by lint-and-test-node) to unit tests only.
    include: ["src/**/*.test.ts", "src/**/*.spec.ts"],
    exclude: ["test/**", "node_modules/**"],
  },
});
