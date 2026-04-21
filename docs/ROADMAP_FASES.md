# Roadmap por Sprints — ArbitrageX v2

8 sprints acumulativos. Cada uno pasa por spec → plan → ejecución → verificación con evidencia real. Cada sprint **no** autoriza pasar al siguiente hasta que sus criterios de aceptación están verificados.

## S1 — Foundations (este)

Infraestructura por capas, contratos canon, esquema DB completo, config tipada, secretos, logging estructurado, `/health` + `/metrics` en todos los servicios, kill-switch global, stack docker-compose operativo, automation scripts que fallan honestamente.

Credenciales externas: **ninguna**.

## S2 — Detection real (searcher-rs)

Cliente de chain real (`ethers-rs`), mempool WS, parseo calldata (4byte + ABI), patrones swap / arb triangular / liquidación, publisher Redis Streams (`XADD arbx:opps:detected`), persistencia `opportunities`.

Credenciales: **RPC WS real** (Alchemy/Infura/self-hosted).

## S3 — Selector + Scoring + Risk gates

selector-api con DB + Redis reales, scoring multi-factor calibrado, token-safety cache contra GoPlus/Honeypot.is, blacklist/whitelist dinámica, circuit breakers, policy engine.

Credenciales: **API keys GoPlus / Honeypot.is**.

## S4 — Simulation real (sim-ctl)

Integración Anvil/Hardhat fork, `eth_call` bundle sim, `debug_traceCall`, revert detection, gas accuracy, pass/fail determinista.

Credenciales: **RPC con `debug_traceCall`** o Anvil local.

## S5 — Execution privada (relays-client)

Flashbots SDK, bundle builder, nonce manager, retry/replacement, multi-relay routing (bloxroute, eden, beaver, titan), signer hardening, kill-switch observado antes de cada submit.

Credenciales: **claves privadas reales + endpoints de relays**.

## S6 — Recon + Learning loop

Tx trace real, PnL real, variance analysis, scoring adaptativo de strategies/relays (escribe `strategy_scores` / `relay_scores`), writer de `incident_log`, detección de anomalías.

Credenciales: ninguna nueva (reutiliza S5).

## S7 — Edge hardened + Frontend operativo

Edge Worker con rate-limit KV-backed, JWT productivo, Vault/1Password Connect para secretos. Frontend Next.js con dashboards completos: oportunidades, simulaciones, ejecuciones, relays, chains, token safety, risk center, incident timeline, configuración de thresholds.

Credenciales: **Cloudflare account**, **Vault endpoint**.

## S8 — Observabilidad + E2E + gobierno

Grafana dashboards poblados con métricas reales acumuladas, Alertmanager con Slack/PagerDuty, Loki queries, smoke/adversarial/E2E tests automatizados, backup/restore probado, runbooks, checklist "listo para paper-trading".

Credenciales: **webhook Slack / PagerDuty**.

## Criterios de paso-a-real (cumplidos sólo al cierre de S8)

- [ ] Sandbox: `smoke-test.sh` pasa 100% en CI
- [ ] Fork: `sim-ctl` valida al menos N oportunidades históricas con precisión > X%
- [ ] Paper trading: pipeline completo sin enviar bundles; logging simétrico al caso real
- [ ] Capital real limitado: cap por trade, cap diario, kill-switch con autoridad clara, alertas probadas

**Ninguno de estos pasos está autorizado hasta que el respectivo sprint lo respalde.**
