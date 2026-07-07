import { defineConfig } from "vitest/config";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  resolve: {
    alias: {
      "@": ROOT,
    },
    // Force a single React/react-dom instance. Without this, pre-bundling
    // radix-ui (below) pulls in a second react-dom copy whose React internals
    // (ReactCurrentDispatcher) are disconnected from the test's react-dom/server.
    dedupe: ["react", "react-dom"],
  },
  test: {
    environment: "node",
    include: [
      "lib/**/*.test.ts",
      "components/**/*.test.{ts,tsx}",
      "features/**/*.test.{ts,tsx}",
      "app/**/*.test.{ts,tsx}",
    ],
    exclude: ["node_modules", ".next", "dist"],
    setupFiles: ["./vitest.setup.ts"],
    // Barrel packages must be pre-bundled by esbuild, otherwise vitest's
    // node/SSR transformer processes them on demand and breaks:
    //   - lucide-react: ~1.5k-module ESM barrel crawled file-by-file → stalls
    //     collection indefinitely (observed as a 6h CI hang) for any test
    //     importing an icon.
    //   - radix-ui: unified primitives barrel whose internal Primitive/Slot
    //     composition is mis-resolved by the SSR transformer, so SSR-rendering
    //     a Radix component (e.g. <Progress/>) throws "Objects are not valid as
    //     a React child".
    // Pre-bundling resolves each barrel in a single fast, correct pass.
    deps: {
      optimizer: {
        ssr: {
          enabled: true,
          include: ["lucide-react", "radix-ui"],
        },
      },
    },
  },
});
