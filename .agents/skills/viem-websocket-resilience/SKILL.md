# Resiliencia de WebSockets en Viem

## Nivel
Nivel intermedio/avanzado.

## Propósito
Implementar workarounds y configuraciones nativas para evitar el "Silent Disconnect" de los WebSockets en `viem`, asegurando que las aplicaciones frontend o scripts en Node.js siempre tengan datos frescos sin tener que recargar la página.

## Fuente de aprendizaje
https://github.com/wevm/viem/issues/2563
https://github.com/wevm/viem/issues/2325

## Conocimiento interiorizado
- **Problema de reconexión (#2563, #2325)**: Históricamente, `viem` en versiones 2.13 a 2.18 tenía problemas donde si un WebSocket (ej. Alchemy) se cerraba inesperadamente, el cliente no lo detectaba o no intentaba reconectar, dejando a la dApp congelada.
- **Solución Nivel Librería**: Configurar el transporte con `retryCount` alto y `keepAlive: true`.
- **Solución Nivel Aplicación (Workaround)**: Para dApps críticas, no se debe confiar 100% en la reconexión oculta de la librería. Se debe mantener una referencia al WebSocket nativo o implementar un intervalo de Ping/Pong manual a nivel de aplicación, y si falla, recrear la instancia del cliente `viem` por completo.

## Cuándo activar esta skill
- Al conectar el frontend (`page.tsx`) a la red blockchain.
- Al escribir un bot en TypeScript usando Node.js y `viem` `webSocket` transport.
- Al observar que la dApp deja de actualizar bloques después de 15 minutos.

## Cuándo no activar esta skill
- Si la app hace llamadas de lectura HTTP únicas.

## Entradas necesarias
- Proveedor WSS.
- Lógica de la interfaz o bot.

## Procedimiento paso a paso
1. Instanciar el `webSocket` transport pasando `{ keepAlive: true, retryCount: 5, retryDelay: 1000 }`.
2. Para máxima seguridad, escuchar el evento `watchBlockNumber`. Si pasan más de N segundos (ej. 30s en Ethereum donde los bloques son cada 12s) sin recibir un bloque, forzar la destrucción y recreación del cliente.
3. Emitir un estado "RECONNECTING" al Frontend para que el usuario sepa que la red se pausó.

## Salidas esperadas
- Configuración de transporte en TypeScript que sobrevive caídas de red.

## Aplicación al proyecto actual
Aplicable en `frontend/lib/api-client.ts` o en cualquier lugar de `ArbitrageX` que use WebSockets directamente hacia Alchemy (aunque la mayoría de la lógica de MEV está delegada en Rust, si el UI usa Viem para leer saldo, debe usar esto).

## Aplicación a futuros proyectos
Cualquier frontend Web3 moderno en React/Next.js.

## Buenas prácticas
- Mostrar un indicador visual (Connection Status Badge) si el WebSocket falla.
- Tratar los RPCs de WebSockets gratuitos como inherentemente inestables.

## Errores comunes
- Asumir que `viem` reconectará siempre automáticamente en versiones viejas.
- Crear una nueva instancia de `PublicClient` en cada re-render de React.

## Riesgos técnicos
- Fuga de memoria en React (Memory Leak) si se crean nuevos clientes Viem repetidamente sin desuscribirse (`unwatch`) de los anteriores.

## Riesgos legales, éticos o financieros
- Si el precio de un oráculo mostrado en UI está "stale" (congelado por desconexión de WS), un usuario podría tomar decisiones financieras erróneas pensando que el precio es real.

## Controles de seguridad
- Detectar "Staleness": Si la data es muy vieja, ocultar los botones de ejecución ("Trade").

## Checklist operativo
- [ ] Transport configurado con `keepAlive: true`.
- [ ] Timeout de estancamiento implementado (ej. no blocks in 30s).
- [ ] Lógica de limpieza en `useEffect` de React (`unwatch()`).

## Ejemplo seguro
Ver `examples.md`.

## Dependencias
- `viem` >= 2.x

## Métricas de calidad
- El frontend recupera la lectura on-chain en <5s tras restaurar la conexión de red simulada (desactivar WiFi local).

## Criterios de finalización
- Implementación de un hook custom `useRobustWebSocket` o configuración en `viem` que pase pruebas de red cortada.
