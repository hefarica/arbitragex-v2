# ArbitrageX v2 — Operator Runbook

> **Audiencia**: el operador del sistema. Guía concisa para usar las features
> shipped en la sesión 2026-05-04/06 (Incidentes #7 y #8 de
> `.agents/memory/anti_reincidencia.md`).
> **Última actualización**: 2026-05-06

---

## 1. URLs en producción

| URL | Función |
|-----|---------|
| `http://195.201.235.70:5173/operations` | PMI/EVM KPIs + **Pipeline Funnel** + S-curve |
| `http://195.201.235.70:5173/strategies` | Tabs: Capital&Risk · Catálogo · MEV Services · Tokens · **Simulación** · Auditoría |
| `http://195.201.235.70:5173/opportunities` | Live opps (50 latest, polling) |
| `http://195.201.235.70:8787/api/scanner/heartbeat?chain_id=1` | Heartbeat snapshot JSON |
| `http://195.201.235.70:8787/api/health` | Health + uptime |
| `http://195.201.235.70:8787/api/trading-config?chain_id=1` | Config snapshot |

---

## 2. Tab Simulación — cómo usarlo

**Path**: `/strategies` → tab **Simulación**.

4 niveles de configuración (todos opcionales, hot-reload <1s al guardar):

### a) Capital simulación global (USD)
Override del `capital_usd` operacional para todo el math del spine. Ej.: tienes
operación con `capital_usd=$10` (test) pero quieres ver oportunidades a $10K
de capital → setea `simulation_capital_usd=10000`. **Cero ejecución on-chain**
(paper-trade está activo por default).

### b) Target profit min (USD)
Filtro de UI (NO gate del backend — backend persiste todo). El dashboard
suprime opps con `expected_profit_usd < target`. Útil para ignorar ruido
sub-$1.

### c) Target ROI min (%)
Mismo patrón. Filtra opps con `roi_pct < target` en la UI.

### d) Caps per-token (USD)
Tabla key→value. Ej.: `WETH=$5000, USDC=$10000`. Cuando el candidate tiene
`token_in == WETH`, el spine usa $5K como cap. **MIN entre todos los caps
aplicables gana** (per-token vs per-strategy vs global).

### e) Caps per-strategy (USD)
Tabla key→value. Ej.: `dex_arb_v2v2=$10000, triangular=$2000`. Cuando el
`strategy_kind` matches, ese cap se aplica. Útil para envelopes de riesgo
distintos por estrategia.

**Migración pendiente del operador (UNA VEZ)**:
Para que opps con WBTC/BNB/LINK/UNI/AAVE/ARB/OP/MATIC dejen de rechazarse
con `UnknownTokenPrice`, poblar `token_prices_usd` en Redis o desde
`/strategies` → tab Tokens. Stables (USDC/USDT/DAI/etc.) ya están a $1
por hardcoded list.

---

## 3. Pipeline Funnel widget — cómo interpretarlo

**Path**: `/operations` → primer card debajo de los KPI cards.

Render del último heartbeat (60s atrás máx). 6 etapas en cascada:

| Etapa | Pregunta que responde |
|-------|------------------------|
| 1. Pending received | ¿Cuántas tx me entrega Alchemy WS? (sample) |
| 2. Decoded OK | ¿Cuántas son routers Uniswap conocidos? |
| 3. Enriched (V2+V3) | ¿De las decodificadas, cuántas tienen ≥2 pools en cache? |
| 4. Passed all gates | ¿Cuántas pasaron allowlist + price + sanity + risk? |
| 5. Persisted to PG | ¿Cuántas se escribieron a opportunities table? |
| ↳ Rejected by gates | Breakdown por razón: TokenNotAllowed / UnknownPrice / AnomalousMath / Other |

**Diagnóstico rápido**:

| Síntoma | Causa probable | Acción |
|---------|----------------|--------|
| Pending alta, decoded baja (<5%) | Mempool dominado por non-Uniswap (normal) | No-op |
| Decoded OK, enriched=0 | Pares no en pool index cache | Añadir pools a PG / esperar |
| Enriched > 0, passed=0 | Risk gate filtra todo | Bajar `min_profit_usd` o subir `simulation_capital_usd` |
| `gate_unknown_token_price > 0` | Token sin price oracle | Añadir entry a `token_prices_usd` |
| `gate_anomalous_math > 0` | Operator misconfig en token_prices_usd | Revisar precios setteados (typos típicos: $10K en vez de $10) |
| `db_errors > 0` | INSERT falla (probable numeric overflow) | Investigar logs `scanner.db_error` — sanity bound debería prevenir |
| Widget 404 / Loading… | Searcher caído o restarted <60s atrás | `ssh arbx 'docker ps \| grep searcher'` |

---

## 4. rejection_reason en el dashboard

Cada opp persistida tiene `rejection_reason` poblado (excepto las que pasan
todos los gates). Query útil:

```sql
-- Distribución de razones de rechazo última hora
SELECT rejection_reason, COUNT(*) AS n
FROM opportunities
WHERE detected_at > NOW() - INTERVAL '1 hour'
GROUP BY rejection_reason
ORDER BY n DESC;
```

Razones posibles:

