import { vi } from "vitest";
import React from "react";

// Mock next/link to avoid useContext errors during SSR tests
vi.mock("next/link", () => ({
  default: ({ children, href, ...props }: any) => {
    return React.createElement("a", { href, ...props }, children);
  }
}));

// Mock next/navigation
vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), prefetch: vi.fn() }),
  usePathname: () => "/",
  useSearchParams: () => new URLSearchParams(),
}));

// NOTE: lucide-react is intentionally NOT vi.mock()'d here. Its ~1.5k-module
// ESM barrel stalls vitest's node/SSR transform indefinitely (6h CI hang) for
// any test importing an icon, and a runtime vi.mock makes it worse — vi.mock
// resolves the bare specifier to the real package and crawls the barrel before
// the mock can apply. Instead it is pre-bundled by esbuild via
// `test.deps.optimizer.ssr` in vitest.config.ts, which resolves the barrel in
// one fast pass and lets the real icons render.