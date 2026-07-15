import { describe, it, expect } from "vitest";
import { createServer } from "http";
import { Server } from "socket.io";
import { io as ioc } from "socket.io-client";
import { CarnotStore } from "./services/carnotStore.js";
import { registerCarnotWebSocket } from "./websocket-carnot.js";

describe("carnot websocket", () => {
  it("broadcasts a cycle to subscribers", async () => {
    const httpServer = createServer();
    const io = new Server(httpServer);
    const store = new CarnotStore();
    registerCarnotWebSocket(io, store, { info: () => {} });
    httpServer.listen(0);
    const port = (httpServer.address() as any).port;

    const client = ioc(`http://127.0.0.1:${port}`);
    await new Promise<void>((res) => client.on("connect", res));
    const subscribed = new Promise<void>((res) => client.on("carnot:subscribed", res));
    client.emit("subscribe:carnot");
    await subscribed;

    const cycle = {
      id: "ws-1",
      chain_id: 1,
      detected_at: new Date().toISOString(),
      eta: 0.1,
      work_extracted_usd: 1.0,
      heat_in_usd: 10.0,
      heat_out_usd: 9.0,
      gradient: {
        token_in: "WETH",
        token_out: "USDC",
        potential_delta_usd: 1.0,
        venue_in: "uni",
        venue_out: "binance",
      },
      dissipation: { gas_usd: 0.5, fee_bps: 30, latency_ms: 50, decoherence_usd: 0.1 },
      status: "detected",
    };

    const received = new Promise<void>((res) =>
      client.on("carnot:cycle", (msg: any) => {
        expect(msg.id).toBe("ws-1");
        res();
      })
    );

    store.onAdd = (c) => {
      io.to("carnot:cycles").emit("carnot:cycle", c);
    };
    store.add(cycle as any);

    await received;
    client.close();
    io.close();
    httpServer.close();
  });
});
