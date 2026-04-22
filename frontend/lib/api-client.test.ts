import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getStatus, toggleKillswitch } from "./api-client";

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

const VALID_STATUS = {
  ok: true,
  services: { "api-server": { ok: true, status: 200 } },
  killswitch: null,
  env: "dev",
  version: "0.1.0",
  ts: "2026-04-22T12:00:00.000Z",
};

describe("api-client — GET happy path", () => {
  beforeEach(() => vi.restoreAllMocks());
  afterEach(() => vi.restoreAllMocks());

  it("returns validated data on a well-formed 200 response", async () => {
    const fetchSpy = vi.spyOn(global, "fetch").mockResolvedValue(jsonResponse(VALID_STATUS));
    const res = await getStatus();
    expect(res.ok).toBe(true);
    if (res.ok) expect(res.data.env).toBe("dev");
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });
});

describe("api-client — validation failures", () => {
  beforeEach(() => vi.restoreAllMocks());
  afterEach(() => vi.restoreAllMocks());

  it("fails with shape-invalid error when response doesn't match schema", async () => {
    vi.spyOn(global, "fetch").mockResolvedValue(jsonResponse({ ok: "not-a-bool" }));
    const res = await getStatus();
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/edge response shape invalid/);
  });

  it("fails with invalid-JSON error when body is not JSON", async () => {
    vi.spyOn(global, "fetch").mockResolvedValue(
      new Response("not-json-at-all", { status: 200, headers: { "content-type": "text/plain" } }),
    );
    const res = await getStatus();
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/edge returned invalid JSON/);
  });
});

describe("api-client — HTTP error handling", () => {
  beforeEach(() => vi.restoreAllMocks());
  afterEach(() => vi.restoreAllMocks());

  it("returns HTTP error on 4xx without retrying", async () => {
    const fetchSpy = vi.spyOn(global, "fetch").mockResolvedValue(
      new Response("bad token", { status: 401 }),
    );
    const res = await getStatus();
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/edge HTTP 401/);
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("retries on 5xx up to default retries (2 → 3 total attempts)", async () => {
    vi.useFakeTimers();
    try {
      const fetchSpy = vi
        .spyOn(global, "fetch")
        .mockResolvedValue(new Response("boom", { status: 503 }));
      const promise = getStatus();
      await vi.runAllTimersAsync();
      const res = await promise;
      expect(res.ok).toBe(false);
      if (!res.ok) expect(res.error).toMatch(/edge HTTP 503/);
      expect(fetchSpy).toHaveBeenCalledTimes(3);
    } finally {
      vi.useRealTimers();
    }
  });

  it("recovers on first retry when 5xx becomes 200", async () => {
    vi.useFakeTimers();
    try {
      const fetchSpy = vi
        .spyOn(global, "fetch")
        .mockResolvedValueOnce(new Response("boom", { status: 502 }))
        .mockResolvedValueOnce(jsonResponse(VALID_STATUS));
      const promise = getStatus();
      await vi.runAllTimersAsync();
      const res = await promise;
      expect(res.ok).toBe(true);
      expect(fetchSpy).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("api-client — network errors / timeout", () => {
  beforeEach(() => vi.restoreAllMocks());
  afterEach(() => vi.restoreAllMocks());

  it("retries on fetch rejection and surfaces the last error", async () => {
    vi.useFakeTimers();
    try {
      const fetchSpy = vi
        .spyOn(global, "fetch")
        .mockRejectedValue(new Error("ECONNREFUSED"));
      const promise = getStatus();
      await vi.runAllTimersAsync();
      const res = await promise;
      expect(res.ok).toBe(false);
      if (!res.ok) expect(res.error).toBe("ECONNREFUSED");
      expect(fetchSpy).toHaveBeenCalledTimes(3);
    } finally {
      vi.useRealTimers();
    }
  });

  it("surfaces AbortError as a timeout message", async () => {
    vi.useFakeTimers();
    try {
      const err = new Error("The user aborted a request.");
      err.name = "AbortError";
      vi.spyOn(global, "fetch").mockRejectedValue(err);
      const promise = getStatus();
      await vi.runAllTimersAsync();
      const res = await promise;
      expect(res.ok).toBe(false);
      if (!res.ok) expect(res.error).toMatch(/edge timeout after \d+ms/);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("api-client — POST mutations do not retry", () => {
  beforeEach(() => vi.restoreAllMocks());
  afterEach(() => vi.restoreAllMocks());

  it("toggleKillswitch does not retry on 5xx (mutations must not replay)", async () => {
    const fetchSpy = vi
      .spyOn(global, "fetch")
      .mockResolvedValue(new Response("boom", { status: 503 }));
    const res = await toggleKillswitch(true, "test reason", "admin-token");
    expect(res.ok).toBe(false);
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("toggleKillswitch succeeds on 200 with validated response", async () => {
    vi.spyOn(global, "fetch").mockResolvedValue(
      jsonResponse({
        enabled: true,
        reason: "test reason",
        triggered_by: null,
        updated_at: "2026-04-22T12:00:00.000Z",
      }),
    );
    const res = await toggleKillswitch(true, "test reason", "admin-token");
    expect(res.ok).toBe(true);
    if (res.ok) expect(res.data.enabled).toBe(true);
  });
});
