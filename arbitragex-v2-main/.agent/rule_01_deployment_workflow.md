# RULE: DEPLOYMENT WORKFLOW — LOCAL DEV → VPS PRODUCTION

## DEFINICIÓN

El proyecto ArbitrageX v2 sigue un flujo de despliegue estricto de dos entornos:

### 1. LOCAL (Windows Desktop)
- **Propósito:** Desarrollo, edición de código, testing unitario, validación de lógica.
- **Ubicación:** `C:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\`
- **Lo que se hace aquí:**
  - Escribir y modificar código (TypeScript, Rust, SQL, configs).
  - Ejecutar tests unitarios (`vitest`, `cargo test`).
  - Type-checking (`tsc --noEmit`).
  - Revisión de arquitectura y auditorías de código.
- **Lo que NO se hace aquí:**
  - NO se levanta Docker Desktop.
  - NO se ejecutan servicios de producción (Postgres, Redis, API Server).
  - NO se accede a la UI funcional con datos reales desde local.

### 2. VPS (Producción — Hetzner CX43)
- **Propósito:** Ejecución del stack completo via Docker, acceso web al dashboard.
- **Servidor:** Hetzner CX43
  - **IP:** `178.104.222.133`
  - **IPv6:** `2a01:4f8:c014:7b6::/64`
  - **OS:** Ubuntu
  - **Specs:** 8 vCPU, 16 GB RAM, 160 GB SSD
  - **Ubicación:** Falkenstein, Germany (eu-central)
- **Lo que corre aquí:**
  - `docker compose -f docker/compose.prod.yml up -d` (stack completo).
  - PostgreSQL, Redis, API Server, Edge Worker, Frontend (Next.js).
  - Monitoreo: Prometheus, Grafana, Loki.
  - Acceso al dashboard: `http://178.104.222.133:<PORT>` o dominio configurado.

## FLUJO DE TRABAJO

```
[LOCAL: Desarrollo]
     │
     ├── 1. Editar código
     ├── 2. Ejecutar tests (vitest, tsc)
     ├── 3. Validar lógica y arquitectura
     │
     ▼
[GIT: Commit & Push]
     │
     ├── git add .
     ├── git commit -m "descripción"
     ├── git push origin main
     │
     ▼
[VPS: Deploy]
     │
     ├── ssh root@178.104.222.133
     ├── cd /opt/arbitragex-v2
     ├── git pull origin main
     ├── docker compose -f docker/compose.prod.yml up -d --build
     │
     ▼
[WEB: Acceso y Verificación]
     │
     └── Abrir http://178.104.222.133:<PORT> en el navegador
```

## REGLAS INMUTABLES

1. **NUNCA instalar Docker Desktop en la máquina local Windows.** Docker solo corre en el VPS.
2. **NUNCA levantar servicios de backend (Postgres, Redis, API Server) en local.** El backend solo vive en el VPS.
3. **El frontend en local (`npm run dev`) es SOLO para desarrollo de UI.** Los datos reales solo aparecen cuando se conecta al backend del VPS.
4. **Todo cambio debe ser validado localmente (tests + typecheck) ANTES de hacer push al VPS.**
5. **El acceso funcional al sistema (con datos reales) siempre es via web apuntando al VPS.**
