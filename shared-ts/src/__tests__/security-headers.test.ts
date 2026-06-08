import { describe, it, expect } from "vitest";
import express from "express";
import http from "node:http";
import { AddressInfo } from "node:net";
import { securityHeadersMiddleware } from "../middleware/index.js";

/**
 * OMEGA-8/M4 Fase 7 — verifies that the security-headers middleware emits the
 * institutional set of HTTP security headers and that the Socket.IO ws:/wss:
 * upgrade path remains allowed by the default CSP. No real socket is opened —
 * we assert the CSP string is permissive enough to allow ws:/wss: in
 * connect-src.
 */

async function withApp<T>(
  setup: (app: express.Express) => void,
  run: (origin: string) => Promise<T>,
): Promise<T> {
  const app = express();
  setup(app);
  app.get("/probe", (_req, res) => res.status(200).json({ ok: true }));
  const server = http.createServer(app);
  await new Promise<void>((resolve) => server.listen(0, resolve));
  const { port } = server.address() as AddressInfo;
  try {
    return await run(`http://127.0.0.1:${port}`);
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
}

describe("securityHeadersMiddleware", () => {
  it("emits the required hardening headers on every response", async () => {
    await withApp(
      (app) => app.use(securityHeadersMiddleware()),
      async (origin) => {
        const res = await fetch(`${origin}/probe`);
        expect(res.status).toBe(200);
        expect(res.headers.get("x-content-type-options")).toBe("nosniff");
        expect(res.headers.get("x-frame-options")).toBe("DENY");
        expect(res.headers.get("referrer-policy")).toBe("no-referrer");
        expect(res.headers.get("cross-origin-resource-policy")).toBe("same-site");
        expect(res.headers.get("cross-origin-opener-policy")).toBe("same-origin");
        const csp = res.headers.get("content-security-policy");
        expect(csp).toBeTruthy();
        expect(csp).toContain("default-src 'self'");
        expect(csp).toContain("frame-ancestors 'none'");
        expect(csp).toContain("object-src 'none'");
      },
    );
  });

  it("CSP allows ws: and wss: in connect-src so Socket.IO still works", async () => {
    await withApp(
      (app) => app.use(securityHeadersMiddleware()),
      async (origin) => {
        const res = await fetch(`${origin}/probe`);
        const csp = res.headers.get("content-security-policy") ?? "";
        // The CSP must whitelist ws:/wss: in connect-src, otherwise the
        // browser will refuse the Socket.IO upgrade. Use a strict substring
        // match — these tokens are not implied by 'self'.
        expect(csp).toContain("connect-src 'self' ws: wss:");
      },
    );
  });

  it("HSTS is OFF by default (api-server runs behind plain HTTP inside VPS)", async () => {
    await withApp(
      (app) => app.use(securityHeadersMiddleware()),
      async (origin) => {
        const res = await fetch(`${origin}/probe`);
        // HSTS on plain HTTP would break browser access; the middleware
        // must NEVER emit it unless explicitly opted in.
        expect(res.headers.get("strict-transport-security")).toBeNull();
      },
    );
  });

  it("HSTS is emitted when enableHsts=true is passed explicitly", async () => {
    await withApp(
      (app) => app.use(securityHeadersMiddleware({ enableHsts: true })),
      async (origin) => {
        const res = await fetch(`${origin}/probe`);
        const hsts = res.headers.get("strict-transport-security");
        expect(hsts).toContain("max-age=63072000");
        expect(hsts).toContain("includeSubDomains");
      },
    );
  });

  it("cspExtras appends to the base CSP without replacing it", async () => {
    await withApp(
      (app) =>
        app.use(
          securityHeadersMiddleware({
            cspExtras: "report-uri /__csp;",
          }),
        ),
      async (origin) => {
        const res = await fetch(`${origin}/probe`);
        const csp = res.headers.get("content-security-policy") ?? "";
        expect(csp).toContain("default-src 'self'");
        expect(csp).toContain("report-uri /__csp;");
      },
    );
  });
});
