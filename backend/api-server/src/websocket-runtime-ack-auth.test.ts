/**
 * RUNTIME-ACK-AUTH (2026-10-24) — regressions del gate del room `runtime_ack`.
 *
 * DEFECTO REPRODUCIDO (cadena medida):
 *   1. El browser no puede leer la cookie httpOnly `arbx_admin_session`, así que
 *      tras V-AT-1 conecta con `auth.token` VACÍO.
 *   2. nginx proxya `/socket.io/` directo a api-server:8080 SIN inyectar
 *      `x-arbx-admin-token` (RULE 02), y el edge que sí traduce cookie→header no
 *      actúa sobre su wsProxy ⇒ el token no llegaba NUNCA.
 *   3. `extractHandshakeToken` sólo leía auth|query|header ⇒
 *      `runtimeAckAllowed=false` ⇒ join rechazado ⇒ chip `runtime_ack ERROR`.
 *   4. Además el ack del join estaba neutralizado (`const reply = undefined`,
 *      TEMP-ROOM-AUTH-01): ni siquiera un cliente AUTORIZADO recibía veredicto.
 *
 * Estos tests ejercen el GATEWAY REAL (`setupWebSocketGateway`), no una réplica,
 * con un cliente socket.io real sobre transporte websocket.
 */

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { createServer, type Server as HttpServer } from "node:http";
import type { Server as IoServer } from "socket.io";
import { io as ioClient, type Socket as ClientSocket } from "socket.io-client";

import {
  broadcastRuntimeAck,
  extractHandshakeToken,
  RUNTIME_ACK_ROOM,
  setupWebSocketGateway,
  type RuntimeAckBroadcast,
} from "./websocket.js";

// >=32 bytes: the boot validator requires it, and tests must use a realistic shape.
const TOKEN = "test-admin-token-0123456789abcdef0123456789abcdef";
const SESSION_COOKIE = "arbx_admin_session";

const PAYLOAD: RuntimeAckBroadcast = {
  event_id: "44444444-4444-4444-8444-444444444444",
  resource: "trading_config",
  chain_id: 1,
  idempotency_key: "idem-runtime-ack-auth",
  config_hash_before: "a".repeat(64),
  config_hash_after: "b".repeat(64),
  worker_id: "test-worker",
  layer: "searcher_rs",
  status: "applied",
  latency_ms: 7,
  error: null,
};

interface JoinVerdict {
  ok: boolean;
  code?: string;
}

describe("extractHandshakeToken — fuentes y prioridad (RUNTIME-ACK-AUTH)", () => {
  const headers = (h: Record<string, string>) => h;

  it("auth.token gana y se etiqueta como 'auth'", () => {
    expect(
      extractHandshakeToken({
        auth: { token: "from-auth" },
        query: { token: "from-query" },
        headers: headers({ "x-arbx-admin-token": "from-header", cookie: `${SESSION_COOKIE}=from-cookie` }),
      })
    ).toEqual({ token: "from-auth", source: "auth" });
  });

  it("query.token se usa cuando no hay auth", () => {
    expect(
      extractHandshakeToken({
        auth: {},
        query: { token: "from-query" },
        headers: headers({ "x-arbx-admin-token": "from-header" }),
      })
    ).toEqual({ token: "from-query", source: "query" });
  });

  it("header x-arbx-admin-token (ruta del edge/tooling) sigue funcionando", () => {
    expect(
      extractHandshakeToken({ auth: {}, query: {}, headers: headers({ "x-arbx-admin-token": "from-header" }) })
    ).toEqual({ token: "from-header", source: "header" });
  });

  it("cookie arbx_admin_session es la ÚLTIMA fuente (sólo browser)", () => {
    expect(
      extractHandshakeToken({
        auth: {},
        query: {},
        headers: headers({ cookie: `other=1; ${SESSION_COOKIE}=from-cookie; arbx_admin_session_ttl=99` }),
      })
    ).toEqual({ token: "from-cookie", source: "cookie" });
  });

  it("sin credencial → token vacío y source 'none'", () => {
    expect(extractHandshakeToken({ auth: {}, query: {}, headers: headers({}) })).toEqual({
      token: "",
      source: "none",
    });
    // Cookie de OTRO nombre no cuenta.
    expect(
      extractHandshakeToken({ auth: {}, query: {}, headers: headers({ cookie: "session=abc" }) })
    ).toEqual({ token: "", source: "none" });
  });
});

