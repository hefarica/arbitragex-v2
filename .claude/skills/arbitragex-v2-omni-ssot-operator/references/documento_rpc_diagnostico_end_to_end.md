# Diagnóstico e Implementación End-to-End: RPC_HTTP_1 y RPC_WS_1

**Fecha:** 2026-05-23  
**Sistema:** ArbitrageX v2 — VPS <VPS_IP>

---

## 1. El Diagnóstico del Bloqueo (Root Cause)

Actualmente el motor principal (`searcher-rs`) está **inactivo**. El análisis forense del código revela exactamente por qué:

1. **La Validación R8 Fail-Honest:** En `backend/searcher-rs/src/main.rs:370`, el orquestador intenta leer `RPC_HTTP_1`. Al no encontrarlo, emite un log de advertencia y **se niega a iniciar los workers** para la cadena 1 (Ethereum Mainnet).
2. **Efecto Cascada:** Sin workers para Ethereum, el sistema activa automáticamente el `paper_mode = true` por seguridad (línea 465) y el pipeline de simulación (`sim-ctl`) se queda sin red.
3. **El Silencio del Mempool:** En `backend/searcher-rs/src/chain_client.rs:116`, el sistema espera `RPC_WS_1` para suscribirse al evento `alchemy_pendingTransactions`. Sin WS, no hay ingesta de datos.

### ¿Cómo llega la variable al contenedor?

1. El archivo físico en el VPS es `/opt/arbitragex-v2/.env`.
2. El archivo `docker/compose.prod.yml` usa la directiva `env_file: ["../.env"]` para inyectar todas las variables a los contenedores.
3. El contenedor `searcher-rs` (y también `token-enricher`) declara explícitamente en su sección `environment:` que requiere estas variables:

```yaml
RPC_WS_1: ${RPC_WS_1:?RPC_WS_1 required for mainnet detection}
RPC_HTTP_1: ${RPC_HTTP_1:?RPC_HTTP_1 required for mainnet receipts}
```

*Nota: El `:?` significa que si la variable está vacía en el `.env`, Docker se negará a arrancar el contenedor.*

---

## 2. El Formato Exacto Requerido (G-RPC-1 Discipline)

El archivo `backend/shared-rs/src/rpc_failover.rs` implementa una disciplina estricta de failover. Espera un formato **CSV (Comma Separated Values)** con pares `nombre=url`.

**Para RPC_HTTP_1:**

```text
RPC_HTTP_1=alchemy=https://eth-mainnet.g.alchemy.com/v2/<TU_API_KEY>,infura=https://mainnet.infura.io/v3/<TU_API_KEY>
```

*(Si solo tienes un proveedor, funciona, pero el sistema emitirá un log de advertencia `rpc_pool.single_vendor` pidiendo un segundo proveedor para redundancia).*

**Para RPC_WS_1:**

```text
RPC_WS_1=alchemy=wss://eth-mainnet.g.alchemy.com/v2/<TU_API_KEY>,infura=wss://mainnet.infura.io/ws/v3/<TU_API_KEY>
```

---

## 3. Plan de Implementación End-to-End

Para cablear esto de extremo a extremo en el VPS sin romper la arquitectura, se deben ejecutar exactamente estos 3 pasos:

### Paso 1: Inyección en el archivo `.env` del VPS

Debemos conectarnos por SSH al VPS y editar el archivo de configuración.

```bash
# Conexión al VPS
ssh root@<VPS_IP>

# Editar el archivo .env
nano /opt/arbitragex-v2/.env
```

Buscar las líneas `RPC_HTTP_1=` y `RPC_WS_1=` y reemplazarlas con tus URLs reales.

### Paso 2: Reinicio Seguro de Contenedores

Dado que modificamos el `.env`, no basta con reiniciar un contenedor; debemos recrear los que dependen de estas variables usando el script oficial idempotente.

```bash
cd /opt/arbitragex-v2
./infra/vps/deploy.sh prod
```

Este script hace un `docker compose pull` y un `up -d --remove-orphans`, inyectando el nuevo `.env` limpiamente en `searcher-rs` y `token-enricher`.

### Paso 3: Verificación de Ingesta (El Latido del Sistema)

Inmediatamente después del reinicio, debemos verificar que el orquestador reconoció el RPC y que el mempool está fluyendo.

```bash
# 1. Verificar que el pool RPC se inicializó correctamente
docker logs arbitragex-v2-searcher-rs-1 | grep "http rpc pool initialized"

# 2. Verificar que la suscripción al mempool está activa
docker logs arbitragex-v2-searcher-rs-1 | grep "filtered mempool subscription active"

# 3. Verificar que las oportunidades empiezan a llegar al API Server
curl -s http://localhost:8080/api/opportunities/live | grep -q '"count": 0' && echo "Aún vacío" || echo "Oportunidades fluyendo"
```

---

## 4. Requisitos para Ejecutar

Para ejecutar este plan, se requieren las URLs del proveedor de nodos, por ejemplo Alchemy, Infura o QuickNode:

| Variable | Ejemplo |
|---|---|
| URL HTTP | `https://eth-mainnet.g.alchemy.com/v2/tu-api-key` |
| URL WSS | `wss://eth-mainnet.g.alchemy.com/v2/tu-api-key` |

Este documento fue descargado desde el enlace público de Manus y normalizado como material de soporte para una skill de operación técnica de ArbitrageX v2.
