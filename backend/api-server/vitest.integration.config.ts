import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Runs only the testcontainer integration tests in test/.
    // Requires Docker (to spin up postgres:15 containers) and is run
    // exclusively from the TypeScript integration CI job which provides
    // a real Postgres service + Docker on the runner.
    include: ["test/**/*.test.ts", "test/**/*.spec.ts"],
    testTimeout: 60_000,
    hookTimeout: 120_000,
  },
});
