import { Server } from 'socket.io';
import { Server as HttpServer } from 'http';

// API-2: WebSocket CORS allowlist. ALLOWED_ORIGINS=comma,separated,list.
// Empty list = same-origin only (no Origin header allowed).
// "*" is INTENTIONALLY NOT supported — fail-honest.
function parseAllowedOrigins(): string[] {
    const raw = process.env["ALLOWED_ORIGINS"] ?? "";
    return raw.split(',').map((s) => s.trim()).filter(Boolean);
}

export function setupWebSocketGateway(server: HttpServer) {
    const allowed = parseAllowedOrigins();
    const io = new Server(server, {
        cors: {
            origin: (origin, cb) => {
                // Same-origin (no Origin header) is always allowed.
                if (!origin) return cb(null, true);
                if (allowed.includes(origin)) return cb(null, true);
                // Reject — do NOT pass an Error object (Socket.IO logs it noisily and the
                // upstream client just sees a generic 403 either way).
                cb(null, false);
            },
            credentials: true,
        },
    });

    io.on('connection', (socket: any) => {
        console.log(`[WebSocket] Nuevo cliente conectado: ${socket.id}`);

        socket.on('subscribe:opportunities', () => {
            console.log(`[WebSocket] Cliente ${socket.id} se suscribió a Oportunidades`);
            socket.join('opportunities');
        });

        socket.on('subscribe:metrics', () => {
            console.log(`[WebSocket] Cliente ${socket.id} se suscribió a Métricas`);
            socket.join('metrics');
        });

        socket.on('disconnect', () => {
            console.log(`[WebSocket] Cliente desconectado: ${socket.id}`);
        });
    });

    return io;
}

// Simulador de emisión de oportunidades
export function broadcastOpportunity(io: Server, opp: any) {
    io.to('opportunities').emit('new_opportunity', opp);
}
