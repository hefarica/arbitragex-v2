# ArbitrageX-V2: Manual Paper-Shadow vs Live Mode

> **Version:** 1.0.0 | **Fecha:** 2026-05-14 | **Doctrina:** Ghost Protocol + R8 Fail-Honest

---

## 1. Visión General

Este documento define los dos modos de operacion de ArbitrageX-V2: **Paper-Shadow** (simulacion sin capital real) y **Live** (operaciones reales con capital). Tambien documenta el **Kill-Switch global**, el **Ghost Protocol** (capital expuesto = 0 en paper), y el procedimiento de transicion segura de paper a live.

### Estado Actual (Sprint 1 - Foundations)

| Componente | Estado | Modo |
|------------|--------|------|
| Deteccion de oportunidades | Esqueleto (S1) | Paper-Shadow |
| Simulacion (sim-ctl) | Esqueleto (S1) - devuelve 501 | Paper-Shadow |
| Ejecucion (relays-client) | Esqueleto (S1) - devuelve 501 | Paper-Shadow |
| Reconciliacion (recon) | Lee DB real (S1) | Paper-Shadow |
| Token enricher | Funcional | Paper-Shadow |

> **⚠️ ADVERTENCIA CRITICA:** En S1-S4, el sistema opera **EXCLUSIVAMENTE** en paper-shadow. La transicion a live requiere cierre completo de S8 con evidencia verificable.

---

## 2. Que es Paper-Shadow Mode

### Definicion

Paper-Shadow es un modo de **simulacion completa** donde:

- **NO se envian transacciones** a la blockchain
- **NO se firma ningun bundle** con claves privadas
- **Capital expuesto = 0** (Ghost Protocol)
- Las oportunidades se detectan, simulan y registran en la base de datos
- Todos los numeros son **estimaciones teoricas**, no ganancias/perdidas reales

### Configuracion Paper-Shadow

```toml
# configs/app.toml — SAFETY DEFAULT
[execution]
paper_mode = true              # Solo el operator cambia a false, manualmente, tras revision
private_only = true            # Solo ejecucion privada (Flashbots, etc.)
max_parallel_executions = 8
max_value_eth = 1.0            # Hard cap por bundle; excedido → reject + risk_event critical
```

```yaml
# docker-compose.edge.yml — Edge Node
sed-core:
  environment:
    - SED_FEATURE=paper-shadow    # Ghost Protocol: CERO capital expuesto
    - SED_EDGE_NODE_ID=edge-001
```

### Variables de Entorno para Paper-Shadow

```bash
# .env
ENV=development                  # o staging; NUNCA production para paper tests
NODE_ENV=development
RUST_LOG=info

# RPC requeridos (para leer estado on-chain)
RPC_HTTP_1=alchemy=https://eth-mainnet.g.alchemy.com/v2/<KEY>
RPC_WS_1=alchemy=wss://eth-mainnet.g.alchemy.com/v2/<KEY>

# Paper-Shadow NO necesita:
# FLASHBOTS_SIGNER_KEY, BLOXROUTE_AUTH, EDEN_AUTH
```

### Ghost Protocol

> **⚠️ GHOST PROTOCOL — Capital Expuesto = 0 en Paper-Shadow**

El Ghost Protocol es una garantia arquitectonica:

| Componente | Comportamiento en Paper-Shadow |
|------------|-------------------------------|
| `sed-core` | `SED_FEATURE=paper-shadow` — simula transacciones via REVM, nunca envia |
| `relays-client` | No se inicializa en paper (S5 requiere esta clave) |
| `FLASHBOTS_SIGNER_KEY` | No se lee, no se requiere |
| Wallet de ejecucion | No existe en paper |
| Balance on-chain | Siempre 0 en paper |
| `sim-ctl` | Simula via Anvil fork o Tenderly (S4+) |

---

## 3. Que es Live Mode

### Definicion

Live Mode es el modo de **operacion real** donde:

- **Se envian transacciones** a la blockchain via Flashbots, BloxRoute, Eden
- **Se firman bundles** con `FLASHBOTS_SIGNER_KEY`
- **Capital real esta expuesto** a slippage, revert rates, y fees de gas
- **Las ganancias/perdidas son reales** (ETH/USDC reales)
- **El riesgo es real** — contratos inteligentes, MEV, slippage

### Requisitos Previos para Live (Checklist S8)

