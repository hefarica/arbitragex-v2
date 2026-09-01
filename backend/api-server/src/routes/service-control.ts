/**
 * Service control plane — start/stop managed docker containers.
 *
 * Replaces the A8 501 stubs (stubs.ts L112-137). Mounted in index.ts BEFORE
 * mountStubs so Express dispatches these real handlers (the L562 comment
 * confirms earlier mounts shadow the bottom stubs). Admin-token gated,
 * allowlist-restricted, audit-logged, and FEATURE-FLAGGED OFF by default
 * (`ARBX_SERVICE_CONTROL !== "on"` → 501) so shadow/paper systems stay safe.
 *
 * The api-server NEVER touches the raw docker socket. It talks to the
 * least-privilege socket-proxy sidecar (docker/socket-proxy/server.js) which
 * allows ONLY list/inspect/start/stop. The :name param is validated against
 * ^[a-z0-9-]+$ AND an operator allowlist, so no docker-API path injection is
 * possible even if the proxy were misconfigured.
 *
 * R8 fail-honest: flag off / unknown service / proxy error all return explicit
 * JSON errors — never a fabricated success.
 *
 * Routes:
 *   POST /api/v1/admin/services/:name/start
 *   POST /api/v1/admin/services/:name/stop
 */
import { Router, type Request, type Response } from "express";

interface Deps {
  requireAdminToken: (expected: string) => import("express").RequestHandler;
  adminToken: string;
  writeAudit: (
    action: string,
    actor: string,
    targetKind: string | null,
    targetId: string | null,
    before: unknown,
    after: unknown,
    ip: string | null,
    traceId: string | null,
    userAgent?: string | null,
  ) => Promise<void>;
  reqUA: (req: Request) => string | null;
  logger: {
    warn: (obj: object, msg?: string) => void;
    error: (obj: object, msg?: string) => void;
  };
}

const NAME_RE = /^[a-z0-9-]+$/;

/**
 * Resolution outcome. SERVICE-CTRL-01 (2026-09-01): production returned
 * 404 container_not_found for six healthy services because the socket-proxy
 * could not reach the docker daemon (DOCKER_GID drift) — the old code conflated
 * "proxy unreachable/errored" with "container genuinely absent" and the UI then
 * told the operator the endpoint was "not implemented". R8 fail-honest: only a
 * healthy proxy answering 404 may claim not_found; any proxy failure is 502.
 */
type Resolution =
  | { kind: "container"; id: string; state: string }
  | { kind: "not_found" }
  | { kind: "proxy_error"; detail: string };

