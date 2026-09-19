# ACTA DE CAPACIDAD LIVE — arbitrage-x v2
> Emitida 2026-09-17 por el orquestador (OMEGA) con evidencia verificada en vivo.
> Estado: **NO_EVALUADO → CAPACIDAD PARCIAL**. Este documento NO habilita trading.
> Template: `.claude/skills/arbx-live-engineering/templates/ACTA_CAPACIDAD_LIVE.template.md`

## Capacidad técnica por red

- **Chain soportadas por código**: cualquier chainId vía allowlist (`live_exec_policy.rs`). Default allowlist: Sepolia `11155111`. **Mainnet `1` soportada por código** (test `explicit_mainnet_is_supported`:71-76).
- **SHA software verificado**: local `dcfe890c` / VPS desplegado `a06a968d` (delta: 3 commits sin deploy).
- **RPC/WSS**: pool Alchemy (vía VPS, sin credenciales en este documento).
- **Contratos**: contrato defi deployado + verificado (quedan los 264 cartuchos = Rhai, off-chain).
- **Wallets/firmantes**: `SIM_SIGNER_ADDRESS` presente en VPS .env (1). `FLASHBOTS_SIGNER_KEY` **AUSENTE** (grep=0) — requerido para el terminus de broadcast.
- **Kill-switch**: Redis `arbx:killswitch` (JSON), check 1 de `pre_execute_checklist.rs:260`, wired en relays-client main.rs:121.
  - **Estado actual leído (2026-09-17)**: `{"enabled":false,"reason":"VER","triggered_by":"admin","updated_at":"2026-09-16T21:41:50.613Z"}`
  - ⚠️ SEMÁNTICA SIN VERIFICAR: el campo `enabled` con `reason:"VER"` disparado por admin ayer requiere confirmación de si `enabled:false` = trading permitido o kill inactivo (documentado como GAP — verificar contra killswitch.rs antes de confiar).

## Testnet

- **Red**: Sepolia 11155111 (default allowlist).
- **Pruebas reales ejecutadas**: **NINGUNA con recebo on-chain**. `executions=0` y `sims_passed=0` EN TODA LA HISTORIA (query PG 2026-09-17: `SELECT COUNT(*) FROM simulations WHERE passed` → 0; `COUNT(*) FROM executions` → 0).
- **Pruebas no realizadas y motivo**: el embudo muere antes de sim (cuello cobertura V3 `v3_quote_unavailable`, branch `fix/v3-slot0-coverage-20260917` en curso). Sin candidato evaluable no existe sim passed ni tx.

## Mainnet

- **Lecturas verificadas**: catálogo de routers/pools mainnet activo (PANCAKE-ROUTER-01, commit dcfe890c).
- **Simulaciones/forks sin envío**: sim-ctl con fork anvil operativo (SIM-FUND-01: probe signer fondeado en fork, commit 125b1e0b).
- **Controles de activación disponibles**: ver `RUNBOOK-ACTIVACION-MAINNET.md` (2 env vars + prerequisitos).
- **Requisitos y defectos pendientes** (bloqueantes, en orden):
  1. `sims_passed = 0` — G2/G3 NO PASS. Cerrar cobertura V3 (slot0 backfill + fee-tiers) es EL prerrequisito.
  2. `FLASHBOTS_SIGNER_KEY` ausente en VPS .env.
  3. `ARBX_LIVE_EXEC_ENABLED=False` (F mayúscula) — el código exige exactamente `"true"`; valor actual es un no-op intencional o accidental (test `exact_true_only`:87-89).
  4. Semántica del kill-switch sin verificar (GAP arriba).
  5. 3 commits sin deploy (VPS corre `a06a968d`, main local lleva `dcfe890c`).
- **Configuración requerida por el código actual** (NO asumir que dos variables bastan):
  `ARBX_LIVE_EXEC_ENABLED=true` (exacto, minúscula) · `ARBX_LIVE_EXEC_CHAINS=1` · `FLASHBOTS_SIGNER_KEY=<operador>` · `ARBX_TRADE_MODE=live` (hoy `paper`) · kill-switch verificado operacional (trip + untrip probados) · firma fondeada con gas.
- **Procedimiento técnico sin activación financiera implícita**: env → `docker compose --env-file .env -f docker/compose.dev.yml up -d relays-client` (binario Rust lee env en runtime; NO requiere rebuild de imagen).
- **Rollback**: `ARBX_LIVE_EXEC_ENABLED=false` + `up -d relays-client` (<10s) + trip kill-switch Redis. **Irreversibles on-chain**: toda tx broadcast + gas; por eso canary limitado.

## Decisión financiera separada del operador

- **Operador**: hefarica (Héctor), dueño único.
- **Autorización registrada**: §34.5 CLAUDE.md (2026-09-15) — flip autorizado SIN nueva ceremonia cuando G1-G8 pasen con evidencia reproducible.
- **Límites canary autorizados**: capital en riesgo ≤ **$350**, principal TLS **5 WETH**.
- **Firma/activación con fondos reales**: A CARGO DEL OPERADOR. Este acta no la delega.
- **Estado observado posteriormente**: (vacío — solo con evidencia).

**MAINNET_RELEASE_READY ≠ MAINNET_ACTIVE. No garantiza beneficios.**