| # | Requisito | Sprint | Evidencia |
|---|-----------|--------|-----------|
| 1 | Deteccion funcional con mempool real | S2 | `/status` muestra oportunidades detectadas |
| 2 | Simulacion con Anvil/Tenderly | S4 | `sim-ctl` devuelve sim_results reales (no 501) |
| 3 | Ejecucion con Flashbots | S5 | Bundles firmados y enviados a relay |
| 4 | Reconciliacion PnL | S6 | `/recon/summary` muestra PnL real con < 20% varianza |
| 5 | Edge + Frontend operativo | S7 | Frontend consume edge, edge valida tokens |
| 6 | Observabilidad E2E + Governance | S8 | Grafana muestra todas las metricas; Vault sellado |
| 7 | Kill-Switch testeado y funcional | S8 | Prueba de armar/desarmar killswitch documentada |
| 8 | Auditoria de seguridad completada | S8 | Reporte de auditoria firmado |
| 9 | Paper-Shadow rentable por 30 dias | S8 | Historial de recon por 30 dias con profit positivo |
| 10 | Fondo de emergencia disponible | S8 | Reserva para cubrir perdidas iniciales |

---

## 4. Como Activar Cada Modo

### 4.1 Activar Paper-Shadow (DEFAULT - ya activo)

El sistema viene con `paper_mode = true` por defecto en `configs/app.toml`.

**Verificar estado actual:**
```bash
# Via API
curl http://localhost:8080/api/v1/config/current | jq '.execution.paper_mode'
# Debe devolver: true

# Verificar por chain
curl http://localhost:8080/api/v1/config/current | jq '.execution.paper_mode_per_chain'
```

**Activar paper-shadow via API (per-chain):**
```bash
# POST /admin/config/paper-mode (B0.2 — isolation per-chain)
# Se REQUIERE chain_id — ya no existen cambios globales
curl -X POST http://localhost:8080/admin/config/paper-mode \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "x-arbx-actor: operator-name" \
  -d '{
    "enabled": true,
    "chain_id": 1
  }'
```

Respuesta:
```json
{
  "enabled": true,
  "updated_at": "2026-05-14T10:30:00Z",
  "updated_by": "operator-name",
  "chain_id": 1,
  "source": "per_chain",
  "key": "arbx:papermode:1"
}
```

**Activar paper-shadow via Redis (directo):**
```bash
redis-cli SET arbx:papermode:1 '{"enabled":true,"updated_at":"2026-05-14T10:30:00Z","updated_by":"operator","chain_id":1}'
redis-cli PUBLISH arbx:papermode:1:changes '{"enabled":true,"updated_at":"2026-05-14T10:30:00Z","updated_by":"operator","chain_id":1}'
```

### 4.2 Activar Live Mode (PER-CHAIN — B0.2 Isolation)

> **⚠️ ADVERTENCIA CRITICA:** Live mode SOLO se activa por chain individual. NUNCA se activa globalmente. Esto evita el "footgun de flip global".

```bash
# Desactivar paper-mode para chain 1 (Ethereum mainnet)
curl -X POST http://localhost:8080/admin/config/paper-mode \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "x-arbx-actor: operator-name" \
  -d '{
    "enabled": false,
    "chain_id": 1
  }'
```

**Que sucede al desactivar paper-mode:**
1. Redis key `arbx:papermode:1` se actualiza a `{"enabled":false,...}`
2. Redis channel `arbx:papermode:1:changes` recibe el broadcast
3. `searcher-rs` suscrito al canal actualiza su estado
4. `relays-client` se inicializa para envio de bundles firmados
5. `FLASHBOTS_SIGNER_KEY` se vuelve requerida y validada
6. Nuevas oportunidades en chain 1 seran evaluadas para ejecucion REAL

**Requisitos para que Live funcione en una chain:**
```bash
# Verificar que TODOS estos checks pasan para la chain
POSTGRES_PASSWORD       → configurado
DATABASE_URL            → conectable
REDIS_URL               → conectable
RPC_HTTP_<chain_id>     → al menos 2 proveedores
RPC_WS_<chain_id>       → al menos 1 proveedor
FLASHBOTS_SIGNER_KEY    → presente (S5+)
BLOXROUTE_AUTH          → presente (si se usa BloxRoute)
EDEN_AUTH               → presente (si se usa Eden)
```

---

## 5. Kill-Switch Global

### 5.1 Estado del Kill-Switch

El killswitch es un mecanismo de **fail-closed** que bloquea TODAS las ejecuciones cuando esta armado.

**Precedencia de estado (audit B10, 2026-05-10):**

| Prioridad | Fuente | Mutable en runtime | Persistencia |
|-----------|--------|-------------------|--------------|
| 1 | Redis key `arbx:killswitch:enabled` | **Si** (canonical) | TTL en Redis |
| 2 | Archivo `killswitch.json` (repo root) | No (legacy fallback) | Solo en boot si Redis unreachable |
| 3 | `configs/app.toml` `kill_switch_enabled_default` | No (config) | Valor por defecto |

### 5.2 Estado del archivo killswitch.json

