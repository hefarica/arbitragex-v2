import type { Server as SocketServer } from "socket.io";
import type { CarnotStore } from "./services/carnotStore.js";
import type { PermittedCycle } from "@arbx/shared";

const ROOM = "carnot:cycles";

export function registerCarnotWebSocket(
  io: SocketServer,
  store: CarnotStore,
  logger: { info: (obj: object, msg?: string) => void }
): void {
  io.on("connection", (socket) => {
    socket.on("subscribe:carnot", () => {
      socket.join(ROOM);
      socket.emit("carnot:snapshot", store.snapshot());
      socket.emit("carnot:subscribed");
      logger.info({ event: "carnot.ws.subscribe", socket: socket.id });
    });

    socket.on("unsubscribe:carnot", () => {
      socket.leave(ROOM);
    });
  });
}

export function broadcastCarnotCycle(
  io: SocketServer,
  cycle: PermittedCycle,
  logger: { info: (obj: object, msg?: string) => void }
): void {
  io.to(ROOM).emit("carnot:cycle", cycle);
  logger.info({ event: "carnot.ws.broadcast", cycle_id: cycle.id });
}
