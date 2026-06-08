---
name: arbitragex-v2-ops-es
description: "[ES] Documentación operacional completa del proyecto ArbitrageX-v2. Use para acceder a la arquitectura del sistema, comprender el runtime de cartuchos, desplegar código, ejecutar tareas comunes, diagnosticar problemas, y gestionar servicios Docker. Incluye autenticación segura, estructura del proyecto, workflow de despliegue, y best practices."
---

# ArbitrageX V2 Operations

## Overview

Este skill documenta todo el conocimiento operacional del proyecto **ArbitrageX-v2**, un sistema de arbitraje descentralizado con motor dinámico de estrategias. Proporciona acceso a la arquitectura del sistema, configuración de servicios, runtime de cartuchos (FASE OMEGA), y procedimientos operacionales esenciales.

## Quick Reference

| Componente | Acceso | Estado |
|-----------|--------|--------|
| **VPS Principal** | `195.201.235.70` (Hetzner CX43, Falkenstein) | Producción |
| **SSH Key** | Almacenada en Manus secrets (ED25519) | Seguro |
| **GitHub Token** | Almacenado en Manus secrets (ghp_...) | Seguro |
| **API Base URL** | `https://arbitragex.example.com` | Activo |
| **Runtime Motor** | Rhai (FASE OMEGA) | Dinámico |

## Authentication & Access

### SSH Access

El acceso SSH utiliza una clave ED25519 almacenada de forma segura en Manus secrets. Todos los secretos se manejan sin exponerlos en el skill.

```bash
ssh -i ~/.ssh/arbitragex_ed25519 ubuntu@195.201.235.70
```

### GitHub Token

El token de GitHub (`ghp_...`) está almacenado en Manus secrets para operaciones de repositorio seguras.

## Project Structure

La estructura del proyecto ArbitrageX-v2 organiza componentes clave en directorios funcionales:

- **`/cartridges`** - Estrategias dinámicas en formato Rhai
- **`/api`** - Servicios API principales
- **`/searcher-rs`** - Motor de búsqueda en Rust
- **`/frontend`** - Interfaz de usuario (Edge/Frontend)
- **`/nginx`** - Configuración de proxy inverso
- **`/docker`** - Definiciones de servicios Docker
- **`/config`** - Variables de entorno y configuración
- **`/monitoring`** - Health checks y métricas

## Docker Services

Todos los servicios corren en contenedores Docker orquestados. Servicios principales:

1. **API Service** - Servidor principal de aplicación
2. **Searcher-RS** - Motor de búsqueda de oportunidades
3. **Redis** - Cache y PubSub para hot-reload
4. **PostgreSQL** - Base de datos principal
5. **Nginx** - Proxy inverso y balanceador
6. **Cloudflare** - CDN y seguridad perimetral

### Gestionar Servicios

```bash
# Ver estado de servicios
docker-compose ps

# Reiniciar un servicio específico
docker-compose restart api

# Ver logs en tiempo real
docker-compose logs -f searcher-rs

# Detener todos los servicios
docker-compose down

# Iniciar todos los servicios
docker-compose up -d
```

## Cartridge Runtime (FASE OMEGA)

El **Cartridge Runtime** es el motor dinámico que ejecuta estrategias de arbitraje. Implementa sandboxing seguro con Rhai (lenguaje de scripting embebido) y host bindings para operaciones nativas.

### Estructura de Cartuchos

Cada cartucho (estrategia) requiere 3 funciones principales:

1. **`init_strategy()`** - Inicialización y configuración
2. **`evaluate_opportunity()`** - Evaluación de oportunidades
3. **`build_payload()`** - Construcción de transacciones

### Host Bindings (Funciones Nativas)

El runtime expone 15+ funciones nativas para interacción con el sistema:

- `fetch_price(chain, token)` - Obtener precios
- `check_liquidity(pool_id)` - Verificar liquidez
- `estimate_gas(chain)` - Estimar costos de gas
- `execute_swap(payload)` - Ejecutar intercambios
- `log_event(message)` - Registrar eventos
- Y más...

### Tipos de Datos Rhai

Los cartuchos utilizan tipos de datos Rhai nativos:

- `string`, `int`, `float`, `bool`
- `array`, `map`, `object`
- Tipos personalizados para datos de blockchain

### Patrones Comunes

```rhai
// Condicionales
if opportunity.profit > min_profit {
    // ejecutar
}

// Loops
for token in tokens {
    let price = fetch_price(chain, token);
}

// Maps
let prices = map();
prices["ETH"] = 1500.0;
```

### Ejemplos Completos

#### DEX Arbitrage

Estrategia de arbitraje entre DEX (Uniswap, SushiSwap, etc.):

