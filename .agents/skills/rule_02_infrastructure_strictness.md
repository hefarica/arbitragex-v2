# RULE 02: INFRASTRUCTURE STRICTNESS & ROUTING DOCTRINE

## 1. DEFINICIÓN DE LA REGLA

Esta regla define los principios obligatorios para el ruteo de red (WebSockets vs REST) y el manejo estricto de variables de entorno en sistemas de producción ("no-hardcode doctrine"). Cualquier desviación de esta regla causará caídas del sistema (`ERR_CONNECTION_REFUSED` o Crash on Boot).

## 2. RUTEO DE WEBSOCKETS VS REST (EDGE)

Está **ESTRICTAMENTE PROHIBIDO** enrutar tráfico de WebSockets (ej. `socket.io-client`) a través del Edge Worker (Cloudflare / Hono). El Edge Worker es **EXCLUSIVAMENTE** un proxy REST.

- **Frontend REST**: Debe usar `NEXT_PUBLIC_EDGE_URL` (puerto 8787 localmente o `edge-arbx.ape-tv.net` en remoto).
- **Frontend WebSocket**: Debe usar explícitamente `NEXT_PUBLIC_WS_URL` (puerto 8080 localmente) para apuntar directamente al `api-server` que monta el Gateway de WebSockets.
- **Consecuencia de incumplimiento**: Si el WebSocket apunta al Edge, recibirá errores `404 Not Found` en el polling, saturando la red y previniendo la conexión en tiempo real.

## 3. DOCTRINA "NO-HARDCODE" EN PRODUCCIÓN

Cuando el entorno es `production-like` (determinado por `configs/app.toml`), el sistema **DEBE FALLAR RÁPIDAMENTE (Fail-Fast)** si falta configuración crítica. 

- **Sentinel Signers**: Está **PROHIBIDO** el uso de direcciones centinela de desarrollo (ej. `0x000000000000000000000000000000000000dEaD`) fuera del entorno `development`.
- **Servicios de Simulación (`sim-ctl`)**: Si `SIM_SIGNER_ADDRESS` no está definido en el archivo `.env` del VPS, el servicio entrará en un bucle de error (Crash on Boot) y no arrancará. **Esto es una característica de seguridad, no un bug.**
- **Solución correcta**: NUNCA degradar el entorno de `production-like` a `development` para evadir el error. La única solución válida es inyectar la variable de entorno explícita (ej. `SIM_SIGNER_ADDRESS=<AddressValida>`) en el `.env` del servidor.

## 4. CONSECUENCIAS DE AUDITORÍA
Cualquier intento de "arreglar" un servicio de backend silenciando estas advertencias, o modificando el Rust `bail!()` para aceptar mocks/sentinels en producción, será considerado una violación a la regla madre y un riesgo crítico de seguridad.
