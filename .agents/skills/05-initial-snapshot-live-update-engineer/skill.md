# Skill 05: Initial Snapshot + Live Update Engineer

## 1. Propósito
Diseñar el patrón puente más robusto para arquitecturas Full-Stack de Alta Frecuencia: Cargar el estado inicial sólido (Snapshot) de manera segura y rápida vía API/SSR, y posteriormente conectarse por WebSocket para procesar incrementos (Deltas/Updates) sin interrupciones ni dobles registros.

## 2. Aplicación directa en ARBITRAGEX
El panel principal de **ArbitrageX** escucha mempool y relays. Las oportunidades deben cargar instantáneamente el último estado retenido en Redis (Snapshot: Las top 50 oportunidades activas de los últimos 15 min). Una vez la interfaz es montada, el socket conecta con Edge Server (`subscribe:opportunities`) y recibe stream de updates que se insertan ordenadamente al inicio del array.

## 3. Problemas que resuelve
- Pantalla vacía ("Loading...") mientras el Web Socket negocia el Handshake y conecta (Afecta UX).
- Duplicación de datos: La oportunidad recibida en el Snapshot inicial se recibe otra vez por el stream del Socket y la interfaz la muestra dos veces.
- Brechas de información (Gap): Oportunidades generadas entre la consulta del Snapshot HTTP y la conexión real del WebSocket.

## 4. Reglas Inmutables
- Cargar un **Snapshot Initial** siempre, preferentemente vía Fetch GET (SSR o On Mount).
- El Payload de Update del WebSocket y el Payload del Snapshot deben compartir **exactamente el mismo Esquema TypeScript** o Zod.
- Toda entidad conectada en tiempo real debe tener un identificador único globalmente (`id` o `uuid`).
- El reducer del estado en el cliente DEBE deducir y evitar duplicados utilizando el `id` o versionado. Si un nuevo dato llega con el mismo ID, reemplaza o ignora el actual.

## 5. Nivel de Madurez
Maestría - Esta es la arquitectura crítica requerida por Exchanges, Trading Bots y Paneles HFT (High-Frequency Trading).
