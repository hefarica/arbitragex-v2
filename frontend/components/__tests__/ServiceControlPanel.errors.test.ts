// frontend/components/__tests__/ServiceControlPanel.errors.test.ts
//
// SERVICE-CTRL-01 (2026-09-01) regression guard: the panel must translate the
// TYPED JSON error contract of the service-control route truthfully. The prior
// panel mapped every 404 to "endpoint not yet implemented (backend Sprint
// TBD)" — an obsolete claim that masked a real production fault (socket-proxy
// DOCKER_GID drift → all six healthy services reported as unresolvable
// containers, and later as a broken control plane).
//
// Contract sources (verified in code, not inferred):
//   edge/worker/src/index.ts        → 400 invalid_action, 401 missing_admin_token
//   backend/api-server/src/routes/service-control.ts
//                                   → 501 not_implemented, 400 service_not_controllable,
//                                      404 container_not_found (+compose_project),
//                                      502 control_plane_error (+detail)
import { describe, it, expect } from "vitest";

import { describeControlFailure } from "@/components/ServiceControlPanel";

describe("describeControlFailure — typed error contract (SERVICE-CTRL-01)", () => {
  it("401 (edge, no/invalid session) points at the admin sign-in, never at the endpoint", () => {
    const msg = describeControlFailure(401, { error: "missing_admin_token" });
    expect(msg).toContain("/admin/signin");
    expect(msg).toContain("arbx_admin_session");
    expect(msg).not.toContain("not implemented");
  });

  it("401 (api-server gate) gets the same actionable treatment", () => {
    const msg = describeControlFailure(401, { error: "unauthorized", source: "admin_token" });
    expect(msg).toContain("/admin/signin");
  });

  it("400 invalid_action names the only valid actions", () => {
    const msg = describeControlFailure(400, { error: "invalid_action", valid_actions: ["start", "stop"] });
    expect(msg).toContain("start|stop");
  });

  it("400 service_not_controllable names the service and the allowlist knob", () => {
    const msg = describeControlFailure(400, { error: "service_not_controllable", service: "postgres" });
    expect(msg).toContain("postgres");
    expect(msg).toContain("ARBX_SERVICE_CONTROL_ALLOWLIST");
  });

  it("404 container_not_found explains label/project resolution — NOT 'endpoint not implemented'", () => {
    const msg = describeControlFailure(404, {
      error: "container_not_found",
      service: "searcher-rs",
      compose_project: "arbitragex-v2",
    });
    expect(msg).toContain("searcher-rs");
    expect(msg).toContain("arbitragex-v2");
    expect(msg).toContain("com.docker.compose.service");
    expect(msg).not.toContain("not implemented");
    expect(msg).not.toContain("Sprint");
  });

  it("501 not_implemented names the feature flag (off is the shadow-safe default)", () => {
    const msg = describeControlFailure(501, {
      error: "not_implemented",
      message: "service control flag off (ARBX_SERVICE_CONTROL != on)",
    });
    expect(msg).toContain("ARBX_SERVICE_CONTROL=on");
  });

  it("502 control_plane_error surfaces the backend detail (DOCKER_GID drift diagnosis)", () => {
    const msg = describeControlFailure(502, {
      error: "control_plane_error",
      service: "searcher-rs",
      detail: "socket-proxy unreachable: fetch failed: connect EACCES",
    });
    expect(msg).toContain("socket-proxy unreachable");
    expect(msg).toContain("DOCKER_GID");
  });

  it("unknown code falls back to status + raw body (never a fabricated story)", () => {
    const msg = describeControlFailure(500, { error: "something_new" });
    expect(msg).toContain("HTTP 500");
    expect(msg).toContain("something_new");
  });

  it("non-JSON body (null) still reports the status honestly", () => {
    const msg = describeControlFailure(502, null);
    expect(msg).toContain("HTTP 502");
    expect(msg).toContain("non-JSON");
  });
});