export function buildServiceControlRouter(deps: Deps): Router {
  const { requireAdminToken, adminToken, writeAudit, reqUA, logger } = deps;

  // Read env at BUILD time (not module load) so tests can set process.env
  // before constructing the app.
  const proxy = process.env["DOCKER_PROXY_URL"] ?? "http://socket-proxy:2375";
  const composeProject = process.env["COMPOSE_PROJECT_NAME"] ?? "arbitragex-v2";
  const allowlist = new Set(
    (process.env["ARBX_SERVICE_CONTROL_ALLOWLIST"] ??
      "searcher-rs,sim-ctl,relays-client,recon,token-enricher")
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean),
  );

  /** Resolve a compose service → container via the label filter (robust to
   * project-name changes); fall back to the deterministic <project>-<svc>-1 name.
   * A 404 from the daemon is the ONLY evidence of not_found — every other
   * failure (network throw, proxy 4xx/5xx) is reported as proxy_error so the
   * caller surfaces 502 control_plane_error instead of a false 404. */
  async function resolveContainer(name: string): Promise<Resolution> {
    const filters = encodeURIComponent(
      JSON.stringify({
        label: [
          `com.docker.compose.service=${name}`,
          `com.docker.compose.project=${composeProject}`,
        ],
      }),
    );
    // Pass 1: label filter.
    try {
      const r = await fetch(`${proxy}/containers/json?all=true&filters=${filters}`);
      if (r.ok) {
        const arr = (await r.json().catch(() => null)) as Array<{ Id: string; State: string }> | null;
        if (Array.isArray(arr) && arr.length > 0 && arr[0]) {
          return { kind: "container", id: arr[0].Id, state: arr[0].State };
        }
        // 200 + empty list: healthy proxy, no labeled match → try the name fallback.
      }
      // Non-ok list (proxy refused / daemon error): the inspect below decides
      // whether the proxy is alive (404 → not_found) or broken (→ proxy_error).
    } catch {
      // Network-level failure on the list call — same arbitration via inspect.
    }
    // Pass 2: deterministic <project>-<svc>-1 inspect.
    const fallback = `${composeProject}-${name}-1`;
    let inspectStatus: number | null = null;
    try {
      const ir = await fetch(`${proxy}/containers/${fallback}/json`);
      inspectStatus = ir.status;
      if (ir.ok) {
        const j = (await ir.json().catch(() => null)) as { Id?: string; State?: { Status?: string } } | null;
        if (j?.Id) return { kind: "container", id: j.Id, state: j.State?.Status ?? "unknown" };
        return { kind: "proxy_error", detail: "socket-proxy inspect response had no container id" };
      }
    } catch (e) {
      return { kind: "proxy_error", detail: `socket-proxy unreachable: ${(e as Error).message}` };
    }
    if (inspectStatus === 404) return { kind: "not_found" };
    return {
      kind: "proxy_error",
      detail: `socket-proxy list+inspect failed (inspect HTTP ${inspectStatus ?? "n/a"})`,
    };
  }

  const handle = (action: "start" | "stop") =>
    async (req: Request, res: Response): Promise<void> => {
      const name = String(req.params.name ?? "");

      // 1. Feature flag — OFF by default (shadow-safe).
      if (process.env["ARBX_SERVICE_CONTROL"] !== "on") {
        res
          .status(501)
          .json({ error: "not_implemented", message: "service control flag off (ARBX_SERVICE_CONTROL != on)" });
        return;
      }

      // 2. Validate name (no path injection into the docker URL) + allowlist.
      if (!NAME_RE.test(name) || !allowlist.has(name)) {
        res.status(400).json({ error: "service_not_controllable", service: name });
        return;
      }

      const actor = req.header("x-arbx-actor") ?? "admin";
      const ip = req.ip ?? req.socket.remoteAddress ?? null;
      const traceId = (req as Request & { traceId?: string }).traceId ?? null;

      try {
        const target = await resolveContainer(name);
        if (target.kind === "proxy_error") {
          logger.error(
            { event: "service_control.resolve_proxy_error", service: name, action, detail: target.detail },
            "socket-proxy resolution failed",
          );
          res.status(502).json({ error: "control_plane_error", service: name, detail: target.detail });
          return;
        }
        if (target.kind === "not_found") {
          // compose_project in the body lets the operator diff it against the
          // container labels without shell access (SERVICE-CTRL-01 diagnosis).
          res.status(404).json({ error: "container_not_found", service: name, compose_project: composeProject });
          return;
        }

        const before = target.state;
        const path =
          action === "start"
            ? `${proxy}/containers/${target.id}/start`
            : `${proxy}/containers/${target.id}/stop?t=10`;

        const pr = await fetch(path, { method: "POST" });
        // 204 = applied; 304 = already in target state (idempotent success).
        if (pr.status !== 204 && pr.status !== 304) {
          logger.error(
            { event: "service_control.proxy_error", service: name, action, status: pr.status },
            "socket-proxy returned non-success",
          );
          res.status(502).json({ error: "control_plane_error", service: name });
          return;
        }

        const after = action === "start" ? "running" : "exited";
        await writeAudit(
          `service.${action}`,
          actor,
          "service",
          name,
          before,
          after,
          ip,
          traceId,
          reqUA(req),
        );

        res.status(200).json({ service: name, action, status: after });
      } catch (e) {
        logger.error(
          { event: "service_control.failure", service: name, action, err: (e as Error).message },
          "service control failed",
        );
        res.status(502).json({ error: "control_plane_error", service: name });
      }
    };

  const router = Router();
  router.post("/api/v1/admin/services/:name/start", requireAdminToken(adminToken), handle("start"));
  router.post("/api/v1/admin/services/:name/stop", requireAdminToken(adminToken), handle("stop"));
  return router;
}
