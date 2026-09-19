# RUNBOOK — ACTIVACIÓN MAINNET (terminus relays-client)
> Paquete de activación para el operador. Emitido 2026-09-17.
> PRECONDICIÓN INQUEBRANTABLE (§34.5): gates G1-G8 PASS con evidencia reproducible.
> Estado actual: **NO PASS** (ver GATES-G1-G8-ESTADO.md) — este runbook NO se ejecuta hoy.

## Fase 0 — Prerrequisitos (orden obligatorio)

| # | Acción | Verificación | Estado hoy |
|---|---|---|---|
| 0.1 | Cerrar cobertura V3 (`fix/v3-slot0-coverage-20260917` mergeado + deployado) | ≥1 `simulations.passed=true` en PG | ❌ sims_passed=0 |
| 0.2 | Primer ciclo real en Sepolia (TESTNET_LIVE_VERIFIED §4 skill) | tx hash + receipt chainId 11155111 | ❌ executions=0 |
| 0.3 | Deploy veraz hasta `dcfe890c`+ (hoy VPS=a06a968d) | `git rev-parse HEAD` == SHA deployado | ❌ 3 commits sin deploy |
| 0.4 | `FLASHBOTS_SIGNER_KEY` en VPS `.env` (la pone el operador, NUNCA por chat) | `grep -cE '^FLASHBOTS_SIGNER_KEY=.+' .env` = 1 | ❌ ausente |
| 0.5 | Semántica kill-switch verificada + drill trip/untrip | test del drill documentado | ⚠️ GAP semántica |
| 0.6 | Firma canary fondeada (gas para ≥10 tx) | balance on-chain > floor | ❌ pendiente |

## Fase 1 — Variables exactas (VPS `/opt/arbitragex-v2/.env`)

```bash
ARBX_LIVE_EXEC_ENABLED=true        # EXACTO en minúscula — "True"/"False"/"1" son no-ops (test exact_true_only)
ARBX_LIVE_EXEC_CHAINS=1            # mainnet. Mantener 11155111 para incluir Sepolia: "1,11155111"
ARBX_TRADE_MODE=live               # hoy=paper
# FLASHBOTS_SIGNER_KEY ya debe existir (Fase 0.4)
```

## Fase 2 — Aplicación (sin rebuild de imagen)

```bash
ssh arbx
cd /opt/arbitragex-v2
# editar .env (operador o sesión autorizada)
docker compose --env-file .env -f docker/compose.dev.yml up -d relays-client
docker logs relays-client --tail 50   # confirmar: live_exec enabled + allowlist [1]
```
El binario Rust lee env en runtime (`std::env::var`, live_exec_policy.rs:19-24) — restart basta.
⚠️ NOTA: si la var viaja a algún servicio Next/edge (`NEXT_PUBLIC_*`), esos SÍ requieren rebuild --no-cache (RULE 03). relays-client no.

## Fase 3 — Canary (límites §34.5)

- Capital en riesgo ≤ **$350** · principal TLS ≤ **5 WETH**.
- Duración sugerida: 1 ciclo completo detectar→validar→simular→ejecutar→reconocer.
- Abort inmediato si: kill-switch trip · gas > floor configurado · revert inesperado · divergencia sim-vs-receipt.

## Fase 4 — Parada / Rollback (en orden de rapidez)

1. **Kill-switch** (ms): `redis-cli SET arbx:killswitch '{"enabled":true,"reason":"canary-abort","triggered_by":"operator"}'` — check 1 del pre-execute (pre_execute_checklist.rs:260) lo bloquea antes de firmar.
2. **Deny de broadcast** (<10s): `ARBX_LIVE_EXEC_ENABLED=false` + `up -d relays-client`.
3. **Irreversibles**: tx ya minadas + gas quemado. El rollback no los recupera — por eso el canary es chico.

## Qué este runbook NO hace

- No firma, no broadcastea valor, no activa nada por sí mismo.
- No sustituye G1-G8. Sin Fase 0 completa, ejecutar Fase 1-2 = capital por pipeline sin ciclos verificados.
