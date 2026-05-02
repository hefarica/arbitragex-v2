import { Server } from 'socket.io';
import { Server as HttpServer } from 'http';

export function setupWebSocketGateway(server: HttpServer) {
    const io = new Server(server, {
        cors: { origin: '*' } // Configurado para aceptar conexiones del dashboard local
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
