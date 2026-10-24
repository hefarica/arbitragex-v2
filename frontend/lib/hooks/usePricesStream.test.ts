import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  applyPriceEvent,
  acceptPriceFrame,
  silenceAction,
  watchdogAction,
  createPricesLifecycle,
  usePricesStream,
  SILENCE_RECONNECT_MS,
  MAX_WS_ERRORS,
  MAX_FORCED_RECONNECTS,
  WATCHDOG_TICK_MS,
  CONNECT_TIMEOUT_MS,
  PENDING_ATTEMPT_DEADLINE_MS,
  POLL_INTERVAL_MS,
  POLL_TIMEOUT_MS,
  EMPTY_PRICES_STATE,
  type PricesState,
  type PricesSocketLike,
  type PricesFetchResponse,
  type PricesLifecycleDeps,
  type UsePricesStreamResult,
} from "./usePricesStream";

const baseState: PricesState = { ...EMPTY_PRICES_STATE };

describe("applyPriceEvent (G-PRICE-1 pure transitions)", () => {
  it("applies a valid full-map frame and rotates prevPrices", () => {
    const s1 = applyPriceEvent(baseState, {
      chain_id: 1,
      prices: { WETH: 2500, USDC: 1 },
      count: 2,
      ttl_secs: 60,
      ts: "2026-08-19T17:00:00Z",
      seq: 1,
    });
    expect(s1.prices).toEqual({ WETH: 2500, USDC: 1 });
    expect(s1.prevPrices).toEqual({});
    expect(s1.seq).toBe(1);
    expect(s1.ttlSecs).toBe(60);

    const s2 = applyPriceEvent(s1, {
      chain_id: 1,
      prices: { WETH: 2600, USDC: 1 },
      count: 2,
      ttl_secs: 58,
      ts: "2026-08-19T17:00:30Z",
      seq: 2,
    });
    // Full-map replace + previous map preserved for direction tracking.
    expect(s2.prices.WETH).toBe(2600);
    expect(s2.prevPrices.WETH).toBe(2500);
    expect(s2.seq).toBe(2);
  });

  it("uppercases symbols and drops non-finite/non-positive prices (R8)", () => {
    const s = applyPriceEvent(baseState, {
      chain_id: 1,
      prices: { weth: 2500, bad_nan: NaN, bad_neg: -1, bad_zero: 0, bad_inf: Infinity },
      count: 5,
      ttl_secs: 60,
      ts: "t",
      seq: 1,
    });
    expect(Object.keys(s.prices)).toEqual(["WETH"]);
  });

  it("rejects non-object payloads and returns the previous state", () => {
    expect(applyPriceEvent(baseState, null)).toBe(baseState);
    expect(applyPriceEvent(baseState, "junk")).toBe(baseState);
    expect(applyPriceEvent(baseState, 42)).toBe(baseState);
  });

  it("rejects frames without a numeric chain_id / prices object", () => {
    expect(applyPriceEvent(baseState, { prices: { A: 1 } })).toBe(baseState);
    expect(applyPriceEvent(baseState, { chain_id: 1, prices: null })).toBe(baseState);
    expect(applyPriceEvent(baseState, { chain_id: "1", prices: {} })).toBe(baseState);
  });

  it("falls back to previous ts/seq when the frame omits them", () => {
    const s1 = applyPriceEvent(baseState, {
      chain_id: 1,
      prices: { A: 1 },
      count: 1,
      ttl_secs: 30,
      ts: "2026-08-19T17:01:00Z",
      seq: 7,
    });
    const s2 = applyPriceEvent(s1, { chain_id: 1, prices: { A: 2 } });
    expect(s2.ts).toBe("2026-08-19T17:01:00Z");
    expect(s2.seq).toBe(7);
    expect(s2.ttlSecs).toBeNull();
  });
});

