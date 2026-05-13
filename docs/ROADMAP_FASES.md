# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# Roadmap por Sprints â€” ArbitrageX v2

8 sprints acumulativos. Cada uno pasa por spec â†’ plan â†’ ejecuciÃ³n â†’ verificaciÃ³n con evidencia real. Cada sprint **no** autoriza pasar al siguiente hasta que sus criterios de aceptaciÃ³n estÃ¡n verificados.

## S1 â€” Foundations (este)

Infraestructura por capas, contratos canon, esquema DB completo, config tipada, secretos, logging estructurado, `/health` + `/metrics` en todos los servicios, kill-switch global, stack docker-compose operativo, automation scripts que fallan honestamente.

Credenciales externas: **ninguna**.

## S2 â€” Detection real (searcher-rs)

Cliente de chain real (`ethers-rs`), mempool WS, parseo calldata (4byte + ABI), patrones swap / arb triangular / liquidaciÃ³n, publisher Redis Streams (`XADD arbx:opps:detected`), persistencia `opportunities`.

Credenciales: **RPC WS real** (Alchemy/Infura/self-hosted).

## S3 â€” Selector + Scoring + Risk gates

selector-api con DB + Redis reales, scoring multi-factor calibrado, token-safety cache contra GoPlus/Honeypot.is, blacklist/whitelist dinÃ¡mica, circuit breakers, policy engine.

Credenciales: **API keys GoPlus / Honeypot.is**.

## S4 â€” Simulation real (sim-ctl)

IntegraciÃ³n Anvil/Hardhat fork, `eth_call` bundle sim, `debug_traceCall`, revert detection, gas accuracy, pass/fail determinista.

Credenciales: **RPC con `debug_traceCall`** o Anvil local.

## S5 â€” Execution privada (relays-client)

Flashbots SDK, bundle builder, nonce manager, retry/replacement, multi-relay routing (bloxroute, eden, beaver, titan), signer hardening, kill-switch observado antes de cada submit.

Credenciales: **claves privadas reales + endpoints de relays**.

## S6 â€” Recon + Learning loop

Tx trace real, PnL real, variance analysis, scoring adaptativo de strategies/relays (escribe `strategy_scores` / `relay_scores`), writer de `incident_log`, detecciÃ³n de anomalÃ­as.

Credenciales: ninguna nueva (reutiliza S5).

## S7 â€” Edge hardened + Frontend operativo

Edge Worker con rate-limit KV-backed, JWT productivo, Vault/1Password Connect para secretos. Frontend Next.js con dashboards completos: oportunidades, simulaciones, ejecuciones, relays, chains, token safety, risk center, incident timeline, configuraciÃ³n de thresholds.

Credenciales: **Cloudflare account**, **Vault endpoint**.

## S8 â€” Observabilidad + E2E + gobierno

Grafana dashboards poblados con mÃ©tricas reales acumuladas, Alertmanager con Slack/PagerDuty, Loki queries, smoke/adversarial/E2E tests automatizados, backup/restore probado, runbooks, checklist "listo para paper-trading".

Credenciales: **webhook Slack / PagerDuty**.

## Criterios de paso-a-real (cumplidos sÃ³lo al cierre de S8)

- [ ] Sandbox: `smoke-test.sh` pasa 100% en CI
- [ ] Fork: `sim-ctl` valida al menos N oportunidades histÃ³ricas con precisiÃ³n > X%
- [ ] Paper trading: pipeline completo sin enviar bundles; logging simÃ©trico al caso real
- [ ] Capital real limitado: cap por trade, cap diario, kill-switch con autoridad clara, alertas probadas

**Ninguno de estos pasos estÃ¡ autorizado hasta que el respectivo sprint lo respalde.**

