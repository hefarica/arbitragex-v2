# E4 — Inventario VPS 12 dominios (ventana 02:06:53–02:10:57 UTC)
> Agente: vps-inventory · Solo lectura · Salida completa en transcript (task a820fb734479e06b3) · Host arbx-v2-clean (195.201.235.70), root via ssh alias arbx

## Dominios (36 checks, todos observed salvo declaración)
1. **identidad_acceso**: Ubuntu 6.8.0-136 KVM; keys ed25519/rsa/ecdsa bajo IP en known_hosts; root uid=0, sudo passwordless; docker-first NO docker-only (nginx, cloudflared, cron en host).
2. **recursos**: 8 vCPU EPYC-Rome; RAM 15Gi (11Gi avail); **swap 0B**; load 0.44 (ocio); 0 OOM ring; DefaultLimitNOFILE 524288.
3. **almacenamiento**: /dev/sda1 150G: 103G usado / 42G libres (72%), inodos 12%; Docker: Images 23 (34.5GB, 2.3GB reclaimable), Containers 24 (458MB), **Build Cache 202 entradas / 24.68GB (20.96GB reclaimable)**, Volumes 15 (59.44GB). Top du: backend 333M, repo-vps-audits 166M, audits 48M (truncamiento tail-10 declarado).
4. **servicios**: **24/24 Up healthy, 0 exited, RestartCount=0, OOMKilled=0**; 21 iniciados 2026-09-15T22:57Z (deploy), thanos×3 desde 08-19. Compose activo **compose.prod.yml** (24 servicios). depends_on mapeado (searcher→redis+postgres; sim-ctl→pg+redis; api-server→redis+socket-proxy; edge→api-server; frontend→edge; …). Red única arbx-net.
5. **datos**: PG 15.18 db arbitragex **46GB, 83 tablas** (opportunities 19GB, scored_opportunities 10GB, route_discovery_outcomes 6.9GB, pool_reserves 5.1GB, risk_events 3.2GB, simulations 687MB…) + db arb_drill 5.7GB. Redis db0 keys=3324 (725 con TTL). AOF ok; rdb_changes_since_last_save 4.2M; PG max_wal_size 1024MB, checkpoint_timeout 300s, **archive_mode=off**; locks 2; replication slots 0.
6. **retencion_backups**: cron root: pg_retention.sh diario 04:17, builder prune semanal, **disk_guard.sh horario (FALLA: Permission denied ×140 — script 0644 no ejecutable)**. /etc/cron.d: arbx-firewall-monitor */5, arbx-watchdog * * * *, arbx-retention DISABLED 09-04. /opt/backups: pre-deploy-20260715 (backup_.sql **0 bytes**) + pre_xen_excl_20260915.sql.gz 4.0G (01:04Z). **Sin backup automatizado ni restore-test.**
7. **red_seguridad**: 26 listeners — públicos: 22, 80 (nginx v4+v6), 5173, 8787; resto loopback (18). iptables INPUT ACCEPT (ufw no instalado; nft anti-spoof bridge). cloudflared active 48d token-based; **token en claro en cmdline (redactado)**; ingress → localhost:5173; dominio arbx.ape-tv.net.
8. **despliegue**: HEAD 2cdbce35 (main, PR #571); working tree limpio salvo untracked archives/; imagen searcher sha256:043076d… coincide con imagen actual (consistente); **sin label git-rev en imágenes**; **/tmp/arbx-deploy.lock VIVO mtime 02:10Z** (actividad deploy/watchdog en curso al inventariar).
9. **configuracion**: .env 130 asignaciones, 119 únicas, **11 duplicadas** (ARBX_EDGE_TOKEN, ARBX_MIGRATOR_PASSWORD, ARBX_RO_PASSWORD, ARBX_RW_PASSWORD, ARBX_SERVICE_TOKEN, ARBX_SIMULATOR_V2_READY, ARBX_USE_SIMULATOR_V2, FLASHLOAN_EXECUTOR_1, GRAFANA_ADMIN_PASSWORD, MINIO_ROOT_PASSWORD, MINIO_ROOT_USER). Compose: compose.prod.yml activo + dev/hotpath-test + 3 overrides. **.env 0644 world-readable + 5 .env.bak también 644** (solo .env.crucible 0600).
10. **observabilidad**: prometheus/loki/grafana/alertmanager/promtail/thanos completos; retention prom default 15d (blocks 2h); LogConfig json-file 5×10m en 23/24 — **anvil-1 SIN límite**; reglas alerts.rules.yml 22KB; silencios NO consultados (gap). Health: 8787/health 200, 8080/api/health 200, 9090/-/healthy 200, 3100/ready 200.
11. **blockchain**: **17 chains con RPC_HTTP_* y 17 con RPC_WS_*** (1,10,100,56,137,8453,84532,42161,421614,43114,560048,59144,81457,534352,11155111,11155420,17000); pools: **SOLO chain 1: 237 activos + 961 inactivos**; tokens 4266. SIM_SIGNER_ADDRESS presente (len 42); EXECUTOR_1 len 42; **ARBITRAGE_EXECUTOR presente (len 42)**; FLASHLOAN_EXECUTOR_1 presente (duplicado); **FLASHBOTS_SIGNER_KEY AUSENTE**.
12. **dependencias**: grafo compose mapeado; conciliación 1:1 servicios↔contenedores (0 huérfanos de contenedor); volúmenes dangling: arbx_vault_data (declarado sin consumidor) + 3 arbx-frontend-devcontainer_* (proyecto ajeno) + 2 anónimos; imagen rust:1.91 sin contenedor. Truncamientos declarados: du top-10, df -v tabla contenedores (suplida), list-timers (solo distro), ps filtrado.

## HALLAZGOS (nada accionado — read-only)
- **H1** disk_guard roto (0644; 140 Permission denied) — guard DISK-GUARD-01 no operativo.
- **H2** token cloudflared en cmdline.
- **H3** .env 0644 + 5 baks 644 (JWT_SECRET/POSTGRES_PASSWORD/tokens dentro).
- **H4** anvil-1 sin rotación de logs.
- **H5** /tmp/arbx-deploy.lock vivo 02:10Z — posible ventana deploy en curso.
- **H6** sin backups automatizados ni restore-test.
- **H7** build cache 20.96GB reclaimable.
- **H8** INPUT ACCEPT (mitigado: loopback-binding + nft anti-spoof + Cloudflare).