describe("silenceAction (EDGE-HARD-1 watchdog)", () => {
  const NOW = 1_000_000;

  it("ok while frames flow within the liveness contract (<90s)", () => {
    expect(silenceAction(NOW - 35_000, 0, NOW)).toBe("ok");
    expect(silenceAction(NOW - 89_999, 2, NOW)).toBe("ok");
  });

  it("no frame yet → ok (connect flow owns the initial window)", () => {
    expect(silenceAction(null, 0, NOW)).toBe("ok");
  });

  it("silent past 90s with budget left → reconnect", () => {
    expect(silenceAction(NOW - SILENCE_RECONNECT_MS, 0, NOW)).toBe("reconnect");
    expect(silenceAction(NOW - 300_000, 2, NOW)).toBe("reconnect");
  });

  it("silent past 90s with budget spent → degrade to polling", () => {
    expect(silenceAction(NOW - SILENCE_RECONNECT_MS, 3, NOW)).toBe("degrade");
    expect(silenceAction(NOW - 600_000, 10, NOW)).toBe("degrade");
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// P0-a / P0-b / §2 — the repaired lifecycle. Everything below drives the REAL
// factory with an injected socket, clock, fetch and logger: no jsdom, no
// @testing-library, no new dependency (contract §4 — Ruta A).
// ─────────────────────────────────────────────────────────────────────────────

function frame(
  chainId: number,
  prices: Record<string, number> = { WETH: 2500 },
): Record<string, unknown> {
  return {
    chain_id: chainId,
    prices,
    count: Object.keys(prices).length,
    ttl_secs: 60,
    ts: "2026-08-19T17:00:00Z",
    seq: 1,
  };
}

/** socket.io-shaped double. `disconnect()` fires "disconnect" synchronously,
 * exactly like the real client, so a forced reconnect is never a silent no-op. */
class FakeSocket implements PricesSocketLike {
  private readonly handlers = new Map<string, Array<(...args: unknown[]) => void>>();
  connectCalls = 0;
  disconnectCalls = 0;
  closeCalls = 0;
  readonly emitted: Array<{ event: string; args: unknown[] }> = [];

  on(event: string, handler: (...args: unknown[]) => void): unknown {
    const list = this.handlers.get(event) ?? [];
    list.push(handler);
    this.handlers.set(event, list);
    return this;
  }
  emit(event: string, ...args: unknown[]): unknown {
    this.emitted.push({ event, args });
    return this;
  }
  connect(): void {
    this.connectCalls += 1;
  }
  disconnect(): void {
    this.disconnectCalls += 1;
    this.fire("disconnect", "io client disconnect");
  }
  close(): void {
    this.closeCalls += 1;
  }
  fire(event: string, ...args: unknown[]): void {
    for (const h of [...(this.handlers.get(event) ?? [])]) h(...args);
  }
}

interface PendingFetch {
  url: string;
  init: RequestInit;
  resolve: (r: PricesFetchResponse) => void;
}

function makeHarness(opts: { chainId?: number } = {}) {
  const sockets: FakeSocket[] = [];
  const frames: unknown[] = [];
  const statuses: string[] = [];
  const logs: Array<{ level: "warn" | "error"; message: string; detail?: unknown }> = [];
  const pendingFetches: PendingFetch[] = [];
  let current = true;
  let clock = 0;

  const deps: PricesLifecycleDeps = {
    url: "ws://api-server.test",
    edgeUrl: "http://edge.test",
    chainId: opts.chainId ?? 1,
    isCurrent: () => current,
    ioFactory: () => {
      const s = new FakeSocket();
      sockets.push(s);
      return s;
    },
    now: () => clock,
    // A REST snapshot that never arrives on its own: it resolves only when the
    // test says so, and rejects when the lifecycle aborts it.
    fetchImpl: (url, init) =>
      new Promise<PricesFetchResponse>((resolve, reject) => {
        pendingFetches.push({ url, init, resolve });
        const signal = init.signal;
        if (signal) signal.addEventListener("abort", () => reject(new Error("AbortError")));
      }),
    onStatus: (s) => {
      statuses.push(s);
    },
    onFrame: (e) => {
      frames.push(e);
    },
    log: (level, entry) => {
      logs.push({ level, message: entry.message, detail: entry.detail });
    },
  };

  const handle = createPricesLifecycle(deps);
  return {
    handle,
    sockets,
    frames,
    statuses,
    logs,
    pendingFetches,
    clock: () => clock,
    setClock: (ms: number) => {
      clock = ms;
    },
    /** Simulate the effect-run teardown that happens when the chain changes. */
    supersede: () => {
      current = false;
    },
  };
}

type Harness = ReturnType<typeof makeHarness>;

function wsOf(h: Harness, index = 0): FakeSocket {
  const s = h.sockets[index];
  if (!s) throw new Error("no socket at index " + index);
  return s;
}

function forcedReconnectLogs(h: Harness): string[] {
  return h.logs.filter((l) => l.message.includes("forcing reconnect")).map((l) => l.message);
}

/** Drive the lifecycle into degradation through the connect_error budget. */
function degradeViaConnectErrors(h: Harness): void {
  const ws = wsOf(h);
  for (let i = 0; i < MAX_WS_ERRORS; i += 1) ws.fire("connect_error", new Error("boom"));
}

async function flush(): Promise<void> {
  await vi.advanceTimersByTimeAsync(0);
  for (let i = 0; i < 8; i += 1) await Promise.resolve();
}

describe("repair · P0-a / P0-b / §2 (createPricesLifecycle)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  // ── P0-a · plazos y presupuestos separados ────────────────────────────────
  describe("P0-a · los plazos quedan como constantes exportadas y verificables", () => {
    it("declara el invariante como expresión: intento pendiente > tick del watchdog", () => {
      expect(WATCHDOG_TICK_MS).toBe(15_000);
      expect(PENDING_ATTEMPT_DEADLINE_MS > WATCHDOG_TICK_MS).toBe(true);
      // socket.io's per-attempt timeout, declared EXPLICITLY on both sockets
      // instead of inherited from the library default.
      expect(CONNECT_TIMEOUT_MS).toBe(20_000);
      expect(PENDING_ATTEMPT_DEADLINE_MS).toBe(25_000);
      // Margin over the library's own window: at the limit the watchdog must not
      // close an attempt socket.io still considers alive.
      expect(PENDING_ATTEMPT_DEADLINE_MS >= CONNECT_TIMEOUT_MS + 5_000).toBe(true);
      // Pin 5: el plazo del intento NO reutiliza el umbral de silencio.
      expect(PENDING_ATTEMPT_DEADLINE_MS).not.toBe(SILENCE_RECONNECT_MS);
      expect(SILENCE_RECONNECT_MS).toBe(90_000);
      expect(MAX_FORCED_RECONNECTS).toBe(3);
    });

    it("un intento pendiente no es 'reconnect' en ningún tick dentro de su plazo", () => {
      const t0 = 1_000_000;
      const pending = {
        phase: "connecting" as const,
        lastFrameAt: t0,
        attemptStartedAt: t0,
        forcedReconnects: 1,
      };
      // The next tick lands 15s into the attempt: inside the deadline.
      expect(watchdogAction(pending, t0 + WATCHDOG_TICK_MS)).toEqual({ kind: "idle" });
      // And so does a tick one millisecond before the deadline.
      expect(watchdogAction(pending, t0 + PENDING_ATTEMPT_DEADLINE_MS - 1)).toEqual({
        kind: "idle",
      });
      // Past the deadline it is declared hung — degrade, never disconnect.
      expect(watchdogAction(pending, t0 + PENDING_ATTEMPT_DEADLINE_MS)).toEqual({
        kind: "degrade",
        reason: "never-connected",
      });
    });

    it("el defecto es CONDICIONAL: la formulación de sólo-silencio sí reconecta ahí, y no si el intento completó", () => {
      const t0 = 1_000_000;
      const now = t0 + SILENCE_RECONNECT_MS + 1;
      // Same state, same instant: the phase-blind ladder reissues the reconnect
      // over an attempt that is still in flight (one budget unit already spent).
      expect(silenceAction(t0, 1, now)).toBe("reconnect");
      // Contrapositive — at that same instant, with the attempt COMPLETED and a
      // frame accepted, the silence clock is fresh and the ladder is idle too.
      // Hence "ocurre siempre" is FALSE and must not be asserted.
      expect(silenceAction(now, 1, now)).toBe("ok");
    });
  });

  describe("P0-a · reproducción en 7 pasos + contraprueba", () => {
    it("pasos 1-7: el intento pendiente se conserva y sólo se declara colgado al vencer su plazo", async () => {
      const h = makeHarness();
      const ws = wsOf(h);

      // 1) connect and deliver a valid snapshot.
      ws.fire("connect");
      expect(h.handle.phase()).toBe("connected");
      ws.fire("prices:snapshot", frame(1));
      expect(h.frames).toHaveLength(1);
      expect(h.statuses).toContain("LIVE");

      // 2) advance past the silence threshold.
      h.setClock(SILENCE_RECONNECT_MS + 1);
      await vi.advanceTimersByTimeAsync(WATCHDOG_TICK_MS);

      // 3) the watchdog started the reconnect.
      expect(h.handle.phase()).toBe("connecting");
      expect(ws.disconnectCalls).toBe(1);
      expect(ws.connectCalls).toBe(1);

      // 4) keep the attempt pending: no `connect`, no error degradation.
      // 5) advance to the next tick.
      h.setClock(SILENCE_RECONNECT_MS + 1 + WATCHDOG_TICK_MS);
      await vi.advanceTimersByTimeAsync(WATCHDOG_TICK_MS);

      // 6) the ORIGINAL ladder, at this exact instant, orders another reconnect.
      expect(silenceAction(0, 1, h.clock())).toBe("reconnect");
      // 7) the CORRECTED one does NOT: the same attempt is still in flight.
      expect(ws.disconnectCalls).toBe(1);
      expect(ws.connectCalls).toBe(1);
      expect(h.handle.phase()).toBe("connecting");

      // …right up to its explicit deadline, and only then is it declared hung.
      h.setClock(SILENCE_RECONNECT_MS + 1 + PENDING_ATTEMPT_DEADLINE_MS);
      await vi.advanceTimersByTimeAsync(WATCHDOG_TICK_MS);
      expect(h.handle.phase()).toBe("polling");
      expect(ws.disconnectCalls).toBe(1); // never disconnected over the attempt
      expect(ws.closeCalls).toBe(1); // degraded once, through the choke point
      expect(
        h.logs.some((l) => l.message.includes("degrading to HTTP polling (never-connected)")),
      ).toBe(true);
      h.handle.dispose();
    });

    it("contraprueba: una reconexión que COMPLETA antes del tick no se cancela ni consume otro intento", async () => {
      const h = makeHarness();
      const ws = wsOf(h);
      ws.fire("connect");
      ws.fire("prices:snapshot", frame(1));

      h.setClock(SILENCE_RECONNECT_MS + 1);
      await vi.advanceTimersByTimeAsync(WATCHDOG_TICK_MS);
      expect(ws.disconnectCalls).toBe(1);
      expect(h.handle.phase()).toBe("connecting");

      // The reconnect COMPLETES before the next tick (handshake only).
      h.setClock(SILENCE_RECONNECT_MS + 2_000);
      ws.fire("connect");
      expect(h.handle.phase()).toBe("connected");

      h.setClock(SILENCE_RECONNECT_MS + 3_000);
      await vi.advanceTimersByTimeAsync(WATCHDOG_TICK_MS);
      // Not cancelled, and the silence clock is fresh ⇒ idle.
      expect(ws.disconnectCalls).toBe(1);
      expect(ws.connectCalls).toBe(1);
      expect(h.handle.phase()).toBe("connected");

      // The handshake did NOT silently reset the budget: the next silence episode
      // spends exactly one more attempt (2/3), not a fresh 1/3.
      h.setClock(SILENCE_RECONNECT_MS + 2_000 + SILENCE_RECONNECT_MS + 1);
      await vi.advanceTimersByTimeAsync(WATCHDOG_TICK_MS);
      expect(ws.disconnectCalls).toBe(2);
      const forced = forcedReconnectLogs(h);
      expect(forced).toHaveLength(2);
      expect(forced[1]).toContain("(2/" + MAX_FORCED_RECONNECTS + ")");
      h.handle.dispose();
    });

    it("§2.2 · sólo un frame válido reinicia el presupuesto y acredita LIVE", async () => {
      const h = makeHarness();
      const ws = wsOf(h);
      ws.fire("connect");
      ws.fire("prices:snapshot", frame(1));
      expect(h.statuses).toContain("LIVE");

      h.setClock(SILENCE_RECONNECT_MS + 1);
      await vi.advanceTimersByTimeAsync(WATCHDOG_TICK_MS);
      expect(forcedReconnectLogs(h)).toHaveLength(1);
      const statusesBefore = h.statuses.length;

      // Invalid payloads while the attempt is pending: no liveness, no credit.
      // (Second one is a VALID frame for a DIFFERENT chain — the other half of
      // the same gate, exercised inside the pending window.)
      ws.fire("prices:snapshot", { chain_id: 1, prices: null });
      ws.fire("prices:update", { chain_id: 2, prices: { WETH: 1 } });
      expect(h.statuses.length).toBe(statusesBefore);
      expect(h.frames).toHaveLength(1);

      // The handshake then completes (still no frame) → budget NOT reset.
      h.setClock(SILENCE_RECONNECT_MS + 2_000);
      ws.fire("connect");
      h.setClock(SILENCE_RECONNECT_MS + 2_000 + SILENCE_RECONNECT_MS + 1);
      await vi.advanceTimersByTimeAsync(WATCHDOG_TICK_MS);
      const forced = forcedReconnectLogs(h);
      expect(forced).toHaveLength(2);
      expect(forced[1]).toContain("(2/" + MAX_FORCED_RECONNECTS + ")");
      h.handle.dispose();
    });
  });

  // ── P0-b · contaminación entre cadenas ────────────────────────────────────
  describe("P0-b · guarda de generación + validación de cadena", () => {
    it("un frame de OTRA cadena no se escribe en el estado activo", () => {
      const s1 = applyPriceEvent(EMPTY_PRICES_STATE, frame(1), 1);
      expect(s1.prices).toEqual({ WETH: 2500 });
      // A late frame for chain 2, arriving while chain 1 is active.
      expect(applyPriceEvent(s1, frame(2, { WETH: 9999 }), 1)).toBe(s1);
      expect(acceptPriceFrame(frame(2), 1)).toBe(false);
      expect(acceptPriceFrame(frame(1), 1)).toBe(true);
      // null keeps the pre-existing behaviour for callers with no chain filter.
      expect(acceptPriceFrame(frame(2), null)).toBe(true);
    });

    it("la respuesta en vuelo de la cadena A no llega al estado de la cadena B", async () => {
      const a = makeHarness({ chainId: 1 });
      degradeViaConnectErrors(a);
      expect(a.handle.phase()).toBe("polling");
      expect(a.pendingFetches).toHaveLength(1);
      expect(a.pendingFetches[0]?.url).toContain("chain_id=1");

      // The chain changes: A is torn down, B starts with its own generation.
      a.supersede();
      a.handle.dispose();
      const b = makeHarness({ chainId: 2 });

      // A's response lands NOW — after A was superseded.
      a.pendingFetches[0]?.resolve({
        ok: true,
        status: 200,
        json: async () => frame(1, { WETH: 1111 }),
      });
      await flush();

      expect(a.frames).toHaveLength(0); // dropped by the generation guard
      expect(b.frames).toHaveLength(0); // and it never reached chain B's state
      expect(b.statuses).not.toContain("LIVE");
      // The abort on teardown is NOT reported as a failure (local intent).
      expect(a.logs.some((l) => l.message.includes("poll failed"))).toBe(false);
      b.handle.dispose();
    });

    it("dentro de la MISMA generación, un frame de otra cadena también se rechaza", async () => {
      const h = makeHarness({ chainId: 1 });
      degradeViaConnectErrors(h);

      h.pendingFetches[0]?.resolve({
        ok: true,
        status: 200,
        json: async () => frame(2, { WETH: 2222 }),
      });
      await flush();
      expect(h.frames).toHaveLength(0);
      expect(h.logs.some((l) => l.message.includes("failed validation"))).toBe(true);

      // Same lifecycle, own chain ⇒ accepted end to end.
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
      expect(h.pendingFetches).toHaveLength(2);
      h.pendingFetches[1]?.resolve({
        ok: true,
        status: 200,
        json: async () => frame(1, { WETH: 3333 }),
      });
      await flush();
      expect(h.frames).toHaveLength(1);
      h.handle.dispose();
    });
  });

  // ── §2 · los cinco defectos confirmados ───────────────────────────────────
  describe("§2 · defectos confirmados", () => {
    it("§2.1/RV-3 · el timeout del fallback ya no es igual al intervalo", () => {
      expect(POLL_TIMEOUT_MS).toBe(8_000);
      expect(POLL_INTERVAL_MS).toBe(4_000);
      // NOT required to sit below the cadence: `pollInFlight` prevents overlap, and
      // forcing it below 4s aborted snapshots that could complete between 3s and 4s.
    });

    it("§2.1/RV-3 · un fallback que nunca responde se reporta, no se traga", async () => {
      const h = makeHarness();
      degradeViaConnectErrors(h);
      expect(h.handle.phase()).toBe("polling");

      await vi.advanceTimersByTimeAsync(POLL_TIMEOUT_MS);
      await flush();
      expect(
        h.logs.some((l) =>
          l.message.includes("poll timed out after " + POLL_TIMEOUT_MS + "ms — no update this tick"),
        ),
      ).toBe(true);
      expect(h.frames).toHaveLength(0);
      h.handle.dispose();
    });

    it("§2.2/RV-1 · un payload inválido no acredita LIVE", () => {
      const h = makeHarness();
      const ws = wsOf(h);
      ws.fire("connect");
      ws.fire("prices:snapshot", { chain_id: 1, prices: "not-an-object" });
      expect(h.frames).toHaveLength(0);
      expect(h.statuses).not.toContain("LIVE");
      expect(h.logs.some((l) => l.message.includes("rejected frame"))).toBe(true);

      // A VALID own-chain frame is the only thing that credits LIVE.
      ws.fire("prices:snapshot", frame(1));
      expect(h.statuses[h.statuses.length - 1]).toBe("LIVE");
      h.handle.dispose();
    });

    it("§2.2 · un mapa vacío pero bien formado SÍ es un frame válido (R8: feed ausente)", () => {
      // Boundary pin, so nobody "fixes" this by accident: an empty map means the
      // feed is currently empty — the caller renders it as absent (R8), and the
      // frame still proves the session is alive. What is rejected is a MALFORMED
      // payload or another chain's frame.
      expect(acceptPriceFrame({ chain_id: 1, prices: {} }, 1)).toBe(true);
      expect(acceptPriceFrame({ chain_id: 1, prices: null }, 1)).toBe(false);
      // Documented quirk, pinned instead of hidden: an array is `typeof "object"`
      // and its entries are indices, so it degrades to an EMPTY map — accepted as
      // an empty feed, never as a fabricated one. The server never sends it.
      expect(acceptPriceFrame({ chain_id: 1, prices: [] }, 1)).toBe(true);
      expect(acceptPriceFrame({ chain_id: 1, prices: "junk" }, 1)).toBe(false);
      expect(acceptPriceFrame({ prices: { WETH: 1 } }, 1)).toBe(false);
    });

    it("§2.3/RV-2 · el handler disconnect existe, actúa y respeta la intención local", () => {
      const h = makeHarness();
      const ws = wsOf(h);
      ws.fire("connect");
      ws.fire("prices:snapshot", frame(1));
      expect(h.statuses[h.statuses.length - 1]).toBe("LIVE");

      ws.fire("disconnect", "transport close");
      expect(h.handle.phase()).toBe("connecting");
      expect(h.statuses[h.statuses.length - 1]).toBe("STALE");
      expect(h.logs.some((l) => l.message.includes("socket disconnected"))).toBe(true);

      // Local intent: degrading already owns the status, so a disconnect arriving
      // while polling must not overwrite POLLING with STALE.
      degradeViaConnectErrors(h);
      expect(h.handle.phase()).toBe("polling");
      expect(h.statuses[h.statuses.length - 1]).toBe("POLLING");
      ws.fire("disconnect", "transport close");
      expect(h.statuses[h.statuses.length - 1]).toBe("POLLING");
      h.handle.dispose();
    });

    it("§2.4 · «consecutivos» es literal: un frame aceptado reinicia el contador", () => {
      const h = makeHarness();
      const ws = wsOf(h);
      ws.fire("connect");
      ws.fire("connect_error", new Error("1"));
      ws.fire("connect_error", new Error("2"));
      expect(h.handle.phase()).toBe("connecting");

      // A valid frame proves the path is alive again → counter back to 0.
      ws.fire("prices:snapshot", frame(1));
      expect(h.handle.phase()).toBe("connected");

      ws.fire("connect_error", new Error("3"));
      ws.fire("connect_error", new Error("4"));
      // Two consecutive errors < MAX_WS_ERRORS ⇒ still on the WebSocket.
      expect(h.handle.phase()).not.toBe("polling");
      ws.fire("connect_error", new Error("5"));
      expect(h.handle.phase()).toBe("polling");

      const counters = h.logs
        .filter((l) => l.message.includes("connect_error"))
        .map((l) => l.message);
      expect(counters).toHaveLength(5);
      expect(counters[0]).toContain("(1/3 consecutive)");
      expect(counters[1]).toContain("(2/3 consecutive)");
      expect(counters[2]).toContain("(1/3 consecutive)"); // reset by the accepted frame
      expect(counters[3]).toContain("(2/3 consecutive)");
      expect(counters[4]).toContain("(3/3 consecutive)");
      h.handle.dispose();
    });

    it("§2.5 · un HTTP no-ok del fallback se reporta en lugar de descartarse", async () => {
      const h = makeHarness();
      degradeViaConnectErrors(h);
      h.pendingFetches[0]?.resolve({ ok: false, status: 503, json: async () => ({}) });
      await flush();
      expect(h.logs.some((l) => l.message.includes("poll rejected — HTTP 503"))).toBe(true);
      h.handle.dispose();
    });

    it("§2.5 · el payload de prices:error se reporta, no se descarta", () => {
      const h = makeHarness();
      const ws = wsOf(h);
      ws.fire("connect");
      ws.fire("prices:snapshot", frame(1));
      ws.fire("prices:error", { reason: "redis timeout" });

      expect(h.statuses[h.statuses.length - 1]).toBe("STALE");
      const entry = h.logs.find((l) => l.message.includes("prices:error"));
      expect(entry).toBeDefined();
      expect(entry?.detail).toEqual({ chainId: 1, payload: { reason: "redis timeout" } });
      h.handle.dispose();
    });

    it("§2.3 · dispose cierra el socket una sola vez y no deja temporizadores vivos", async () => {
      const h = makeHarness();
      const ws = wsOf(h);
      ws.fire("connect");
      ws.fire("prices:snapshot", frame(1));
      h.handle.dispose();

      expect(ws.closeCalls).toBe(1);
      const fetchesAtDispose = h.pendingFetches.length;
      // Well past the watchdog tick, the silence threshold and the probe floor:
      // nothing may fire after dispose.
      await vi.advanceTimersByTimeAsync(2 * SILENCE_RECONNECT_MS);
      expect(ws.closeCalls).toBe(1);
      expect(h.pendingFetches).toHaveLength(fetchesAtDispose);
      expect(ws.disconnectCalls).toBe(0);
    });

    it("RV-4 · el retorno del hook sigue siendo { state, status } (PriceTicker.tsx:83)", () => {
      // Compile-time proof: PriceTicker.tsx:83 destructures exactly these two
      // keys and `tsc --noEmit` validates it. Here the type itself is pinned, so
      // dropping either field from UsePricesStreamResult fails to compile.
      const result: UsePricesStreamResult = { state: EMPTY_PRICES_STATE, status: "LIVE" };
      const { state, status } = result;
      expect(state.prices).toEqual({});
      expect(status).toBe("LIVE");
      expect(typeof usePricesStream).toBe("function");
    });

    it("§3.1 · los tres plazos y sus invariantes quedan como expresiones", () => {
      expect(CONNECT_TIMEOUT_MS).toBe(20_000);
      expect(PENDING_ATTEMPT_DEADLINE_MS).toBe(25_000);
      expect(POLL_TIMEOUT_MS).toBe(8_000);
      // At the limit the watchdog must never close an attempt socket.io still
      // considers alive — both invariants hold as EXPRESSIONS, not by hand:
      expect(PENDING_ATTEMPT_DEADLINE_MS > WATCHDOG_TICK_MS).toBe(true);
      expect(PENDING_ATTEMPT_DEADLINE_MS >= CONNECT_TIMEOUT_MS + 5_000).toBe(true);
      // The poll deadline is no longer forced below the interval: overlap is
      // prevented by `pollInFlight`, and forcing it below 4s aborted snapshots
      // that could have completed between 3s and 4s.
      expect(POLL_INTERVAL_MS).toBe(4_000);
      expect(POLL_TIMEOUT_MS).toBeGreaterThan(3_000);
    });

    it("F-RV1 · el socket de un probe PROMOVIDO arranca su propio connect() al caer", async () => {
      const h = makeHarness();
      const ws = wsOf(h);
      // Control negativo: el socket principal SÍ se reconecta solo, así que su
      // caída no debe gastar un connect() manual.
      ws.fire("connect");
      ws.fire("prices:snapshot", frame(1));
      const mainConnectsBefore = ws.connectCalls;
      ws.fire("disconnect", "transport close");
      expect(ws.connectCalls).toBe(mainConnectsBefore);

      // Degradar y dejar que el loop de probe cree uno (POLL_PROBE_MS = 60_000).
      degradeViaConnectErrors(h);
      expect(h.handle.phase()).toBe("polling");
      await vi.advanceTimersByTimeAsync(60_000);
      await flush();
      const probe = wsOf(h, 1);
      expect(probe.connectCalls).toBe(1); // el arranque del probe

      // Promoción: un frame aceptado del probe lo convierte en socket activo.
      probe.fire("connect");
      probe.fire("prices:snapshot", frame(1));
      expect(h.logs.some((l) => l.message.includes("leaving polling"))).toBe(true);

      // El probe promovido se creó con `reconnection: false`: nadie más lo va a
      // reabrir, y la fase ya declara un intento pendiente. Ese intento debe ser
      // REAL — antes de F-RV1 el handler ni siquiera corría (guardaba por
      // `source === "probe"`) y el reloj supervisaba un handshake inexistente.
      probe.fire("disconnect", "transport close");
      expect(probe.connectCalls).toBe(2);
      expect(h.handle.phase()).toBe("connecting");
      h.handle.dispose();
    });
  });
});
