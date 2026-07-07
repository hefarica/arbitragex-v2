# Auditoría ArbitrageX - 51 Páginas

## Fecha: 2026-07-07
## Estado VPS: ✅ Backend operativo (20 servicios healthy)
## Problema identificado: Frontend local no conecta a backend VPS

---

## Resumen Ejecutivo

| Métrica | Valor |
|---------|-------|
| Páginas detectadas | 51 |
| Backend VPS | ✅ Operativo |
| Frontend local | ⚠️ Sin conexión a backend |
| Errores críticos | 1 (conectividad) |

---

## Estado del Backend (VPS)

### Servicios Healthy (20/20)
- ✅ postgres: Up 7 days
- ✅ redis: Up 7 days  
- ✅ api-server: Up 2 days, respondiendo 200
- ✅ edge: Up 2 days, respondiendo 200
- ✅ frontend: Up 2 days (puerto 5173)
- ✅ searcher-rs: Up 2 days
- ✅ token-enricher: Up 2 days
- ✅ sim-ctl: Up 2 days
- ✅ relays-client: Up 2 days
- ✅ recon: Up 2 days
- ✅ selector-api: Up 2 days
- ✅ anvil: Up 2 days
- ✅ + 8 servicios de observabilidad

### Logs API-Server
- ✅ Respondiendo correctamente a `/status` (200)
- ✅ Respondiendo a `/api/config/current` (200)
- ✅ Respondiendo a `/api/recon/summary` (200)
- ⚠️ Paper archiver: skip_no_sim_profit (comportamiento esperado)
- ⚠️ Warnings: omega_strategy_pack desconocido

### Logs Edge
- ✅ Todos los requests 200
- ✅ Health checks pasando
- ✅ Métricas disponibles

---

## Problema de Conectividad

**Síntoma:** Frontend local (localhost:3000) muestra:
```
PG: edge_error
Redis: edge_error
edge timeout after 5000ms
```

**Causa:** El frontend local intenta conectar a `NEXT_PUBLIC_EDGE_URL` que probablemente apunta a localhost:8787, pero el edge real está en el VPS.

**Solución:** Configurar el frontend local para apuntar al VPS o usar el frontend del VPS directamente.

---

## Páginas Auditadas

### Core Trading (11)
| Página | Estado | Notas |
|--------|--------|-------|
| `/` | ⚠️ Renderiza | Sin datos de backend |
| `/opportunities` | - | Pendiente |
| `/opportunities/by-strategy` | - | Pendiente |
| `/executions` | - | Pendiente |
| `/paper/history` | - | Pendiente |
| `/pools` | - | Pendiente |
| `/routes/discovery` | - | Pendiente |
| `/route-outcomes` | - | Pendiente |
| `/strategies` | - | Pendiente |
| `/strategies/forge` | - | Pendiente |

### Observability (6)
| Página | Estado | Notas |
|--------|--------|-------|
| `/status` | ⚠️ Con errores | `/api/status/summary` no responde |
| `/worker-health` | - | Pendiente |
| `/live-readiness` | - | Pendiente |
| `/audit-logs` | - | Pendiente |
| `/recon` | - | Pendiente |
| `/operations` | - | Pendiente |

### Risk & Control (8)
| Página | Estado | Notas |
|--------|--------|-------|
| `/risk` | - | Pendiente |
| `/killswitch` | - | Pendiente |
| `/operator` | - | Pendiente |
| `/operator/self-test` | - | Pendiente |
| `/operator/presets` | - | Pendiente |
| `/audit-logs` | - | Pendiente |
| `/apex/allocator` | - | Pendiente |

### Configuration (16)
| Página | Estado | Notas |
|--------|--------|-------|
| `/settings` | - | Pendiente |
| `/settings/credentials` | - | Pendiente |
| `/config` | - | Pendiente |
| `/config/trading` | - | Pendiente |
| `/chains` | - | Pendiente |
| `/rpcs` | - | Pendiente |
| `/pools` | - | Pendiente |
| `/dex-registry` | - | Pendiente |
| `/wallets` | - | Pendiente |
| `/wallet` | - | Pendiente |
| `/deploy-pipeline` | - | Pendiente |
| `/admin/topology` | - | Pendiente |
| `/admin/chains` | - | Pendiente |
| `/admin/signin` | - | Pendiente |
| `/onboarding` | - | Pendiente |
| `/onboarding/*` (5) | - | Pendiente |

### Omega S5 (8)
| Página | Estado | Notas |
|--------|--------|-------|
| `/omega-s5/core` | - | Pendiente |
| `/omega-s5/crucible` | - | Pendiente |
| `/omega-s5/factory` | - | Pendiente |
| `/omega-s5/adapters` | - | Pendiente |
| `/omega-s5/drift` | - | Pendiente |
| `/omega-s5/operator` | - | Pendiente |
| `/omega-s5/registry` | - | Pendiente |
| `/omega-s5/wallets` | - | Pendiente |

---

## Recomendaciones

### Inmediatas
1. **Configurar conexión frontend-local a VPS** o usar frontend del VPS (puerto 5173)
2. **Verificar variables de entorno** en frontend local (`NEXT_PUBLIC_EDGE_URL`)
3. **Continuar auditoría** una vez resuelta la conectividad

### A Corto Plazo
1. Documentar todas las 51 páginas con screenshots
2. Validar funcionalidad end-to-end
3. Ejecutar transacciones de prueba

---

## Próximos Pasos

1. ✅ Tarea 1: Backend observado (VPS operativo)
2. 🔄 Tarea 2: Frontend audit en progreso (bloqueado por conectividad)
3. ⏳ Tarea 3: Modo manual (pendiente de resolución)