```rhai
fn init_strategy() {
    return #{
        name: "dex_arbitrage",
        chains: ["ethereum", "polygon"],
        min_profit_bps: 50
    };
}

fn evaluate_opportunity(market_data) {
    let price_a = market_data.uniswap_price;
    let price_b = market_data.sushiswap_price;
    
    if price_a > price_b {
        return #{
            type: "buy_b_sell_a",
            profit_bps: calculate_profit(price_a, price_b)
        };
    }
    return null;
}

fn build_payload(opportunity) {
    return #{
        swaps: [
            { dex: "sushiswap", action: "buy", amount: 100 },
            { dex: "uniswap", action: "sell", amount: 100 }
        ]
    };
}
```

#### Triangular Arbitrage

Estrategia de arbitraje triangular (A → B → C → A):

```rhai
fn init_strategy() {
    return #{
        name: "triangular_arbitrage",
        tokens: ["USDC", "ETH", "DAI"],
        min_profit_bps: 30
    };
}

fn evaluate_opportunity(prices) {
    let path_profit = (prices.usdc_to_eth * prices.eth_to_dai * prices.dai_to_usdc) - 1.0;
    
    if path_profit > 0.003 {
        return #{
            path: "USDC → ETH → DAI → USDC",
            profit_ratio: path_profit
        };
    }
    return null;
}

fn build_payload(opportunity) {
    return #{
        route: opportunity.path,
        amounts: calculate_amounts(opportunity.profit_ratio)
    };
}
```

### Constraints y Límites

- **Timeout de ejecución:** 5 segundos por cartucho
- **Memoria máxima:** 256 MB por instancia
- **Llamadas de host:** Máximo 100 por ejecución
- **Tamaño de payload:** 10 MB máximo

### Manejo de Errores

```rhai
try {
    let result = execute_swap(payload);
} catch (error) {
    log_event("Error en swap: " + error);
    return null;
}
```

### Best Practices

1. **Validación temprana** - Validar datos de entrada antes de procesamiento
2. **Logging detallado** - Registrar decisiones clave para debugging
3. **Manejo de edge cases** - Considerar slippage, gas, y condiciones de mercado
4. **Optimización de gas** - Minimizar operaciones on-chain
5. **Testing** - Validar cartuchos en testnet antes de producción

## Deployment Workflow

### Pasos para Desplegar Código

1. **Preparar cambios**
   ```bash
   git checkout -b feature/nueva-estrategia
   # hacer cambios
   git add .
   git commit -m "Agregar nueva estrategia"
   ```

2. **Push a repositorio**
   ```bash
   git push origin feature/nueva-estrategia
   ```

3. **Crear Pull Request**
   - Describir cambios y rationale
   - Esperar revisión

4. **Merge a main**
   ```bash
   git checkout main
   git pull origin main
   git merge feature/nueva-estrategia
   ```

5. **Desplegar a producción**
   ```bash
   ssh ubuntu@195.201.235.70
   cd /opt/arbitragex-v2
   git pull origin main
   docker-compose pull
   docker-compose up -d
   ```

6. **Validar despliegue**
   ```bash
   # Verificar salud de servicios
   curl https://arbitragex.example.com/health
   ```

## Common Tasks

### Inyectar Nuevo Cartucho

```bash
# 1. Crear archivo de cartucho
cat > /opt/arbitragex-v2/cartridges/nueva_estrategia.rhai << 'EOF'
fn init_strategy() { ... }
fn evaluate_opportunity(data) { ... }
fn build_payload(opp) { ... }
EOF

# 2. Registrar en base de datos
psql -h localhost -U arbitragex -d arbitragex_db -c \
  "INSERT INTO cartridge_registry (name, path, status) VALUES ('nueva_estrategia', '/cartridges/nueva_estrategia.rhai', 'active');"

# 3. Trigger hot-reload vía Redis
redis-cli PUBLISH cartridge:reload "nueva_estrategia"
```

### Verificación de Salud

```bash
# Health check completo
curl https://arbitragex.example.com/health

# Respuesta esperada:
# {
#   "status": "healthy",
#   "services": {
#     "api": "ok",
#     "searcher": "ok",
#     "database": "ok",
#     "cache": "ok"
#   }
# }
```

### Monitoreo de Métricas

```bash
# Ver métricas en tiempo real
curl https://arbitragex.example.com/metrics

# Métricas clave:
# - cartridge_executions_total
# - cartridge_success_rate
# - average_execution_time_ms
# - total_profit_usd
```

### Revisar Logs de Auditoría

