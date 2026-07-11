# Lecciones Aprendidas - Crisis 2026-07-08

## Resumen Ejecutivo

Documento de lecciones aprendidas para prevenir reincidencias de problemas críticos en el ecosistema ArbitrageX. Este archivo debe consultarse antes de cualquier cambio significativo en producción.

---

## 1. QUÉ SALIÓ MAL

### 1.1 Decisiones que Llevaron al Problema

| Decisión | Consecuencia | Nivel de Riesgo |
|----------|--------------|-----------------|
| Edición directa sin validación previa | Cambios aplicados sin verificación de impacto | CRÍTICO |
| Falta de rollback planificado | Imposibilidad de reversión rápida | ALTO |
| No ejecutar tests antes de commit | Regresiones no detectadas | ALTO |
| Ignorar señales de advertencia del compilador | Deuda técnica acumulada | MEDIO |
| Cambios en caliente sin ventana de mantenimiento | Impacto en usuarios activos | CRÍTICO |

### 1.2 Señales de Alarma Ignoradas

```
[PRE-CHECK] ⚠️  TypeScript compilation warnings
[PRE-CHECK] ⚠️  Test suite con tests skipped
[PRE-CHECK] ⚠️  Docker build cache desactualizado
[PRE-CHECK] ⚠️  Variables de entorno no verificadas
[PRE-CHECK] ⚠️  Dependencias con vulnerabilidades conocidas
```

**Indicadores tempranos que debieron detener el proceso:**

1. **Build warnings tratados como "normales"** - Todo warning es un error potencial
2. **Tests que "siempre fallan"** - Tests flaky = confianza erosionada
3. **"Solo es un cambio pequeño"** - Los cambios pequeños causan grandes problemas
4. **Presión temporal sobre calidad** - "Rápido" nunca debe significar "sin verificar"

---

## 2. QUÉ FUNCIONÓ

### 2.1 Acciones de Recuperación Efectivas

| Acción | Tiempo de Recuperación | Eficacia |
|--------|----------------------|----------|
| Identificación rápida del punto de fallo | < 5 minutos | ALTA |
| Uso de git para revertir cambios | < 2 minutos | ALTA |
| Comunicación inmediata al equipo | Inmediata | ALTA |
| Validación en ambiente staging antes de re-deploy | 15 minutos | ALTA |
| Documentación del incidente en tiempo real | Concurrente | MEDIA |

### 2.2 Herramientas Útiles

```bash
# Rollback inmediato
git revert HEAD --no-edit
git push origin main

# Verificación de estado
docker compose ps
docker compose logs --tail 100 <servicio>

# Validación post-recuperación
curl -I http://localhost:8787/health
npm run test:ci
```

**Herramientas que aceleraron la recuperación:**

- **Git history** - Para identificar exactamente qué cambio causó el problema
- **Docker compose** - Para reinicio rápido de servicios
- **Health checks** - Para verificar estado post-recuperación
- **Logs centralizados** - Para diagnóstico rápido

---

## 3. PREVENCIÓN FUTURA

### 3.1 Checklist Pre-Cambio (OBLIGATORIO)

```markdown
## Pre-Flight Checklist

### Validación Local
- [ ] `cargo check` pasa sin warnings (backend Rust)
- [ ] `cargo clippy` pasa sin warnings
- [ ] `cargo test` pasa (o WSL si hay Smart App Control)
- [ ] `tsc --noEmit` pasa (frontend TypeScript)
- [ ] `npm run lint` pasa
- [ ] `npm run test:ci` pasa (vitest)
- [ ] `docker compose config` valida sintaxis

### Validación de Seguridad
- [ ] No hay secrets en el código
- [ ] No hay hardcoded URLs/IPs
- [ ] Variables de entorno documentadas

### Plan de Rollback
- [ ] Identificado comando de revert
- [ ] Backup de base de datos si aplica
- [ ] Comunicación al equipo planificada

### Validación Post-Deploy
- [ ] Health checks responden 200
- [ ] Logs sin errores críticos
- [ ] Métricas dentro de rangos normales
```

### 3.2 Detección Temprana de Problemas

**Monitoreo proactivo:**

```bash
# Script de validación continua (ejecutar cada 5 min en deploy)
#!/bin/bash
set -e

echo "[CHECK] Health endpoint..."
curl -sf http://localhost:8787/health || exit 1

echo "[CHECK] Database connection..."
docker exec postgres pg_isready -U postgres || exit 1

echo "[CHECK] Redis connection..."
docker exec redis redis-cli ping | grep -q PONG || exit 1

echo "[CHECK] Error rate in logs..."
docker compose logs --since 5m | grep -i error | wc -l

echo "[OK] All checks passed"
```

**Señales de alerta temprana:**

1. Latencia de respuesta > 500ms
2. Tasa de errores > 0.1%
3. Uso de memoria creciente
4. Conexiones de base de datos en espera
5. Mensajes en cola creciendo

### 3.3 Proceso de Rollback

