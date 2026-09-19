# 01-GROUND-TRUTH — snapshot orquestador (2026-09-19T05:42Z)

## Local (estación)
- HEAD: cdb4c8908f038e839ef79f54224b8fbc1746a755 (branch feat/s1-fee-dual-unit-20260918)
- cdb4c890 fix(anchor): S1 — fee dual-unit divisor por tipo de pool (LANDED — el defecto
  "fee V3 pips /10_000" está CERRADO en disco; fee_fraction() en
  backend/searcher-rs/src/quote_anchor_runtime.rs:129-136)
- Working tree: .agents/skills/* modificados (masivamente), resto limpio.

## VPS (ssh arbx, lectura)
- Flota COMPLETA healthy: searcher-rs (44m), postgres (8h), api-server (2h), selector-api (2h),
  frontend/edge/promtail/sim-ctl/recon/token-enricher/relays-client/grafana/vault/prometheus/
  redis/alertmanager/socket-proxy/loki/minio/anvil (20h).
- Redis: XLEN arbx:opps:detected = 10003 (stream lleno, flujo activo).
- PostgreSQL: MAX(detected_at) = 2026-09-19 05:42:24 UTC vs sondeo 05:42:29 UTC
  → **oportunidades fluyendo con 5s de frescura**. El origen de datos NO está caído.

## Hermes
- Gateway local 127.0.0.1:8642 v0.21.3 corriendo (restart a las ~00:39 tras crash de update).
- Run durable del ciclo: run_735380fc427447378ce00f5eb333d5b6 (running).
- Contrato API tatuado en memoria global: hermes-local-api-contract.md.

## Implicación para el gang
Si en el dominio vivo no se ve la economía por hop, el origen (searcher→redis→PG) está
descartado como causa: el problema vive ENTRE PG/redis y el browser (api-server proyección,
edge reshape, WS, frontend render) — foco de WO-02.