```bash
# Últimas 100 ejecuciones
psql -h localhost -U arbitragex -d arbitragex_db -c \
  "SELECT timestamp, cartridge_name, status, profit FROM audit_log ORDER BY timestamp DESC LIMIT 100;"
```

## Troubleshooting

### Problema: Cartuchos no se ejecutan

**Síntomas:** Las estrategias no se están ejecutando, logs muestran "no active cartridges"

**Solución:**
```bash
# 1. Verificar estado del runtime
docker-compose logs api | grep "cartridge runtime"

# 2. Verificar cartuchos registrados
psql -h localhost -U arbitragex -d arbitragex_db -c \
  "SELECT name, status FROM cartridge_registry;"

# 3. Reactivar cartuchos si es necesario
psql -h localhost -U arbitragex -d arbitragex_db -c \
  "UPDATE cartridge_registry SET status='active' WHERE status='inactive';"

# 4. Reiniciar servicio API
docker-compose restart api
```

### Problema: Alto uso de memoria

**Síntomas:** Contenedores usando >80% de memoria disponible

**Solución:**
```bash
# 1. Identificar contenedor problemático
docker stats

# 2. Revisar cartuchos con memory leaks
docker-compose logs searcher-rs | grep "memory"

# 3. Reiniciar contenedor
docker-compose restart searcher-rs

# 4. Considerar optimizar cartuchos o aumentar recursos
```

### Problema: Errores de conexión a base de datos

**Síntomas:** "Connection refused" o "database unavailable"

**Solución:**
```bash
# 1. Verificar estado de PostgreSQL
docker-compose ps | grep postgres

# 2. Revisar logs
docker-compose logs postgres

# 3. Reiniciar si es necesario
docker-compose restart postgres

# 4. Verificar conectividad
psql -h localhost -U arbitragex -d arbitragex_db -c "SELECT 1;"
```

### Problema: Hot-reload no funciona

**Síntomas:** Cambios de cartuchos no se aplican sin reiniciar

**Solución:**
```bash
# 1. Verificar Redis está activo
redis-cli ping

# 2. Verificar suscripción a canal
redis-cli PUBSUB CHANNELS

# 3. Trigger manual de reload
redis-cli PUBLISH cartridge:reload "*"

# 4. Revisar logs de API
docker-compose logs api | grep "reload"
```

## Environment Variables

Configuración requerida en `/opt/arbitragex-v2/.env`:

```bash
# Base de datos
DATABASE_URL=postgresql://arbitragex:password@postgres:5432/arbitragex_db
REDIS_URL=redis://redis:6379/0

# API
API_PORT=8080
API_HOST=0.0.0.0
LOG_LEVEL=info

# Blockchain
RPC_ETHEREUM=https://eth-mainnet.g.alchemy.com/v2/KEY
RPC_POLYGON=https://polygon-mainnet.g.alchemy.com/v2/KEY

# Seguridad
JWT_SECRET=your-secret-key-here
ADMIN_TOKEN=your-admin-token

# Monitoreo
SENTRY_DSN=https://key@sentry.io/project-id
METRICS_ENABLED=true
```

## Monitoring

### Health Checks

El sistema ejecuta health checks continuos en todos los servicios:

```bash
# Endpoint de health
GET /health

# Respuesta:
{
  "status": "healthy",
  "timestamp": "2026-05-31T17:48:59Z",
  "services": {
    "api": { "status": "ok", "response_time_ms": 2 },
    "searcher": { "status": "ok", "response_time_ms": 5 },
    "database": { "status": "ok", "response_time_ms": 3 },
    "cache": { "status": "ok", "response_time_ms": 1 }
  }
}
```

### Métricas Clave

- **Cartridge Executions** - Total de ejecuciones por período
- **Success Rate** - Porcentaje de ejecuciones exitosas
- **Average Execution Time** - Tiempo promedio de ejecución
- **Total Profit** - Ganancia acumulada en USD
- **Gas Costs** - Costos totales de gas
- **Error Rate** - Porcentaje de errores

### Alertas Automáticas

El sistema genera alertas para:

- Tasa de error > 5%
- Tiempo de respuesta > 1000ms
- Uso de memoria > 85%
- Espacio en disco < 10%
- Servicios no disponibles

## References

Para información detallada, consultar:

- **`references/architecture.md`** - Diagrama completo del sistema, flujos de datos, schema de base de datos, pipeline de despliegue
- **`references/cartridge_api.md`** - Documentación completa de API de cartuchos, host bindings, tipos de datos, ejemplos avanzados

---

**Nota de Seguridad:** Todos los secretos (SSH keys, tokens, credenciales) se manejan de forma segura sin exponerlos en este skill. Acceder siempre a través de Manus secrets.
