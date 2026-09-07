// frontend/lib/websocket-client.test.ts
// WO-01 (2026-09-06) — suite nueva: no existía test del cliente WS.
//
// Cobertura del contrato de eventos del room `opportunities`:
//   server emite → "new_opportunity"          (PG LISTEN, row_to_json(NEW))
//   server emite → "opportunity:detected"     (hot streamer, Redis stream)
//   server emite → "opportunity:validated"    (hot streamer, Redis stream)
//   cliente emite → "subscribe:opportunities" (on connect)
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("socket.io-client", () => ({ io: vi.fn() }));

import { io } from "socket.io-client";
import {
  HotOpportunityWebSocket,
  adaptNewOpportunityToHotEvent,
} from "./websocket-client";

type Handler = (...args: unknown[]) => void;

// Anotación explícita (mismo patrón que socket-lifecycle.test.ts) — rompe el
// ciclo de auto-referencia de `on` que dispara TS7022.
type MockSocket = {
  connected: boolean;
  on: (event: string, handler: Handler) => unknown;
  emit: (event: string, ...args: unknown[]) => unknown;
  disconnect: () => void;
  trigger: (event: string, ...args: unknown[]) => void;
};

function makeFakeSocket(): MockSocket {
  const handlers = new Map<string, Handler>();
  const socket: MockSocket = {
    connected: false,
    on: vi.fn((event: string, handler: Handler) => {
      handlers.set(event, handler);
      return socket;
    }),
    emit: vi.fn(),
    disconnect: vi.fn(),
    trigger: (event: string, ...args: unknown[]) =>
      handlers.get(event)?.(...args),
  };
  return socket;
}

// Fiel al escritor canónico (backend/searcher-rs/src/persistence.rs,
// INSERT INTO opportunities): columnas y tipos tal como los serializa
// row_to_json(NEW) — id uuid string, chain_id number, detected_at ISO string.
const PG_ROW = {
  id: "3f9c2b1e-8a7d-4c5b-9e6f-1a2b3c4d5e6f",
  chain_id: 1,
  strategy_kind: "dex_arb",
  dex_a: "uniswap-v2",
  dex_b: "sushiswap",
  pair_symbol: "WETH/USDC",
  token_in: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  token_out: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  amount_in_wei: "1000000000000000000",
  expected_profit_usd: 12.34,
  net_expected_profit_usd: 8.21,
  roi_pct: 0.42,
  risk_score: 0.11,
  block_number: 21000000,
  status: "detected",
  rejection_reason: null,
  trace_id: "7c1d9e0a-52f4-4b8a-a3d6-9f0e1d2c3b4a",
  detected_at: "2026-09-06T12:34:56.789+00:00",
  route_metadata: {},
  cartridge_id: null,
};

// ─── adaptNewOpportunityToHotEvent (fn pura) ──────────────────────────────────

describe("adaptNewOpportunityToHotEvent — contrato PG row_to_json(NEW)", () => {
  it("mapea id, chain_id (number→string), strategy_kind y detected_at (ISO→ms string)", () => {
    const adapted = adaptNewOpportunityToHotEvent(PG_ROW);

    expect(adapted).toEqual({
      id: PG_ROW.id,
      chain_id: "1",
      strategy_kind: "dex_arb",
      detected_at_ms: String(Date.parse("2026-09-06T12:34:56.789+00:00")),
    });
    // detected_at_ms es SIEMPRE un string numérico — nunca "NaN".
    expect(/^\d+$/.test(adapted!.detected_at_ms!)).toBe(true);
  });

  it("NO fabrica status/net_profit_wei/gas_used/timestamp_ms (R8 fail-honest)", () => {
    const adapted = adaptNewOpportunityToHotEvent(PG_ROW)!;

    // El row PG trae status='detected' y USD, no wei — nada de eso se inventa.
    expect(adapted.status).toBeUndefined();
    expect(adapted.net_profit_wei).toBeUndefined();
    expect(adapted.gas_used).toBeUndefined();
    expect(adapted.timestamp_ms).toBeUndefined();
  });

  it("detected_at no parseable → omite detected_at_ms pero conserva el resto", () => {
    const adapted = adaptNewOpportunityToHotEvent({
      ...PG_ROW,
      detected_at: "not-a-date",
    })!;

    expect(adapted.detected_at_ms).toBeUndefined();
    expect(adapted.id).toBe(PG_ROW.id);
    expect(adapted.chain_id).toBe("1");
  });

  it("chain_id como string pasa directo (tolerancia de shape)", () => {
    const adapted = adaptNewOpportunityToHotEvent({
      ...PG_ROW,
      chain_id: "8453",
    })!;

    expect(adapted.chain_id).toBe("8453");
  });

  it("payload corrupto → null (R8: descartar, jamás despachar basura)", () => {
    expect(adaptNewOpportunityToHotEvent(null)).toBeNull();
    expect(adaptNewOpportunityToHotEvent(undefined)).toBeNull();
    expect(adaptNewOpportunityToHotEvent("string")).toBeNull();
    expect(adaptNewOpportunityToHotEvent(42)).toBeNull();
    expect(adaptNewOpportunityToHotEvent([1, 2])).toBeNull();
    expect(adaptNewOpportunityToHotEvent({})).toBeNull();
    // El contrato del server SIEMPRE trae id uuid string — otro tipo se descarta.
    expect(adaptNewOpportunityToHotEvent({ ...PG_ROW, id: 42 })).toBeNull();
    expect(adaptNewOpportunityToHotEvent({ ...PG_ROW, id: "" })).toBeNull();
  });
});

