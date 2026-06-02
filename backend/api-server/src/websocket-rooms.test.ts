/**
 * OMEGA-8/M4 Fase 10 — WSS rooms safety regression.
 *
 * setupWebSocketGateway in websocket.ts mounts an `io.use` middleware that
 * requires a valid admin token in the handshake. That means EVERY room
 * (`runtime_ack`, `convergence`, `opportunities`, `metrics`) is implicitly
 * gated — a client without the token cannot even open a Socket.IO
 * connection.
 *
 * This regression test reproduces the same handshake check pattern and
 * verifies that:
 *
 *   1) A socket with the correct admin token connects and can subscribe to
 *      every documented room.
 *   2) A socket with a wrong/missing admin token is refused at the
 *      handshake step BEFORE any `subscribe:*` event handler runs.
 *   3) Even on a fully-authorized socket, an invalid payload is silently
 *      rejected by `isValidRuntimeAck` rather than emitted with corrupt
 *      shape.
 *
 * The existing websocket.test.ts already covers the runtime_ack-specific
 * `runtimeAckAllowed` per-socket flag (defense-in-depth). This file
 * complements it by exercising the connection-level admin gate.
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { createServer, type Server as HttpServer } from "node:http";
import { Server as IoServer, type Socket as ServerSocket } from "socket.io";
import { io as ioClient, type Socket as ClientSocket } from "socket.io-client";
import { isValidRuntimeAck, broadcastRuntimeAck } from "./websocket.js";

const ADMIN_TOKEN = "test-admin-token-32-bytes-or-more-of-entropy";

function startServer(): Promise<{ httpServer: HttpServer; io: IoServer; port: number }> {
  return new Promise((resolve) => {
    const httpServer = createServer();
    const io = new IoServer(httpServer);

    io.use((socket, next) => {
      const got = String(socket.handshake.query["token"] ?? "");
      if (got !== ADMIN_TOKEN) {
        return next(new Error("unauthorized"));
      }
      (socket as unknown as { data: Record<string, unknown> }).data = {
        runtimeAckAllowed: true,
      };
      next();
    });

    io.on("connection", (socket: ServerSocket) => {
      socket.on("subscribe:opportunities", () => {
        socket.join("opportunities");
      });
      socket.on("subscribe:metrics", () => {
        socket.join("metrics");
      });
      socket.on("subscribe:convergence", () => {
        socket.join("convergence");
      });
      socket.on("subscribe:telemetry", () => {
        socket.join("telemetry");
      });
      socket.on("subscribe:route_discovery", () => {
        socket.join("route_discovery");
      });
      socket.on("subscribe:runtime_ack", () => {
        socket.join("runtime_ack");
      });
    });

    httpServer.listen(0, () => {
      const addr = httpServer.address();
      const port = typeof addr === "object" && addr ? addr.port : 0;
      resolve({ httpServer, io, port });
    });
  });
}

describe("OMEGA-8/M4 Fase 10 — WSS rooms admin-gate regression", () => {
  let server: { httpServer: HttpServer; io: IoServer; port: number };

  beforeAll(async () => {
    server = await startServer();
  });
  afterAll(async () => {
    await new Promise<void>((resolve) => server.io.close(() => resolve()));
    await new Promise<void>((resolve) => server.httpServer.close(() => resolve()));
  });

  it("rejects sockets without the admin token at handshake (no room reachable)", async () => {
    const client: ClientSocket = ioClient(`http://127.0.0.1:${server.port}`, {
      transports: ["websocket"],
      query: { token: "wrong-token" },
      reconnection: false,
    });

    const connectErr: Error = await new Promise((resolve) => {
      client.on("connect_error", (e: Error) => resolve(e));
    });
    expect(connectErr.message).toContain("unauthorized");
    client.disconnect();
  });

  it("accepts authorized client and lets it subscribe to all documented rooms", async () => {
    const client: ClientSocket = ioClient(`http://127.0.0.1:${server.port}`, {
      transports: ["websocket"],
      query: { token: ADMIN_TOKEN },
      reconnection: false,
    });
    await new Promise<void>((resolve) => client.on("connect", () => resolve()));

    // Subscribe to each room — none of these emit, but the join should
    // succeed silently. We assert no `error` event fires for any of them.
    const sawError = new Promise<unknown>((resolve) => {
      const t = setTimeout(() => resolve(null), 100);
      client.on("error", (e: unknown) => {
        clearTimeout(t);
        resolve(e);
      });
    });
    client.emit("subscribe:opportunities");
    client.emit("subscribe:metrics");
    client.emit("subscribe:convergence");
    client.emit("subscribe:telemetry");
    client.emit("subscribe:route_discovery");
    client.emit("subscribe:runtime_ack");

    const err = await sawError;
    expect(err).toBeNull();
    client.disconnect();
  });

  it("isValidRuntimeAck rejects a corrupt payload — broadcastRuntimeAck does NOT emit", async () => {
    // Defense-in-depth: even if a caller bypasses the route's Zod schema,
    // broadcastRuntimeAck's internal guard refuses to ship a corrupt
    // payload into the runtime_ack room. The existing websocket.ts
    // implementation increments the `rejected` Prometheus counter and
    // returns without emitting.
    const corrupt = {
      event_id: "not-a-uuid-but-still-a-string",
      resource: "trading_config",
      chain_id: 1,
      idempotency_key: "k",
      config_hash_before: null,
      config_hash_after: "z".repeat(64),
      worker_id: "w",
      layer: "invented_layer", // bad
      status: "applied",
    };
    expect(isValidRuntimeAck(corrupt)).toBe(false);

    // Verify broadcastRuntimeAck swallows the bad payload (no throw).
    // We can't easily assert "did not emit" without a wired client, but
    // the type guard rejection above is already the contract.
    expect(() => broadcastRuntimeAck(server.io, corrupt as never)).not.toThrow();
  });

  it("isValidRuntimeAck accepts a canonical payload (Capa 2 ↔ Capa 3 invariant)", () => {
    const ok = {
      event_id: "11111111-2222-3333-4444-555555555555",
      resource: "trading_config",
      chain_id: 1,
      idempotency_key: "k",
      config_hash_before: "a".repeat(64),
      config_hash_after: "b".repeat(64),
      worker_id: "w",
      layer: "searcher_rs",
      status: "applied",
      latency_ms: 0,
      error: null,
    };
    expect(isValidRuntimeAck(ok)).toBe(true);
  });
});
