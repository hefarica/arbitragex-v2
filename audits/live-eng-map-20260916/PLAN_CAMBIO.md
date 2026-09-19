# PLAN_CAMBIO — post-mapa 2026-09-16 (VPS_MAPPED → requiere autorización por acción)
> Skill arbx-live-engineering §8. NADA de esto se ha ejecutado. Cada acción es reversible y acotada.
> Congelado por orden del operador ("no modifiques nada", 2026-09-15 ~02:00 UTC) hasta su GO por ítem.

## Contexto médico del sistema (síntesis de E1+E2+E3+E4)

1. **El stall de detección (01:16:25 UTC) NO es una falla**: es fail-honest por diseño. El reload del working set de pool_sync (~cada 12 min) aplicó `is_active=false` de los 5 pools XEN; el índice V2 se reescribió y las reserves murieron a 30s; el gate del cartridge (runner.rs:427-439) silenció el único flujo que existía (XEN = 99.86% de TODO). Infra sana (H1 y H3 del stall descartadas).
2. **El problema real es anterior y mayor**: el embudo de detección REAL produce ~0 candidatos/hora desde antes del cambio (flujo independiente = 31 filas/46 min, ya en 0 desde 00:38; route_discovery daba routes=0 toda la ventana; dirty_marked:0 en todos los ticks = el universo activo de 237 pools no cambia reserves on-chain — señales de un pool set mayormente dormido).
3. Deuda operativa acumulada del VPS: H1-H8 del inventario (disk_guard roto, .env 0644, token cloudflared en cmdline, anvil sin log-rotation, sin backups automáticos, build cache 21GB, deploy-lock activo a las 02:10Z).

## Acciones propuestas (ordenadas por impacto/riesgo; CERO ejecutadas)

### A1 — Deploy del PR #574 (consumidor canónico #567) — RIESGO BAJO
- **Causa**: Issue #567 (712/1000 sims not_implemented, 0 passed). CI 30/30 verde. Código verificado (check + 47/47 tests en Docker VPS).
- **Prerequisito duro**: verificar que `/tmp/arbx-deploy.lock` NO esté vivo (H5) y coordinar con watchdog.
- **Acción**: merge PR #574 → VPS `git pull` → `docker compose --env-file .env -f docker/compose.prod.yml build --no-cache sim-ctl` → `up -d sim-ctl` → L4 (SHA anclado + logs `sim.backend_selected`).
- **Env**: activar path B2c en sim-ctl (`SIM_BACKEND=revm`, `REVM_RPC_URL`); `ARBITRAGE_EXECUTOR` y `FLASHLOAN_EXECUTOR_1` ya presentes (len 42). REDIS_URL ya existe en el stack. **Sin REVM_RPC_URL válido el path queda inerte (fail-honest) — definir valor es decisión de operador (qué proveedor/fork usar).**
- **Rollback**: redeploy del SHA anterior (2cdbce35); el código es aditivo (carrier ausente → path previo).
- **Verificación**: tallies de simulations — la clase `strategy_cyclic_route_not_simulatable_in_s4` debe desaparecer cuando el carrier está presente; smoke: `docker logs sim-ctl | grep canonical_plan_hit`.

### A2 — Higiene operativa crítica (H1/H2/H3/H4) — RIESGO MÍNIMO, 4 comandos
- `chmod +x /opt/arbitragex-v2/scripts/disk_guard.sh` (restaura el guard 30/15GB del PR #557; evidencia: 140 Permission denied).
- `chmod 600 /opt/arbitragex-v2/.env /opt/arbitragex-v2/.env.bak*` (secrets world-readable).
- Mover token de cloudflared de cmdline a EnvironmentFile (requiere editar unidad systemd + restart de cloudflared ~2s — ventana breve del túnel).
- LogConfig json-file 5×10m para anvil-1 (via override compose + up -d anvil).
- **Rollback**: chmod inversos; unidad systemd original.
- **Verificación**: ejecución manual de disk_guard (exit 0); `ls -l .env` = 600; `systemctl show cloudflared -p ExecStart` sin token.

### A3 — Backups automatizados + restore-test (H6) — RIESGO BAJO
- Cron diario `pg_dump | gzip` a /opt/backups con retención 7 + restore-test mensual en contenedor efímero (no en prod).
- **Prerequisito**: espacio (usa el reclaimable de A4 como colchón).
- **Rollback**: quitar la entrada cron.

### A4 — Recuperación de disco (H7) — RIESGO BAJO
- `docker builder prune` controlado (~21GB reclaimable; ya hay cron semanal — ejecutar manual una vez + verificar guard A2 vivo).
- **Rollback**: no aplica (cache regenerable).

### A5 — Reactivar detección REAL (el corazón del objetivo de plata) — TRABAJO DE INGENIERÍA, no un comando
Tres frentes (en orden):
1. **Universo de pools**: auditar por qué 237 pools activos están dormidos (dirty_marked:0). Habilitar `ARBX_POOL_ENUM_MODE=shadow` (worker de enumeración, default OFF) para re-descubrir pools con volumen real; promover a activos los top por volumen/TVL on-chain.
2. **route_discovery → emisor**: hoy es radar shadow (nunca escribe opps por diseño). Con universo vivo, evaluar su promoción (routes>0 → gates propios) según su contrato de shadow→active.
3. **#567 desplegado (A1)** para que lo detectado pueda pasar simulación.
- **Cada frente**: PR propio con ID de anomalía (§37), contract tests, y validación con tallies reales post-deploy.

## Qué NO se hará sin autorización explícita adicional
- Cualquier flip live/mainnet (§34.5: gates G1-G8 con evidencia verificada primero).
- Reactivar los pools XEN (se mantienen excluidos; work order actualizada con resultado real).
- Toques en NGINX/cloudflared más allá de A2, o en otras chains del catálogo.

## Presupuesto e interrupción
- A1: ~15 min de build + 2 min deploy (por-servicio); aborta si CI del merge falla o lock H5 vivo.
- A2-A4: <5 min total; aborta si cualquier comando falla (cada uno independiente).
- A5: sesiones de ingeniería sucesivas, presupuesto por PR.
