/**
 * Circuit Breaker — 3-state machine with sliding window + cooldown + probe.
 *
 * Usage:
 *   const cb = new CircuitBreaker({ name: "goplus", threshold: 5, window_ms: 60_000, cooldown_ms: 120_000 });
 *   try { const result = await cb.execute(() => callApi(...)); }
 *   catch (e) { if (e instanceof CircuitBreakerOpenError) handleTripped(); else throw; }
 *
 * Emits events on state transitions — observed via registerListener(cb, handler).
 *
 * Concurrency: JS is single-threaded so no locks are needed, but async order
 * matters: we only record success/failure AFTER the awaited fn settles, so
 * concurrent in-flight probes do not race each other's state transitions
 * beyond the strictly-monotonic state machine below.
 */

import { EventEmitter } from "node:events";

export type CircuitState = "closed" | "open" | "half_open";

export interface CircuitBreakerOpts {
  name: string;
  threshold: number;         // errors in window before trip
  window_ms: number;         // sliding window for threshold counting
  cooldown_ms: number;       // time to wait in 'open' before trying 'half_open'
  half_open_probe_count?: number;  // successes needed in half_open to close. Default 3.
}

export class CircuitBreakerOpenError extends Error {
  public readonly name = "CircuitBreakerOpenError";
  constructor(public readonly breakerName: string, public readonly reason: string | null) {
    super(`circuit breaker '${breakerName}' is open${reason ? `: ${reason}` : ""}`);
  }
}

type FailureTs = number;

export class CircuitBreaker extends EventEmitter {
  private _state: CircuitState = "closed";
  private failures: FailureTs[] = [];
  private openedAt: number | null = null;
  private lastTripReason: string | null = null;
  private probeSuccessCount = 0;
  private readonly probeTarget: number;

  constructor(private readonly opts: CircuitBreakerOpts) {
    super();
    this.probeTarget = opts.half_open_probe_count ?? 3;
  }

  get name(): string { return this.opts.name; }
  state(): CircuitState {
    this.maybeHalfOpen();
    return this._state;
  }

  /** Execute fn under breaker protection. Throws CircuitBreakerOpenError if open. */
  async execute<T>(fn: () => Promise<T>): Promise<T> {
    const s = this.state();
    if (s === "open") {
      throw new CircuitBreakerOpenError(this.opts.name, this.lastTripReason);
    }
    try {
      const result = await fn();
      this.onSuccess();
      return result;
    } catch (err) {
      this.onFailure((err as Error)?.message ?? "unknown_error");
      throw err;
    }
  }

  /** Manually force open. Used from admin endpoint. */
  trip(reason: string): void {
    if (this._state !== "open") {
      this._state = "open";
      this.openedAt = Date.now();
      this.lastTripReason = reason;
      this.emit("trip", { name: this.opts.name, reason, ts: this.openedAt });
    }
  }

  /** Manually close. Used from admin endpoint. */
  reset(): void {
    const was = this._state;
    this._state = "closed";
    this.failures = [];
    this.openedAt = null;
    this.lastTripReason = null;
    this.probeSuccessCount = 0;
    if (was !== "closed") this.emit("reset", { name: this.opts.name, ts: Date.now() });
  }

  /** Snapshot for /admin/circuit_breakers. */
  snapshot() {
    return {
      name: this.opts.name,
      state: this.state(),
      failures_in_window: this.countRecentFailures(),
      last_trip_reason: this.lastTripReason,
      opened_at_ms: this.openedAt,
      cooldown_ms: this.opts.cooldown_ms,
      probe_progress: this._state === "half_open" ? `${this.probeSuccessCount}/${this.probeTarget}` : null,
    };
  }

  private onSuccess(): void {
    if (this._state === "half_open") {
      this.probeSuccessCount++;
      if (this.probeSuccessCount >= this.probeTarget) {
        const was = this._state;
        this._state = "closed";
        this.failures = [];
        this.openedAt = null;
        this.lastTripReason = null;
        this.probeSuccessCount = 0;
        if (was !== "closed") this.emit("reset", { name: this.opts.name, ts: Date.now() });
      }
    } else if (this._state === "closed") {
      // drop oldest failures outside window proactively (memory)
      this.pruneWindow();
    }
  }

  private onFailure(reason: string): void {
    if (this._state === "half_open") {
      // Probe failed → back to open, reset cooldown clock
      this._state = "open";
      this.openedAt = Date.now();
      this.lastTripReason = `half_open_probe_failed: ${reason}`;
      this.probeSuccessCount = 0;
      this.emit("trip", { name: this.opts.name, reason: this.lastTripReason, ts: this.openedAt });
      return;
    }
    if (this._state === "open") {
      // shouldn't happen: execute() already throws before fn runs.
      return;
    }
    this.failures.push(Date.now());
    this.pruneWindow();
    if (this.failures.length >= this.opts.threshold) {
      this._state = "open";
      this.openedAt = Date.now();
      this.lastTripReason = `threshold: ${this.failures.length} errors in ${this.opts.window_ms}ms — last: ${reason}`;
      this.emit("trip", { name: this.opts.name, reason: this.lastTripReason, ts: this.openedAt });
    }
  }

  private pruneWindow(): void {
    const cutoff = Date.now() - this.opts.window_ms;
    while (this.failures.length > 0 && this.failures[0]! < cutoff) {
      this.failures.shift();
    }
  }

  private countRecentFailures(): number {
    this.pruneWindow();
    return this.failures.length;
  }

  private maybeHalfOpen(): void {
    if (this._state === "open" && this.openedAt !== null) {
      if (Date.now() - this.openedAt >= this.opts.cooldown_ms) {
        this._state = "half_open";
        this.probeSuccessCount = 0;
        this.emit("half_open", { name: this.opts.name, ts: Date.now() });
      }
    }
  }
}

/** Registry for naming CBs and exposing snapshots via admin endpoint. */
export class CircuitBreakerRegistry {
  private readonly breakers = new Map<string, CircuitBreaker>();

  register(cb: CircuitBreaker): CircuitBreaker {
    if (this.breakers.has(cb.name)) {
      return this.breakers.get(cb.name)!;
    }
    this.breakers.set(cb.name, cb);
    return cb;
  }

  get(name: string): CircuitBreaker | undefined {
    return this.breakers.get(name);
  }

  snapshots() {
    return Array.from(this.breakers.values()).map(cb => cb.snapshot());
  }
}

export const globalRegistry = new CircuitBreakerRegistry();