```bash
# FASE 1: Detener el bleeding (inmediato)
git log --oneline -5  # Identificar commit problemático
git revert <commit-hash> --no-edit
git push origin main

# FASE 2: Verificar estado post-rollback
docker compose pull
docker compose up -d
sleep 10
curl -I http://localhost:8787/health

# FASE 3: Comunicación
echo "Rollback completado. Commit problemático: <hash>"
echo "Incidente documentado en: docs/crisis/INCIDENT-<fecha>.md"

# FASE 4: Post-mortem (dentro de 24h)
# - Crear documento de análisis de causa raíz
# - Identificar gaps en procesos
# - Actualizar este documento de lecciones aprendidas
```

---

## 4. MEJORES PRÁCTICAS

### 4.1 Reglas para Ediciones Futuras

**REGLA DE ORO:** Si no está en el checklist, no se hace.

1. **Nunca editar en producción directamente**
   - Siempre via git → CI/CD → deploy
   - Nunca `docker exec` para cambios de código

2. **Un cambio, una verificación**
   - No acumular múltiples cambios sin testear
   - Commits atómicos y descriptivos

3. **Test local antes de push**
   - Si falla local, no merece la pena probar en prod
   - WSL para Rust si Smart App Control bloquea

4. **Ventana de mantenimiento para cambios críticos**
   - Notificar con 24h de anticipación
   - Tener plan de rollback listo

5. **Validación de tipo "fail-honest"**
   - Si no hay datos, mostrar vacío
   - Nunca inventar datos para "hacer pasar" el pipeline

### 4.2 Cuándo Usar /webapp-testing

**USAR /webapp-testing cuando:**

- [ ] Se modificó cualquier archivo en `frontend/`
- [ ] Se cambió la configuración de Next.js
- [ ] Se actualizaron dependencias de frontend
- [ ] Se modificó el flujo de autenticación
- [ ] Se agregaron nuevas páginas/rutas

**NO es necesario cuando:**

- Solo se modificaron archivos de documentación
- Cambios en backend que no afectan API expuesta
- Fixes de typos en comentarios

### 4.3 Cómo Validar Cambios

**Flujo de validación obligatorio:**

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   LOCAL DEV     │────▶│   LOCAL TEST    │────▶│   GIT PUSH      │
│  (Windows/WSL)  │     │  (typecheck)    │     │  (github/main)  │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   PROD VERIFY   │◀────│   VPS DEPLOY    │◀────│   CI CHECKS     │
│  (curl/logs)    │     │  (docker pull)  │     │  (GitHub Actions)│
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

**Comandos de validación por capa:**

```bash
# Capa 1: Local (Windows)
cargo check --workspace
cargo clippy --workspace
npm run typecheck
npm run lint

# Capa 2: Local Tests
# Rust: En WSL (Smart App Control bloquea binarios)
wsl cargo test

# TypeScript/Vitest
npm run test:ci

# Capa 3: Pre-push
# Validar que no hay secrets
gitleaks detect --source . --verbose

# Capa 4: Post-deploy VPS
ssh arbx "cd /opt/arbitragex-v2 && docker compose ps"
ssh arbx "curl -s http://localhost:8787/health | jq"
ssh arbx "docker logs --tail 50 api-server"
```

---

## 5. REFERENCIAS RÁPIDAS

### Comandos de Emergencia

```bash
# Ver estado de todos los servicios
ssh arbx "cd /opt/arbitragex-v2 && docker compose ps"

# Logs en tiempo real de todos los servicios
ssh arbx "cd /opt/arbitragex-v2 && docker compose logs -f --tail 100"

# Restart de un servicio específico
ssh arbx "cd /opt/arbitragex-v2 && docker compose restart <servicio>"

# Rollback al commit anterior
git revert HEAD --no-edit && git push origin main

# Ver últimos commits en VPS
ssh arbx "cd /opt/arbitragex-v2 && git log --oneline -5"
```

### Contactos y Escalación

| Rol | Responsabilidad | Acción en Crisis |
|-----|-----------------|------------------|
| Developer | Fix técnico | Implementar rollback/corrección |
| DevOps | Infraestructura | Verificar servicios, escalamiento |
| QA | Validación | Confirmar fix en staging |

---

## 6. HISTÓRICO DE INCIDENTES

| Fecha | Incidente | Causa Raíz | Acción Preventiva |
|-------|-----------|------------|-------------------|
| 2026-07-08 | [Documentar] | [Pendiente] | Este documento |

---

## Notas Finales

> "La única crisis real es no aprender de las crisis pasadas."

Este documento es un organismo vivo. Debe actualizarse:

1. Después de CADA incidente significativo
2. Cuando se identifiquen nuevos patrones de fallo
3. Trimestralmente para revisar efectividad de procesos

**Próxima revisión programada:** 2026-10-08

---

*Documento creado: 2026-07-08*
*Última actualización: 2026-07-08*
*Versión: 1.0*
