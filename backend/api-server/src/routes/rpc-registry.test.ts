import { describe, it, expect } from "vitest";

import { RpcDescriptor } from "../lib/registry-engine.js";
import { mountRpcRegistry } from "./rpc-registry.js";

// The import endpoint validates every row with RpcDescriptor.schema before any
// DB write — these guard that contract (the DB-integration path is verified live
// post-deploy via GET /api/v1/rpc/status + a real catalog import).
describe("rpc import row validation (RpcDescriptor.schema)", () => {
  it("accepts a valid catalog row and applies defaults", () => {
    const r = RpcDescriptor.schema.safeParse({
      chain_id: 1,
      url: "https://eth.drpc.org",
      transport: "https",
    });
    expect(r.success).toBe(true);
    if (r.success) {
      expect(r.data.tier).toBe("primary");
      expect(r.data.weight).toBe(100);
      expect(r.data.max_concurrency).toBe(16);
      expect(r.data.enabled).toBe(true);
      expect(r.data.status).toBe("pending_validation");
      expect(r.data.auth_kind).toBe("none");
    }
  });

  it("rejects a malformed url", () => {
    expect(RpcDescriptor.schema.safeParse({ chain_id: 1, url: "not-a-url", transport: "https" }).success).toBe(false);
  });

  it("rejects an unknown transport", () => {
    expect(RpcDescriptor.schema.safeParse({ chain_id: 1, url: "https://x.io", transport: "carrier-pigeon" }).success).toBe(false);
  });

  it("rejects a non-positive chain_id", () => {
    expect(RpcDescriptor.schema.safeParse({ chain_id: 0, url: "https://x.io", transport: "https" }).success).toBe(false);
  });

  it("rejects an out-of-range weight", () => {
    expect(
      RpcDescriptor.schema.safeParse({ chain_id: 1, url: "https://x.io", transport: "https", weight: 99999 }).success,
    ).toBe(false);
  });
});

describe("mountRpcRegistry", () => {
  it("is a mountable function", () => {
    expect(typeof mountRpcRegistry).toBe("function");
  });
});
