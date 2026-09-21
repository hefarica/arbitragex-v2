# Feed CEX Binance WebSocket — variables de entorno (BE-3.2 Fase 2)

Writer: `backend/searcher-rs/src/workers/binance_stream_worker.rs` (spawn en `main.rs`).
Lector/merge: `backend/shared-rs/src/price_oracle.rs` (`merge_cex_fallback`, precedencia on-chain absoluta vía `or_insert`).

## Flags

| Variable | Default | Descripción |
|---|---|---|
| `ARBX_BINANCE_WS_ENABLED` | `true` (ON) | Master switch. Solo los valores `false`, `0`, `off`, `no` (case-insensitive) desactivan el spawn del worker. Cualquier otro valor —o ausencia— lo deja ON: el feed es best-effort y su fallo degrada honestamente, no desactiva nada. |
| `ARBX_BINANCE_WS_SYMBOLS` | `BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,AVAXUSDT` | Lista CSV de símbolos spot para el combined `bookTicker` stream. Se normalizan a mayúsculas; se filtran los < 5 chars. Lista inválida/vacía → warn + defaults (nunca crash de boot). |
| `ARBX_BINANCE_WS_CHANGE_THRESHOLD_PCT` | `0.01` | Umbral percentual de cambio de mid para reescribir la entrada en el hash CEX (0.01% filtra ruido de top-of-book). Rango válido `(0, 100)` finito; valor malformado → warn + default. |

## Comportamiento

- Endpoints: `wss://stream.binance.com:9443` y `:443` (rotación en reconnect), URL combined-stream `<base>/stream?streams=<sym>@bookTicker/…`.
- Escritura ADITIVA en el hash Redis `arbx:cex_prices:<chain_id>` (TTL 300 s) — campo = BASE asset, valor = JSON `CexPriceEntry {price, ts_ms, quote, source}`. JAMÁS escribe en `arbx:token_prices:<chain_id>` (torre on-chain).
- Fusión con precedencia on-chain absoluta: un valor CEX solo entra con `entry().or_insert(...)` — nunca sobrescribe un precio on-chain (Chainlink/price_worker). Mapping BASE→símbolo oracle: `ETH→WETH`, `BTC→WBTC`, resto identidad.
- Reescritura forzada a los 30 s (keepalive de TTL/staleness) aunque el mid no cruce umbral.
- Watchdog de socket zombie: 60 s sin frames → reconnect con backoff exponencial 1 s→30 s.

## Degradación honesta (RULE 00 / R8)

Geo-block (HTTP 451), fallo DNS o socket muerto NO fabrican precios:
`arbx_binance_ws_feed_healthy` pasa a `0`, `arbx_binance_ws_reconnects_total` incrementa y la torre on-chain queda en pie como única fuente. El hash CEX expira solo (TTL) — no se sirven precios CEX stale.

## Métricas Prometheus (shared-rs `metrics.rs`)

| Métrica | Significado |
|---|---|
| `arbx_binance_ws_messages_total{chain_id}` | Frames bookTicker recibidos (todos los símbolos). |
| `arbx_binance_ws_reconnects_total{chain_id}` | Intentos de reconexión (rotación + backoff). |
| `arbx_binance_ws_change_events_total{chain_id,symbol}` | Cambios de mid que cruzaron el umbral y se persistieron. |
| `arbx_binance_ws_feed_healthy{chain_id}` | 1 = conectado y escribiendo; 0 = degradado (torre on-chain manda). |
