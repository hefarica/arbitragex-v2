---
name: sop_deployment_operations
description: Cuando se planifique deploy a producción, infraestructura VPS, RPC fallbacks, monitoreo Prometheus/Grafana, o ubicación geográfica de servidores. Activa con triggers "deploy producción", "infra ArbitrageX", "VPS Hetzner AWS", "RPC fallback Alchemy QuickNode", "Prometheus Grafana monitoreo", "ubicación VPS latencia". Trae infraestructura recomendada del SOP §16 + tabla RPC fallback × 3 por cadena.
type: arbx_architecture
source_section: SOP_ArbitrageX_2026.pdf §16
---

# Despliegue y Operaciones

## Infraestructura requerida (§16.1)

| Componente | Especificación | Propósito |
|------------|-----------------|-----------|
| **VPS Principal** | 4 vCPU, 16GB RAM, NVMe SSD | Ejecución searcher + sim-ctl |
| **VPS Backup** | 2 vCPU, 8GB RAM | Failover + monitoreo |
| **VPS CEX Feed** | 2 vCPU, 4GB RAM, cercano al exchange | WebSocket price feeds CEX |
| **RPC Nodes** | Dedicados (Alchemy / QuickNode) | Latencia < 10ms |
| **Red** | Dedicada, baja latencia | Comunicación entre componentes |

## Ubicación VPS por blockchain (§16.2)

Crítico minimizar latencia. Nodos RPC más rápidos:

- **Ethereum L1**: AWS us-east-1 (Virginia) o eu-central-1 (Frankfurt). Latencia objetivo **< 20ms** al RPC.
- **Arbitrum**: VPS en misma región del sequencer (us-east-1).
- **Base**: VPS cercano al RPC de Base (us-west-2 recomendado).
- **CEX Feed**: Tokio para Binance APAC, Londres/Frankfurt para OKX/Bybit.

**Recomendación**: instancias bare-metal o dedicadas. Evitar virtualización compartida ("noisy neighbor" effect).

## Monitoreo y alertas (§16.3)

Stack: **Prometheus + Grafana** (open source, máxima flexibilidad).

### Alertas críticas configuradas
- Pérdida de conexión WebSocket.
- RPC timeouts.
- Simulaciones fallidas consecutivas (>5).
- Tasa de éxito de bundles < 50%.
- Beneficio diario negativo.

### Dashboard en tiempo real
- Operaciones por minuto.
- Beneficio acumulado.
- Distribución por estrategia.
- Latencia promedio detección-ejecución.
- Utilización de gas.

## Dashboard de PnL (§16.4)

Métricas agregadas en múltiples timeframes (horario, diario, semanal, mensual):
- **PnL neto** (después de gas).
- **ROI**.
- **Tasa de éxito de bundles**.
- **Costo total de gas**.
- **Gráfico de equidad** (crecimiento compuesto del capital).

Desglose por:
- Estrategia (DEX Triangular, CEX-DEX, Liquidations, etc.).
- Cadena (ETH, ARB, BASE, BSC, etc.).
- Par de tokens.
- DEX usado.

## RPC Endpoints Fallback (§16.5)

ArbitrageX mantiene **mínimo 3 proveedores RPC por cadena** para redundancia.
**Failover automático**: si primario no responde en < 50ms → conmuta al siguiente.

| Cadena | RPC Primario | RPC Secundario | RPC Terciario |
|--------|--------------|------------------|------------------|
| **Ethereum** | Alchemy (dedicated) | QuickNode (dedicated) | Flashbots RPC |
| **Arbitrum** | Alchemy ARB | QuickNode ARB | Public ARB RPC |
| **Base** | Alchemy BASE | QuickNode BASE | Public BASE RPC |
| **BSC** | Ankr BSC | QuickNode BSC | Public BSC RPC |
| **Polygon** | Alchemy MATIC | QuickNode MATIC | Public MATIC RPC |

## Configuración Docker / Kubernetes

ArbitrageX usa Docker Compose para deployment simple, Kubernetes para alta disponibilidad. Orquestación:

```yaml
services:
  searcher-rs:
    image: arbitragex-searcher:latest
    deploy:
      replicas: 2
      restart_policy:
        condition: on-failure
    environment:
      - RPC_WS_1=${ALCHEMY_WSS}
      - RPC_WS_1_BACKUP=${QUICKNODE_WSS}
      - DATABASE_URL=postgres://arbx:${PG_PWD}@postgres:5432/arbitragex
    depends_on:
      postgres: { condition: service_healthy }
      redis: { condition: service_healthy }

  sim-ctl:
    image: arbitragex-sim-ctl:latest
    environment:
      - ARBX_USE_SIMULATOR_V2=true   # cuando Sprint 4 esté listo
```

## Stack de monitoreo

```
[Searcher metrics] → Prometheus → Grafana dashboard
                  ↘ Loki ← Promtail (logs)
                  ↘ Alertmanager → Telegram/Slack
```

## Checklist pre-deploy producción

- [ ] `--no-cache` flag en `docker compose build`.
- [ ] `--env-file .env` explícito (no asumir).
- [ ] Postgres `DATABASE_URL` configurado en TODOS los productores.
- [ ] Redis health check verde.
- [ ] 3 RPCs configurados por cadena.
- [ ] Flashbots Protect activo.
- [ ] Alertas Prometheus configuradas.
- [ ] Kill switch accesible vía API.
- [ ] `ARBX_PAPER_TRADE=true` por default. Live requiere doble confirmación.
- [ ] Monitoreo de profit diario activo (alerta si negativo).

## Invariantes
- Mínimo 3 RPCs por cadena (sin excepción).
- Failover < 50ms (sino timeout).
- VPS en región que minimiza latencia (no genérico AWS-west si Ethereum builders están en AWS-east).
- Bare-metal o dedicado SIEMPRE. Compartido NO.
- Backup VPS en región diferente (DR).
- Live mode requiere confirmación humana doble.

## Cross-references
- RPC failover code: arbx-rpc-failover-discipline skill.
- Postgres + Redis setup: capítulo 6 (R6 anti-reincidencia + DATABASE_URL doctrine).
- Monitoring metrics list: skill `mev-dashboard-observability` (índice MEV).
- Capital/mode config: Tab 1 del Strategy Panel.