| `rejection_reason` | Significado |
|--------------------|-------------|
| `TokenNotAllowed:0xb90b…` | Token fuera de tu `allowed_token_symbols` |
| `StrategyDisabled:dex_arb_v2v2` | Strategy no en `enabled_strategies` |
| `UnknownTokenPrice` | Oracle (BUG-2 fix) no resolvió precio para token_in/out |
| `AnomalousMath` | Sanity bound disparó (ROI>999% o profit>$1M = math bug) |
| `LowLiquidity` | Risk gate: liquidez insuficiente vs amount_in |
| `NegativeNetProfit` | Profit < min_profit_usd después de gas + fees |
| `ExcessiveSlippage` | Slippage estimado > max_slippage_pct |
| (NULL) | Pasó todos los gates (raro hoy con $10 capital) |

---

## 5. Comandos operacionales útiles

### Ver heartbeat live
```bash
curl -s http://195.201.235.70:8787/api/scanner/heartbeat?chain_id=1 | jq
```

### Update token prices vía Redis (alternativa al UI)
```bash
ssh arbx 'docker exec arbitragex-v2-redis-1 redis-cli SET arbx:trading_config:1 \
  "$(docker exec arbitragex-v2-redis-1 redis-cli GET arbx:trading_config:1 | \
    python3 -c "import json,sys; c=json.load(sys.stdin); \
    c[\"token_prices_usd\"]={\"WBTC\":95000,\"BNB\":600,\"LINK\":14,\"UNI\":8,\"AAVE\":90,\"ARB\":0.4,\"OP\":1.3,\"MATIC\":0.5}; \
    print(json.dumps(c))")"'
```

### Re-deploy un servicio (siempre con R3)
```bash
ssh arbx 'cd /opt/arbitragex-v2 && git pull origin main && \
  docker compose --env-file .env -f docker/compose.dev.yml build --no-cache <servicio> && \
  docker compose --env-file .env -f docker/compose.dev.yml up -d <servicio>'
```
Servicios: `searcher-rs · api-server · edge · frontend · postgres · redis · ...`

### Ver logs estructurados del searcher
```bash
ssh arbx 'docker logs --since 5m arbitragex-v2-searcher-rs-1 | jq -r "select(.fields.event)"'
```

### Estadísticas opps últimas 24h
```bash
ssh arbx 'docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex \
  -c "SELECT COUNT(*) AS total, COUNT(*) FILTER (WHERE expected_profit_usd > 0) AS profit_pos, \
      COUNT(DISTINCT pair_symbol) AS unique_pairs, COUNT(DISTINCT rejection_reason) AS reject_types \
      FROM opportunities WHERE detected_at > NOW() - INTERVAL \"'24 hours'\";"'
```

---

## 6. Bandera roja — cuándo PARAR el sistema

| Condición | Severidad | Acción |
|-----------|-----------|--------|
| `pg_period_profit_pos > 0` consistentemente con ROI > 100% | 🔴 CRÍTICO | Paper-trade ya está on, pero VERIFICAR antes de subir capital. Probable BUG nuevo. |
| `db_errors > 5/min` sostenido | 🟠 ALTO | Investigar `scanner.db_error` logs. Schema/constraint issue. |
| `gate_anomalous_math > 5/min` | 🟠 ALTO | Sanity bound disparando = misconfig OR nuevo bug matemático. |
| Heartbeat 404 por >5min | 🔴 CRÍTICO | Searcher caído. `docker logs searcher-rs --tail 50`. |
| Frontend `/operations` no responde | 🟡 MEDIO | Container stuck. `docker restart frontend`. |

---

## 7. Sub-tareas pendientes (sprints futuros)

| # | Item | Esfuerzo | Bloquea |
|---|------|---------:|---------|
| REVM Phase 5b | Executor body + fork tests + atomic wiring en simulator.simulate_candidate | 3-4h | Round-trip arb auténtico (vs spread upper-bound actual) |
| Strategy CEX-DEX | Binance/Coinbase WS + cross-correlation + private mempool | 1-2 sem | Diferenciador competitivo (la EXTREMA #7 de §17) |
| Strategy JIT V3 | Just-in-time liquidity provision en pools V3 | 1-2 sem | Diferenciador competitivo (la EXTREMA #5 de §17) |
| Auto-deploy CD | Webhook GitHub → VPS rebuild | 2-3h | Eliminar manual deploy steps (valor cuestionable single-op) |
| Allowlist expansion | Operacional: añadir tokens mainstream (PEPE, SHIB, etc.) | min | Aumentar enriched_v2/v3 throughput |

---

## 8. Doctrina recordatoria (R0-R8)

- **RULE 00 — Zero Mocks**: si no hay dato real → vacío/loading/error
- **RULE 01 — Deploy LOCAL→GIT→VPS**: ssh arbx → git pull → docker rebuild
- **RULE 02 — REST→Edge / WS→api-server directo**
- **RULE 03 — `--no-cache --env-file .env` siempre**
- **RULE 04 — NEXT_PUBLIC_* baked at build time** (rebuild si cambia env)
- **R1 — Mounted Snapshot Pattern** (Server Component → initialSnapshot → Client polling)
- **R2 — Build-Time Guard** (next.config.js bloquea localhost en prod)
- **R5 — Auditoría componentes transitivos** (cuando un fix en hidratación)
- **R6 — Completitud Docker Compose** (DATABASE_URL + depends_on healthy)
- **R7 — Trazabilidad E2E** (searcher→Redis→PG→API→Frontend)
- **R8 — Fail-Honest** (null > inventar; rechazar > fabricar; 404 > stale)

Cuando dudes, defaultea a **R8**: el sistema PUEDE mostrar "no data", "loading",
"error", "rejected" — TODOS son estados aceptables. Lo único INACEPTABLE es
inventar un valor para que la UI "se vea bien". El operador toma decisiones
de capital basadas en lo que ve; mentirle es la ofensa máxima.
