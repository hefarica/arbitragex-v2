/**
 * OMEGA-8 / M3 FASE 2 — P0-2: WSS room `runtime_ack` gating.
 *
 * Tests the defense-in-depth check on `subscribe:runtime_ack`. The
 * handshake `io.use` middleware in setupWebSocketGateway already requires a
 * valid admin token; this test exercises the per-room capability flag layer
 * that fires AFTER the handshake. Two scenarios:
 *
 *   1) Authorized: socket carries the `runtimeAckAllowed` flag → join
 *      succeeds, broadcast is received.
 *   2) Unauthorized: socket lacks the flag (simulated by bypassing the
 *      handshake setter) → join is refused, an `error` event with code
 *      `unauthorized` is emitted to that socket only.
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { createServer, type Server as HttpServer } from "node:http";
import { Server as IoServer, type Socket as ServerSocket } from "socket.io";
import { io as ioClient, type Socket as ClientSocket } from "socket.io-client";
import { broadcastRuntimeAck, RUNTIME_ACK_ROOM } from "./websocket.js";
import type { RuntimeAckBroadcast } from "./websocket.js";

const PAYLOAD: RuntimeAckBroadcast = {
  event_id: "33333333-3333-3333-3333-333333333333",
  resource: "trading_config",
  chain_id: 1,
  idempotency_key: "idem-test",
  config_hash_before: "a".repeat(64),
  config_hash_after: "b".repeat(64),
  worker_id: "test-worker",
  layer: "searcher_rs",
  status: "applied",
  latency_ms: 10,
  error: null,
};

/**
 * Spin up a minimal Socket.IO server that mirrors the production gating
 * logic for `subscribe:runtime_ack` without requiring the full setup
 * (boot validator, env vars). We replicate the handler verbatim so this
 * test exercises the same control flow.
 */
function startServer(): Promise<{ httpServer: HttpServer; io: IoServer; port: number }> {
  return new Promise((resolve) => {
    const httpServer = createServer();
    const io = new IoServer(httpServer);

    io.use((socket, next) => {
      // Mark every socket as authorized — the test toggles the flag below
      // by setting `socket.data.runtimeAckAllowed` from a custom handshake
      // query param. This mirrors the production handshake gate.
      const allowed = socket.handshake.query["allowed"] === "true";
      (socket as unknown as { data: Record<string, unknown> }).data = {
        runtimeAckAllowed: allowed,
      };
      next();
    });

    io.on("connection", (socket: ServerSocket) => {
      // ROOM-AUTH-01 (2026-09-17): replica updated in lockstep with the
      // production handler — the join verdict also rides the ack callback.
      socket.on("subscribe:runtime_ack", (ack?: (res: { ok: boolean; code?: string }) => void) => {
        const allowed = (socket.data as { runtimeAckAllowed?: boolean })?.runtimeAckAllowed === true;
        const reply = typeof ack === "function" ? ack : undefined;
        if (!allowed) {
          socket.emit("error", { code: "unauthorized", room: RUNTIME_ACK_ROOM });
          reply?.({ ok: false, code: "unauthorized" });
          return;
        }
        socket.join(RUNTIME_ACK_ROOM);
        reply?.({ ok: true });
      });
    });

    httpServer.listen(0, () => {
      const addr = httpServer.address();
      const port = typeof addr === "object" && addr ? addr.port : 0;
      resolve({ httpServer, io, port });
    });
  });
}

