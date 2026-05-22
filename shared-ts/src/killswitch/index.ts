import { Redis as IORedis } from "ioredis";
import { KillSwitchStateSchema, type KillSwitchState } from "../contracts/index.js";
import { killswitchEnabled } from "../metrics/index.js";

export const KILLSWITCH_KEY = "arbx:killswitch";
export const KILLSWITCH_CHANNEL = "arbx:killswitch:changes";

export interface KillSwitchClientOpts {
  redisUrl: string;
  defaultWhenAbsent: boolean;
  cacheTtlMs?: number;
}

export class KillSwitchClient {
  private redis: IORedis;
  private sub?: IORedis;
  private cache?: { state: KillSwitchState; at: number };
  private readonly ttlMs: number;

  constructor(private readonly opts: KillSwitchClientOpts) {
    this.redis = new IORedis(opts.redisUrl, { lazyConnect: false, maxRetriesPerRequest: 3 });
    this.ttlMs = opts.cacheTtlMs ?? 1000;
  }

  async state(): Promise<KillSwitchState> {
    const now = Date.now();
    if (this.cache && now - this.cache.at < this.ttlMs) return this.cache.state;

    const raw = await this.redis.get(KILLSWITCH_KEY);
    let state: KillSwitchState;
    if (raw) {
      state = KillSwitchStateSchema.parse(JSON.parse(raw));
    } else {
      state = {
        enabled: this.opts.defaultWhenAbsent,
        reason: null,
        triggered_by: null,
        updated_at: new Date().toISOString(),
      };
    }
    this.cache = { state, at: now };
    killswitchEnabled.set(state.enabled ? 1 : 0);
    return state;
  }

  async isEnabled(): Promise<boolean> {
    try {
      return (await this.state()).enabled;
    } catch {
      return this.opts.defaultWhenAbsent;
    }
  }

  async set(opts: { enabled: boolean; reason?: string | null; triggered_by?: string | null }): Promise<KillSwitchState> {
    const state: KillSwitchState = {
      enabled: opts.enabled,
      reason: opts.reason ?? null,
      triggered_by: opts.triggered_by ?? null,
      updated_at: new Date().toISOString(),
    };
    const json = JSON.stringify(state);
    await this.redis.set(KILLSWITCH_KEY, json);
    await this.redis.publish(KILLSWITCH_CHANNEL, json);
    this.cache = { state, at: Date.now() };
    killswitchEnabled.set(state.enabled ? 1 : 0);
    return state;
  }

  /** Optional subscription to invalidate cache immediately on change. */
  async subscribeChanges(onChange?: (s: KillSwitchState) => void): Promise<void> {
    const sub = new IORedis(this.opts.redisUrl, {
      lazyConnect: false,
      // One-shot subscriber: if the connection drops for any reason, give up
      // immediately rather than retrying forever.  The KillSwitchClient falls
      // back to its 1-second TTL poll automatically.
      retryStrategy: () => null,
      // Shorter connect timeout so a stale connection fails fast.
      connectTimeout: 2_000,
    });
    // Prevent Node.js from treating connection errors as unhandled exceptions.
    sub.on("error", () => {});
    this.sub = sub;
    await sub.subscribe(KILLSWITCH_CHANNEL);
    sub.on("message", (_channel: string, message: string) => {
      try {
        const state = KillSwitchStateSchema.parse(JSON.parse(message));
        this.cache = { state, at: Date.now() };
        killswitchEnabled.set(state.enabled ? 1 : 0);
        if (onChange) onChange(state);
      } catch {
        // ignore malformed updates; next poll will re-read Redis
      }
    });
  }

  async close(): Promise<void> {
    await this.redis.quit();
    if (this.sub) await this.sub.quit();
  }
}
