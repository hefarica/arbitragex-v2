import { io } from "socket.io-client";

const socket = io("https://edge-arbx.ape-tv.net", {
  transports: ["websocket", "polling"]
});

socket.on("connect", () => {
  console.log("Connected to Edge WebSocket:", socket.id);
  socket.emit("subscribe:opportunities");
});

socket.on("new_opportunity", (data) => {
  console.log("Received opportunity:", data);
});

socket.on("connect_error", (err) => {
  console.error("Connection error:", err.message);
});

setTimeout(() => {
  console.log("Test finished.");
  process.exit(0);
}, 10000);