describe("OMEGA-8/M3 P0-2: WSS runtime_ack gating", () => {
  let server: { httpServer: HttpServer; io: IoServer; port: number };

  beforeAll(async () => {
    server = await startServer();
  });

  afterAll(async () => {
    server.io.close();
    server.httpServer.close();
  });

  it("authorized client joins runtime_ack and receives broadcast", async () => {
    const client = ioClient(`http://localhost:${server.port}`, {
      query: { allowed: "true" },
      transports: ["websocket"],
      forceNew: true,
    });
    await new Promise<void>((resolve) => client.on("connect", () => resolve()));

    client.emit("subscribe:runtime_ack");
    // Give the server a tick to process the join.
    await new Promise((r) => setTimeout(r, 50));

    const received = new Promise<RuntimeAckBroadcast>((resolve) => {
      client.on("runtime_ack", (data: RuntimeAckBroadcast) => resolve(data));
    });

    broadcastRuntimeAck(server.io, PAYLOAD);

    const got = await Promise.race([
      received,
      new Promise<null>((r) => setTimeout(() => r(null), 500)),
    ]);
    expect(got).not.toBeNull();
    expect((got as RuntimeAckBroadcast).event_id).toBe(PAYLOAD.event_id);

    client.disconnect();
  });

  it("unauthorized client receives error and does NOT receive broadcast", async () => {
    const client = ioClient(`http://localhost:${server.port}`, {
      query: { allowed: "false" },
      transports: ["websocket"],
      forceNew: true,
    });
    await new Promise<void>((resolve) => client.on("connect", () => resolve()));

    const errorReceived = new Promise<{ code: string; room: string }>((resolve) => {
      client.on("error", (data: { code: string; room: string }) => resolve(data));
    });
    const broadcastReceived = new Promise<RuntimeAckBroadcast | null>((resolve) => {
      client.on("runtime_ack", (data: RuntimeAckBroadcast) => resolve(data));
      setTimeout(() => resolve(null), 300);
    });

    client.emit("subscribe:runtime_ack");

    const err = await Promise.race([
      errorReceived,
      new Promise<null>((r) => setTimeout(() => r(null), 500)),
    ]);
    expect(err).not.toBeNull();
    expect((err as { code: string; room: string }).code).toBe("unauthorized");
    expect((err as { code: string; room: string }).room).toBe(RUNTIME_ACK_ROOM);

    // Emit a broadcast — the unauthorized client (not in room) MUST NOT receive it.
    broadcastRuntimeAck(server.io, PAYLOAD);
    const got = await broadcastReceived;
    expect(got).toBeNull();

    client.disconnect();
  });

  // ROOM-AUTH-01 (2026-09-17): the join verdict must reach the client via the
  // ack callback so the frontend can certify the channel from the server's
  // decision (this is what ends the `LIVE over unauthorized join` mislabel).
  it("join verdict rides the ack callback for both authorized and unauthorized clients", async () => {
    const authorized = ioClient(`http://localhost:${server.port}`, {
      query: { allowed: "true" },
      transports: ["websocket"],
      forceNew: true,
    });
    await new Promise<void>((resolve) => authorized.on("connect", () => resolve()));
    const okVerdict = new Promise<{ ok: boolean; code?: string }>((resolve) => {
      authorized.emit("subscribe:runtime_ack", (res: { ok: boolean; code?: string }) => resolve(res));
    });
    const ok = await Promise.race([
      okVerdict,
      new Promise<null>((r) => setTimeout(() => r(null), 500)),
    ]);
    expect(ok).not.toBeNull();
    expect((ok as { ok: boolean }).ok).toBe(true);
    expect((ok as { code?: string }).code).toBeUndefined();
    authorized.disconnect();

    const anonymous = ioClient(`http://localhost:${server.port}`, {
      query: { allowed: "false" },
      transports: ["websocket"],
      forceNew: true,
    });
    await new Promise<void>((resolve) => anonymous.on("connect", () => resolve()));
    const nackVerdict = new Promise<{ ok: boolean; code?: string }>((resolve) => {
      anonymous.emit("subscribe:runtime_ack", (res: { ok: boolean; code?: string }) => resolve(res));
    });
    const nack = await Promise.race([
      nackVerdict,
      new Promise<null>((r) => setTimeout(() => r(null), 500)),
    ]);
    expect(nack).not.toBeNull();
    expect((nack as { ok: boolean }).ok).toBe(false);
    expect((nack as { code?: string }).code).toBe("unauthorized");
    anonymous.disconnect();
  });

  // ROOM-AUTH-01 (2026-09-17): legacy clients emit without a callback — the
  // handler must tolerate that (no crash, error event still emitted).
  it("legacy emit without ack callback is tolerated", async () => {
    const client = ioClient(`http://localhost:${server.port}`, {
      query: { allowed: "false" },
      transports: ["websocket"],
      forceNew: true,
    });
    await new Promise<void>((resolve) => client.on("connect", () => resolve()));
    const errorReceived = new Promise<{ code: string; room: string }>((resolve) => {
      client.on("error", (data: { code: string; room: string }) => resolve(data));
    });
    client.emit("subscribe:runtime_ack"); // no callback — pre-ROOM-AUTH-01 shape
    const err = await Promise.race([
      errorReceived,
      new Promise<null>((r) => setTimeout(() => r(null), 500)),
    ]);
    expect(err).not.toBeNull();
    expect((err as { code: string }).code).toBe("unauthorized");
    client.disconnect();
  });
});
