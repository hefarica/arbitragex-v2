import type { Redis } from "ioredis";

/**
 * Redis-backed implementation of the KVNamespace slice that edge/worker uses.
 * Mirrors Cloudflare KV semantics for the four operations the worker calls:
 * get(string), get(json), put(with optional expirationTtl), delete.
 *
 * All keys are prefixed so the worker's keyspace never collides with the
 * keys used by api-server / dev-local in the shared Redis instance.
 */
export class RedisKV {
  constructor(
    private readonly redis: Redis,
    private readonly prefix: string,
  ) {}

  private k(key: string): string {
    return `${this.prefix}${key}`;
  }

  async get(key: string): Promise<string | null>;
  async get<T>(key: string, type: "json"): Promise<T | null>;
  async get<T>(key: string, type?: "json"): Promise<string | T | null> {
    const raw = await this.redis.get(this.k(key));
    if (raw === null) return null;
    if (type === "json") {
      try {
        return JSON.parse(raw) as T;
      } catch {
        return null;
      }
    }
    return raw;
  }

  async put(
    key: string,
    value: string,
    opts?: { expirationTtl?: number },
  ): Promise<void> {
    const k = this.k(key);
    if (opts?.expirationTtl && opts.expirationTtl > 0) {
      await this.redis.set(k, value, "EX", opts.expirationTtl);
    } else {
      await this.redis.set(k, value);
    }
  }

  async delete(key: string): Promise<void> {
    await this.redis.del(this.k(key));
  }
}
