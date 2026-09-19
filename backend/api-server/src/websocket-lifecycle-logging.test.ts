/**
 * WO-NO-WS-LOGS-01 (2026-09-17) — structured socket.io lifecycle logging.
 *
 * QA-WS §5.3 could not confirm a WebSocket connection server-side from
 * `docker logs` (grep ws/socket = 0 lines in 400): the gateway only logged
 * bare `console.log("[WebSocket] …")` lines, and those were drowned by the
 * paper_archiver.skip_rejected info-flood (~30 lines/s, QA-WS §5.4 / R9).
 *
 * This test spins up the REAL `setupWebSocketGateway` with an injected
 * structural logger and pins the contract the fix promises:
 *
 *   1) `ws.connected` at INFO with socket_id (+ transport when available),
 *   2) `ws.disconnected` at INFO with socket_id + reason verbatim (R8),
 *   3) `ws.subscribe_denied` at WARN when an anonymous socket attempts the
 *      admin-gated runtime_ack room (coexists with the ROOM-AUTH-01 ack
 *      callback — this test emits WITHOUT a callback, legacy path),
 *   4) room subscriptions at DEBUG only (info channel = lifecycle only).
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { createServer, type Server as HttpServer } from "node:http";
import type { Server as IoServer } from "socket.io";
import { io as ioClient, type Socket as ClientSocket } from "socket.io-client";
import { setupWebSocketGateway, type WsLogger } from "./websocket.js";

type LogLine = { level: "info" | "warn" | "debug"; obj: Record<string, unknown> };

describe("WO-NO-WS-LOGS-01 — structured ws.* lifecycle logs", () => {
  let httpServer: HttpServer;
  let io: IoServer;
  let port: number;
  const lines: LogLine[] = [];
  const fakeLogger: WsLogger = {
    info: (obj, _msg) => lines.push({ level: "info", obj: obj as Record<string, unknown> }),
    warn: (obj, _msg) => lines.push({ level: "warn", obj: obj as Record<string, unknown> }),
    debug: (obj, _msg) => lines.push({ level: "debug", obj: obj as Record<string, unknown> }),
  };

  beforeAll(async () => {
    httpServer = createServer();
    io = setupWebSocketGateway(httpServer, undefined, fakeLogger);
    await new Promise<void>((resolve) => httpServer.listen(0, resolve));
    const addr = httpServer.address();
    port = typeof addr === "object" && addr ? addr.port : 0;
  });

  afterAll(async () => {
    await new Promise<void>((resolve) => io.close(() => resolve()));
    await new Promise<void>((resolve) => httpServer.close(() => resolve()));
  });

  it("logs ws.connected at info (grep-friendly event name, socket_id present)", async () => {
    const client: ClientSocket = ioClient(`http://127.0.0.1:${port}`, {
      transports: ["websocket"],
      reconnection: false,
    });
    await new Promise<void>((resolve) => client.on("connect", () => resolve()));

    const connected = lines.find((l) => l.obj["event"] === "ws.connected");
    expect(connected).toBeDefined();
    expect(connected!.level).toBe("info");
    expect(typeof connected!.obj["socket_id"]).toBe("string");
    expect(connected!.obj["socket_id"]).not.toBe("");

    // socket.io-client with reconnection:false does not always emit the
    // client-side "disconnect" event after an explicit disconnect() — waiting
    // on it hangs (CI 2026-09-19). Settle like the tests below instead.
    client.disconnect();
    await new Promise<void>((resolve) => setTimeout(resolve, 250));
  });

  it("logs ws.disconnected at info with the disconnect reason verbatim (R8)", async () => {
    const client: ClientSocket = ioClient(`http://127.0.0.1:${port}`, {
      transports: ["websocket"],
      reconnection: false,
    });
    await new Promise<void>((resolve) => client.on("connect", () => resolve()));
    client.disconnect();
    // Give the server a beat to process the disconnect frame.
    await new Promise<void>((resolve) => setTimeout(resolve, 250));

    const disconnected = lines.find((l) => l.obj["event"] === "ws.disconnected");
    expect(disconnected).toBeDefined();
    expect(disconnected!.level).toBe("info");
    expect(typeof disconnected!.obj["reason"]).toBe("string");
    expect((disconnected!.obj["reason"] as string).length).toBeGreaterThan(0);
  });

  it("logs ws.subscribe_denied at warn for an anonymous runtime_ack join (legacy no-callback path)", async () => {
    const client: ClientSocket = ioClient(`http://127.0.0.1:${port}`, {
      transports: ["websocket"],
      reconnection: false,
    });
    await new Promise<void>((resolve) => client.on("connect", () => resolve()));

    const err = await new Promise<{ code: string; room: string }>((resolve) => {
      client.on("error", (payload: { code: string; room: string }) => resolve(payload));
      client.emit("subscribe:runtime_ack"); // no ack callback — legacy path
    });
    expect(err.code).toBe("unauthorized");

    const denied = lines.find((l) => l.obj["event"] === "ws.subscribe_denied");
    expect(denied).toBeDefined();
    expect(denied!.level).toBe("warn");
    expect(denied!.obj["room"]).toBe("runtime_ack");
    expect(denied!.obj["code"]).toBe("unauthorized");

    client.disconnect();
    await new Promise<void>((resolve) => setTimeout(resolve, 250));
  });

  it("room subscriptions log at DEBUG only — the info channel carries lifecycle events only", async () => {
    const before = lines.filter((l) => l.level === "info").length;
    const client: ClientSocket = ioClient(`http://127.0.0.1:${port}`, {
      transports: ["websocket"],
      reconnection: false,
    });
    await new Promise<void>((resolve) => client.on("connect", () => resolve()));
    client.emit("subscribe:opportunities");
    client.emit("subscribe:route_discovery");
    await new Promise<void>((resolve) => setTimeout(resolve, 250));

    const subs = lines.filter((l) => l.obj["event"] === "ws.subscribed");
    expect(subs.length).toBeGreaterThanOrEqual(2);
    expect(subs.every((l) => l.level === "debug")).toBe(true);
    // Only the lifecycle pair (connected, later disconnected) may add info lines
    // — a subscription never floods info (R9).
    expect(lines.filter((l) => l.level === "info").length).toBe(before + 1);

    client.disconnect();
    await new Promise<void>((resolve) => setTimeout(resolve, 250));
  });
});