```json
{"enabled":false,"reason":"disabled","updated_at":"2026-05-02T18:50:00Z"}
```

> **NOTA:** Editar este archivo NO afecta un servicio en ejecucion. Redis es la fuente de verdad en runtime.

### 5.3 Armar Kill-Switch

```bash
# Via API (metodo canonical)
curl -X POST http://localhost:8080/admin/killswitch \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "x-arbx-actor: operator-name" \
  -d '{
    "enabled": true,
    "reason": "alta revert rate detectada — investigacion",
    "triggered_by": "operator-name"
  }'
```

Respuesta:
```json
{
  "enabled": true,
  "reason": "alta revert rate detectada — investigacion",
  "triggered_by": "operator-name",
  "updated_at": "2026-05-14T10:30:00Z"
}
```

**Sintomas cuando el killswitch esta ARMADO:**
- Slack `#arbx-alerts` recibe: *"Kill switch is ON — All executions are refused"*
- Grafana panel "Kill-switch" es **rojo / ARMED**
- `/killswitch` page muestra banner "ARMED — executions blocked"
- `arbx_execution_total{status=~"submitted|included"}` deja de incrementar

### 5.4 Desarmar Kill-Switch

```bash
curl -X POST http://localhost:8080/admin/killswitch \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "x-arbx-actor: operator-name" \
  -d '{
    "enabled": false,
    "reason": "auto-trip recovered — revert rate cayo por debajo del umbral",
    "triggered_by": "operator-name"
  }'
```

> **⚠️ REGLA DE ORO:** Quien arme el killswitch DEBE ser quien lo desarme. No desarmar en nombre de otro operador.

### 5.5 Auto-Trip por Revert Rate

```toml
# configs/app.toml
[recon]
auto_trip_on_high_revert_rate = true   # Plataforma se auto-arma
anomaly_revert_rate_pct = 50.0         # Umbral de disparo
```

Cuando `recon` detecta revert rate > 50% en la ventana de 15 minutos, la plataforma se auto-arma y registra el evento en `risk_events`.

### 5.6 Kill-Switch en Paper-Shadow

En modo paper-shadow, el killswitch:
- **Si funciona** — bloquea incluso las simulaciones
- **Protege** contra gasto innecesario de recursos (RPC calls, compute)
- **Registra** en audit_log todas las activaciones
- Es util para testing del mecanismo antes de live

---

## 6. Transicion de Paper a Live: Checklist de Verificacion

### Fase 1: Pre-Condiciones (S1-S7 completos)

- [ ] Todos los sprints S1-S7 estan completos con evidencia
- [ ] Auditoria de seguridad S8 firmada
- [ ] 30 dias de paper-shadow rentable
- [ ] Kill-switch testeado y funcional

### Fase 2: Verificacion de Infraestructura

- [ ] Vault esta sellado y healthy (`vault operator status`)
- [ ] PostgreSQL tiene roles `arbx_migrator`, `arbx_rw`, `arbx_ro` con passwords seguros
- [ ] Redis responde a `PING`
- [ ] RPC tiene al menos 2 proveedores por chain activa
- [ ] Todos los servicios reportan UP en `/status`

### Fase 3: Verificacion de Secretos

- [ ] `FLASHBOTS_SIGNER_KEY` generada y almacenada en Vault
- [ ] Clave tiene **balance = 0** (nunca debe tener fondos — solo firma)
- [ ] `BLOXROUTE_AUTH` configurado (si se usa BloxRoute)
- [ ] `EDEN_AUTH` configurado (si se usa Eden)
- [ ] Todas las credenciales T0/T1 rotadas en los ultimos 30 dias

### Fase 4: Verificacion de Configuracion

- [ ] `configs/app.toml` tiene `private_only = true`
- [ ] `configs/app.toml` tiene `max_value_eth = 1.0` (cap inicial)
- [ ] `configs/app.toml` tiene `max_revert_rate_pct = 5.0`
- [ ] Trading config para la chain tiene `capital_usd` > 0
- [ ] Trading config tiene `min_profit_usd` > 0
- [ ] `enabled_strategies` esta configurado

### Fase 5: Verificacion On-Chain

- [ ] `FLASHBOTS_SIGNER_KEY` address tiene balance = 0 verificado en block explorer
- [ ] Flashbots relay responde: `curl https://relay.flashbots.net/` devuelve 200
- [ ] Al menos un pool activo para la chain (`/api/v1/pools?chain_id=1` devuelve pools)

### Fase 6: Activacion Controlada (una chain a la vez)

