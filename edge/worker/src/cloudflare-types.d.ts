/**
 * Ambient declarations for the Cloudflare Workers types referenced by the
 * canonical index.ts, so it compiles under the Node build (tsconfig.build.json)
 * WITHOUT @cloudflare/workers-types in `types` and WITHOUT editing index.ts.
 *
 * Only the slice of KVNamespace/D1Database that index.ts touches is declared.
 * The real runtime objects are supplied by kv-redis.ts (RedisKV) and the
 * ARBX_TELEMETRY binding is left undefined in node-server.ts.
 */

interface KVNamespaceGetOptions {
  type?: "json" | "text" | "arrayBuffer" | "stream";
}

interface KVNamespace {
  get(key: string, options?: KVNamespaceGetOptions): Promise<string | null>;
  get<T>(key: string, options: { type: "json" }): Promise<T | null>;
  put(key: string, value: string, options?: { expirationTtl?: number }): Promise<void>;
  delete(key: string): Promise<void>;
}

interface D1Database {
  prepare(query: string): { bind(...values: unknown[]): { run(): Promise<unknown> } };
}

interface D1Result<T = unknown> {
  results?: T[];
  success: boolean;
  meta?: unknown;
}