// ─── HotOpportunityWebSocket — wiring de listeners ───────────────────────────

describe("HotOpportunityWebSocket — listeners del room opportunities", () => {
  let fake: MockSocket;

  beforeEach(() => {
    fake = makeFakeSocket();
    vi.mocked(io).mockClear();
    vi.mocked(io).mockImplementation(() => fake as never);
  });

  function makeClient() {
    return new HotOpportunityWebSocket({
      url: "http://localhost:8080",
      token: "test-token",
    });
  }

  it("connect() registra el listener new_opportunity (aditivo)", () => {
    makeClient().connect();

    expect(fake.on).toHaveBeenCalledWith(
      "new_opportunity",
      expect.any(Function),
    );
  });

  it("'connect' del socket emite subscribe:opportunities (room compartido por ambos eventos)", () => {
    makeClient().connect();

    fake.trigger("connect");

    expect(fake.emit).toHaveBeenCalledWith("subscribe:opportunities");
  });

  it("new_opportunity (row PG) despacha a onDetected con payload adaptado", () => {
    const client = makeClient();
    const onDetected = vi.fn();
    client.onDetected(onDetected);
    client.connect();

    fake.trigger("new_opportunity", PG_ROW);

    expect(onDetected).toHaveBeenCalledTimes(1);
    expect(onDetected).toHaveBeenCalledWith({
      id: PG_ROW.id,
      chain_id: "1",
      strategy_kind: "dex_arb",
      detected_at_ms: String(Date.parse("2026-09-06T12:34:56.789+00:00")),
    });
  });

  it("new_opportunity corrupto NO despacha ni lanza (fail-honest)", () => {
    const client = makeClient();
    const onDetected = vi.fn();
    client.onDetected(onDetected);
    client.connect();

    expect(() => fake.trigger("new_opportunity", "basura")).not.toThrow();
    expect(() => fake.trigger("new_opportunity", null)).not.toThrow();

    expect(onDetected).not.toHaveBeenCalled();
  });

  it("opportunity:detected sigue despachando SIN adaptación (referencia intacta)", () => {
    const client = makeClient();
    const onDetected = vi.fn();
    client.onDetected(onDetected);
    client.connect();

    const hotPayload = {
      id: "stream-1",
      chain_id: "1",
      strategy_kind: "dex_arb",
      status: "passed" as const,
      net_profit_wei: "1000",
    };
    fake.trigger("opportunity:detected", hotPayload);

    expect(onDetected).toHaveBeenCalledTimes(1);
    // Mismo objeto por referencia — el listener existente no fue modificado.
    expect(onDetected).toHaveBeenCalledWith(hotPayload);
    expect(onDetected.mock.calls[0]![0]).toBe(hotPayload);
  });

  it("opportunity:validated sigue despachando a onValidated (intacto)", () => {
    const client = makeClient();
    const onValidated = vi.fn();
    client.onValidated(onValidated);
    client.connect();

    const hotPayload = { id: "stream-1", status: "failed" as const };
    fake.trigger("opportunity:validated", hotPayload);

    expect(onValidated).toHaveBeenCalledTimes(1);
    expect(onValidated).toHaveBeenCalledWith(hotPayload);
  });

  it("unsubscribe de onDetected remueve el callback para ambos eventos", () => {
    const client = makeClient();
    const onDetected = vi.fn();
    const off = client.onDetected(onDetected);
    client.connect();

    fake.trigger("opportunity:detected", { id: "a" });
    fake.trigger("new_opportunity", PG_ROW);
    expect(onDetected).toHaveBeenCalledTimes(2);

    off();
    fake.trigger("opportunity:detected", { id: "b" });
    fake.trigger("new_opportunity", PG_ROW);
    expect(onDetected).toHaveBeenCalledTimes(2);
  });
});
