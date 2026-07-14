---
name: auto-compact
trigger: context >= 90%
version: 2.0.0
status: active
---

# Auto-Compact Proactivo - Context Management System

Sistema de gestion de contexto con monitoreo proactivo y alertas graduales.

## Umbrales de Alerta

| Umbral | Estado | Accion |
|--------|--------|--------|
| 70% | HEALTHY_HIGH | Log silencioso |
| 80% | WARNING | Sugerir /compact |
| 90% | CRITICAL | Ejecutar /compact auto |
| 95% | EMERGENCY | /compact forzado |

## Implementacion

### 1. Pre-Compact Preservation
- Guardar todos
- Archivos modificados (git status)
- Branch activo
- Variables de entorno

### 2. Post-Compact Recovery
- Restaurar todos
- Generar resumen de sesion
- Notificar operador

## Integracion OMEGA

- Nunca compactar durante migraciones/deploys
- Siempre preservar estado de gates
- Verificar post-compact

## Uso

```bash
# Estado
claude auto-compact status

# Forzar
claude auto-compact now
```
