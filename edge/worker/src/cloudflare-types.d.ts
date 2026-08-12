/**
 * Ambient declarations for the Cloudflare Workers runtime surface that the
 * canonical index.ts references, so it compiles under the Node build
 * (tsconfig.build.json with `types: ["node"]`) WITHOUT @cloudflare/workers-types
 * and WITHOUT editing index.ts.
 *
 * Runtime note: the real objects are supplied differently under Node —
 * KVNamespace is implemented by kv-redis.ts (RedisKV), D1Database/ARBX_TELEMETRY
 * is left undefined (guarded by `if (c.env.ARBX_TELEMETRY)`), and the
 * WebSocketPair route only activates on `upgrade: websocket` (never sent by the
 * POC curls). The `cf` fetch option and `webSocket` ResponseInit option are
 * silently ignored by Node's fetch / Response at runtime.
 */

// --- KVNamespace ----------------------------------------------------------
// CF KV accepts both get(key, "json") and get(key, { type: "json" }).
// index.ts calls the positional string form (e.g. L88, L94).
interface KVNamespaceGetOptions {
  type?: "json" | "text" | "arrayBuffer" | "stream";
}

interface KVNamespace {
  get(key: string, options?: KVNamespaceGetOptions): Promise<string | null>;
  get(key: string, type: "json"): Promise<unknown | null>;
  get<T>(key: string, options: { type: "json" }): Promise<T | null>;
  put(key: string, value: string, options?: { expirationTtl?: number }): Promise<void>;
  delete(key: string): Promise<void>;
}

// --- D1Database -----------------------------------------------------------
interface D1PreparedStatement {
  bind(...values: unknown[]): D1PreparedStatement;
  run(): Promise<unknown>;
}
interface D1Database {
  prepare(query: string): D1PreparedStatement;
}

// --- Cloudflare WebSocket pair (Carnot /carnot-cycles route only) ---------
interface WebSocket {
  accept(): void;
  addEventListener(type: "message", listener: (ev: { data: unknown }) => void): void;
  addEventListener(type: "open" | "close" | "error", listener: (ev: unknown) => void): void;
  send(data: unknown): void;
  close(code?: number, reason?: string): void;
  readonly readyState: number;
}
interface WebSocketPair {
  0: WebSocket;
  1: WebSocket;
}
declare const WebSocketPair: {
  new (): WebSocketPair;
};
interface ErrorEvent extends Event {
  message: string;
}
interface MessageEvent<T = unknown> extends Event {
  data: T;
}

// --- fetch / Response CF extensions ---------------------------------------
// `cf` on RequestInit and `webSocket` on ResponseInit are CF-only; Node's
// implementations ignore unknown keys, so we widen the lib types structurally.
// Node 20's fetch types come from undici-types (a module), so we augment that
// module's RequestInit/ResponseInit rather than declaring loose globals.
declare module "undici-types" {
  interface RequestInit {
    cf?: unknown;
  }
  interface ResponseInit {
    webSocket?: unknown;
  }
}