describe("gateway real — join de runtime_ack con y sin credencial", () => {
  let httpServer: HttpServer;
  let io: IoServer;
  let url: string;
  const clients: ClientSocket[] = [];

  beforeAll(async () => {
    process.env["ARBX_ADMIN_TOKEN"] = TOKEN;
    // Allowlist fail-closed: sólo nuestro origen (igual que prod).
    process.env["ALLOWED_ORIGINS"] = "https://arbx.test";
    httpServer = createServer();
    io = setupWebSocketGateway(httpServer, undefined, {
      info: () => {},
      warn: () => {},
      debug: () => {},
    });
    await new Promise<void>((resolve) => httpServer.listen(0, resolve));
    const addr = httpServer.address();
    const port = typeof addr === "object" && addr ? addr.port : 0;
    url = `http://127.0.0.1:${port}`;
  });

  afterAll(async () => {
    for (const c of clients) c.disconnect();
    await new Promise<void>((resolve) => io.close(() => resolve()));
    await new Promise<void>((resolve) => httpServer.close(() => resolve()));
    delete process.env["ARBX_ADMIN_TOKEN"];
  });

  /** Conecta un cliente websocket puro (sin polling) con headers opcionales. */
  function connect(opts: { auth?: Record<string, unknown>; headers?: Record<string, string> } = {}): Promise<ClientSocket> {
    return new Promise((resolve, reject) => {
      const socket = ioClient(url, {
        transports: ["websocket"],
        forceNew: true,
        reconnection: false,
        timeout: 4000,
        ...(opts.auth ? { auth: opts.auth } : {}),
        ...(opts.headers ? { extraHeaders: opts.headers } : {}),
      });
      clients.push(socket);
      socket.once("connect", () => resolve(socket));
      socket.once("connect_error", reject);
    });
  }

  /** Emite el join y captura el veredicto del ack (con timeout propio). */
  function join(socket: ClientSocket): Promise<JoinVerdict> {
    return new Promise<JoinVerdict>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("no ack for subscribe:runtime_ack")), 3000);
      socket.emit("subscribe:runtime_ack", (verdict: JoinVerdict) => {
        clearTimeout(timer);
        resolve(verdict);
      });
    });
  }

  it("(i)+(iv) handshake SIN credencial: conecta (transporte sano) pero el join se rechaza con ack false + error estructurado", async () => {
    const socket = await connect();
    expect(socket.connected).toBe(true); // el WS público sigue 101/abierto

    const errorEvent = new Promise<{ code?: string; room?: string }>((resolve) => {
      socket.once("error", (e: { code?: string; room?: string }) => resolve(e));
    });

    const verdict = await join(socket);
    expect(verdict).toEqual({ ok: false, code: "unauthorized" });

    // (iv) regresión: el error estructurado sigue emitiéndose igual.
    await expect(errorEvent).resolves.toEqual({ code: "unauthorized", room: RUNTIME_ACK_ROOM });
  });

  it("(ii) cookie de sesión VÁLIDA: runtimeAckAllowed=true ⇒ ack ok:true y el room queda unido", async () => {
    const socket = await connect({ headers: { cookie: `${SESSION_COOKIE}=${TOKEN}` } });

    await expect(join(socket)).resolves.toEqual({ ok: true });

    // Prueba de pertenencia al room: un broadcast real debe llegarle.
    const received = new Promise<RuntimeAckBroadcast>((resolve) => {
      socket.once("runtime_ack", (payload: RuntimeAckBroadcast) => resolve(payload));
    });
    broadcastRuntimeAck(io, PAYLOAD);
    await expect(received).resolves.toMatchObject({ event_id: PAYLOAD.event_id });
  });

  it("(iii) cookie INVÁLIDA: rechazo con ack false + error", async () => {
    const socket = await connect({ headers: { cookie: `${SESSION_COOKIE}=not-the-token` } });

    const errorEvent = new Promise<{ code?: string }>((resolve) => {
      socket.once("error", (e: { code?: string }) => resolve(e));
    });

    await expect(join(socket)).resolves.toEqual({ ok: false, code: "unauthorized" });
    await expect(errorEvent).resolves.toMatchObject({ code: "unauthorized" });
  });

  it("SEGURIDAD: cookie válida con Origin de OTRO sitio NO autoriza (anti cross-site WebSocket hijacking)", async () => {
    const socket = await connect({
      headers: { cookie: `${SESSION_COOKIE}=${TOKEN}`, origin: "https://evil.example" },
    });

    await expect(join(socket)).resolves.toEqual({ ok: false, code: "unauthorized" });
  });

  it("SEGURIDAD: cookie válida con Origin ALLOWLISTED sí autoriza (el browser propio funciona)", async () => {
    const socket = await connect({
      headers: { cookie: `${SESSION_COOKIE}=${TOKEN}`, origin: "https://arbx.test" },
    });

    await expect(join(socket)).resolves.toEqual({ ok: true });
  });

  it("no-regresión: header x-arbx-admin-token y auth.token siguen autorizando", async () => {
    const byHeader = await connect({ headers: { "x-arbx-admin-token": TOKEN } });
    await expect(join(byHeader)).resolves.toEqual({ ok: true });

    const byAuth = await connect({ auth: { token: TOKEN } });
    await expect(join(byAuth)).resolves.toEqual({ ok: true });
  });

  it("el ack es opcional: emitir el join SIN callback no rompe el socket", async () => {
    const socket = await connect({ headers: { cookie: `${SESSION_COOKIE}=${TOKEN}` } });
    socket.emit("subscribe:runtime_ack");
    // Si el handler llamara a un ack inexistente, el proceso/lanzaría; verificamos
    // que el socket sigue vivo y que un join CON callback ackea ok:true.
    await expect(join(socket)).resolves.toEqual({ ok: true });
    expect(socket.connected).toBe(true);
  });
});
