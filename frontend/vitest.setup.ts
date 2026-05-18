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

// Mock lucide-react to prevent React child errors in SSR
vi.mock("lucide-react", () => {
  return new Proxy({}, {
    get: () => {
      return (props: any) => React.createElement("svg", props, React.createElement("path"));
    }
  });
});