```bash
# 1. Elegir chain con menor riesgo (ej: Ethereum mainnet)
CHAIN_ID=1

# 2. Verificar que paper-mode esta activo para la chain
curl http://localhost:8080/api/v1/config/current | jq ".execution.paper_mode_per_chain[\"$CHAIN_ID\"]"
# Esperado: {"enabled":true,...}

# 3. Armar killswitch (precaucion)
curl -X POST http://localhost:8080/admin/killswitch \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{"enabled":true,"reason":"pre-live safety check","triggered_by":"operator"}'

# 4. Desactivar paper-mode SOLO para esa chain
curl -X POST http://localhost:8080/admin/config/paper-mode \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "x-arbx-actor: operator-name" \
  -d "{\"enabled\":false,\"chain_id\":$CHAIN_ID}"

# 5. Monitorear logs por 5 minutos sin desarmar killswitch
# Verificar que NO hay errores de inicializacion

# 6. Desarmar killswitch y monitorear
# EMPEZAR CON capital_usd MUY BAJO ($100 maximo)
```

### Fase 7: Monitoreo Post-Activacion

- [ ] Primeras 24h: `capital_usd <= $500`
- [ ] Revert rate < 5% en primeras 24h
- [ ] PnL positivo o neutro (no perdidas significativas)
- [ ] `/recon/summary` muestra datos consistentes
- [ ] No hay risk_events con severity `critical`
- [ ] Grafana dashboards muestran metricas normales

### Fase 8: Escalado (si todo esta bien)

| Dia | capital_usd max | max_value_eth | Observacion |
|-----|----------------|---------------|-------------|
| 1-3 | $500 | 0.1 | Monitoreo intensivo |
| 4-7 | $1,000 | 0.25 | Revisar revert rate |
| 8-14 | $2,500 | 0.5 | Si revert rate < 3% |
| 15-30 | $5,000 | 1.0 | Si PnL positivo sostenido |
| 30+ | A discrecion del operator | 1.0 | Basado en rendimiento |

---

## 7. Endpoints Relacionados

| Metodo | Endpoint | Auth | Proposito |
|--------|----------|------|-----------|
| POST | `/admin/killswitch` | Admin token | Armar/desarmar killswitch |
| GET | `/status` | Publico | Ver estado de killswitch + servicios |
| POST | `/admin/config/paper-mode` | Admin token | Activar/desactivar paper-mode per-chain |
| GET | `/api/v1/config/current` | Publico | Ver configuracion actual (incl. paper_mode_per_chain) |
| GET | `/admin/config` | Admin token | Ver configuracion completa (sin secretos) |
| GET | `/api/v1/risk/alerts` | Publico | Ver alertas de riesgo |
| GET | `/api/v1/recon/summary` | Publico | Resumen de reconciliacion PnL |
| GET | `/api/v1/executions/recent` | Publico | Ver ejecuciones recientes |

---

## 8. Troubleshooting

| Sintoma | Causa probable | Solucion |
|---------|---------------|----------|
| `paper_mode` siempre true en GET | Redis tiene key `arbx:papermode:1` = true | Revisar `redis-cli GET arbx:papermode:1` |
| Cambio de paper-mode no tiene efecto | `searcher-rs` no recibio broadcast | Verificar `redis-cli PUBLISH` llego a suscriptores |
| Killswitch no responde | Redis no disponible | Verificar `redis-cli PING` |
| `POST /admin/config/paper-mode` devuelve 400 | Falta `chain_id` en body (B0.2) | Agregar `"chain_id": 1` al body |
| `POST /admin/config/paper-mode` devuelve 503 | Redis no disponible | Verificar REDIS_URL y estado del container |
| Live mode envia bundles sin firmar | `FLASHBOTS_SIGNER_KEY` no configurado | Verificar variable de entorno + restart relays-client |
| `max_value_eth` excedido en live | Bundle supera cap | Reducir capital en trading config para la chain |
| Revert rate > 50% en live | Slippage, MEV, pools poco liquidas | Armar killswitch, volver a paper, ajustar parametros |

---

## 9. Auditoria y Compliance

Toda accion de killswitch o paper-mode se registra en `audit_log`:

```sql
-- Ver historia de killswitch
SELECT actor, action, after_state, created_at
FROM audit_log
WHERE action LIKE '%killswitch%'
ORDER BY created_at DESC
LIMIT 20;

-- Ver historia de paper-mode
SELECT actor, action, target_id, after_state, created_at
FROM audit_log
WHERE action = 'config.papermode.update'
ORDER BY created_at DESC;
```

**Alertas configuradas:**
- `KillSwitchActivated`: Se dispara cuando `increase(audit_log_rows{action="killswitch.enable"}[5m]) > 0`
- `NoOpportunitiesDetectedLongWindow`: Se dispara si no se detectan oportunidades en ventana larga

---

*Documento generado el 2026-05-14. Verificar contra ultima version en `docs/operations/` y `docs/runbooks/killswitch-activated.md`.*
