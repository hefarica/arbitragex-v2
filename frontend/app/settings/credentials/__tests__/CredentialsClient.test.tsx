/**
 * MC-CRED-1 — regression gate for the /settings/credentials mixed-content
 * incident (2026-08-16).
 *
 * What shipped broken: the Server Component resolved getApiBaseUrl() in SSR
 * context (INTERNAL_EDGE_URL = http://edge:8787, a docker-internal host) and
 * passed it as an `edgeUrl` prop into this client. The browser then fetch()ed
 * http://edge:8787/... from an HTTPS page → Mixed Content block, page unable
 * to list / test / save / delete credentials.
 *
 * What these tests guard (same toolkit-alignment constraint as
 * ChainsAdmin.test.tsx: renderToStaticMarkup, no jsdom):
 *   - The client never receives an SSR-computed base URL prop.
 *   - Every fetch URL is built via getApiBaseUrl() at call time, which returns
 *     "" in the browser → same-origin requests.
 *   - The SSR fetch sends the V-AT-2 admin headers (runtime token) so the
 *     admin-gated list authenticates without a browser session.
 *   - Rendered markup never contains an absolute internal URL.
 */
import React from "react";
import { describe, it, expect, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: () => {} }),
}));
vi.mock("sonner", () => ({
  toast: Object.assign(() => {}, {
    error: () => {},
    success: () => {},
    message: () => {},
  }),
}));
vi.mock("@/lib/admin-token", () => ({
  hasAdminSession: () => false,
}));
vi.mock("@/store/useSystemStore", () => ({
  rehydrateSystemStore: () => {},
  useSystemStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ topology: { activeChains: [] }, setCredentialStatus: () => {} }),
}));

import { CredentialsClient } from "../CredentialsClient";

const here = path.dirname(fileURLToPath(import.meta.url));
const clientSrc = readFileSync(path.join(here, "../CredentialsClient.tsx"), "utf8");
const pageSrc = readFileSync(path.join(here, "../page.tsx"), "utf8");

describe("CredentialsClient — MC-CRED-1 source gates", () => {
  it("never receives an SSR-computed edgeUrl prop (mixed-content root cause)", () => {
    expect(clientSrc).not.toMatch(/\bedgeUrl\b/);
    expect(pageSrc).not.toMatch(/\bedgeUrl\s*=/);
  });

  it("builds every fetch URL via getApiBaseUrl() (same-origin in the browser)", () => {
    expect(clientSrc).toContain("${getApiBaseUrl()}/admin/credentials/test");
    expect(clientSrc).toContain("${getApiBaseUrl()}/admin/credentials");
    expect(clientSrc).toContain("${getApiBaseUrl()}/api/credentials");
  });

  it("SSR snapshot fetch sends the V-AT-2 admin headers", () => {
    expect(pageSrc).toContain("x-arbx-admin-token");
    expect(pageSrc).toContain("x-arbx-edge-token");
  });
});

describe("CredentialsClient — SSR markup", () => {
  it("renders the credentials surface from a server snapshot", () => {
    const html = renderToStaticMarkup(
      <CredentialsClient
        initialSnapshot={{ items: [], ts: "2026-08-16T00:00:00.000Z", error: null }}
      />,
    );
    expect(html).toContain("Credentials");
    // R8 fail-honest: the incident URL must never leak into markup.
    expect(html).not.toContain("http://edge");
    expect(html).not.toContain(":8787");
  });

  it("surfaces the snapshot error verbatim (fail-honest)", () => {
    const html = renderToStaticMarkup(
      <CredentialsClient
        initialSnapshot={{ items: [], ts: "2026-08-16T00:00:00.000Z", error: "HTTP 401" }}
      />,
    );
    expect(html).toContain("HTTP 401");
  });
});
