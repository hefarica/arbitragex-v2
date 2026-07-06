# SKILL: Database Design & Time-Series Optimization
**Level:** PhD Database Systems | Time-Series Architect
**Specialty:** Tick Data Storage & Query Optimization

## AGENT DIRECTIVE
Los datos son tu petróleo. Almacénalos con eficiencia extrema. Consulta en milisegundos.

## CORE KNOWLEDGE
- **Time-Series DB:** InfluxDB, TimescaleDB, ClickHouse, kdb+
- **Compression:** Gorilla, Delta, RLE, Dictionary encoding
- **Partitioning:** Time-based, hash-based, range-based

## TICK DATA SCHEMA (TimescaleDB)
```sql
CREATE TABLE tick_data (
    time TIMESTAMPTZ NOT NULL,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    size DOUBLE PRECISION NOT NULL,
    side SMALLINT NOT NULL,
    order_id BIGINT,
    PRIMARY KEY (time, symbol, exchange, order_id)
);
SELECT create_hypertable('tick_data', 'time', chunk_time_interval => INTERVAL '1 hour');
```

## COMPRESSION RATIOS
```
Tick data: 10:1 con Gorilla + Delta
OHLCV: 20:1 con Dictionary + RLE
Order book: 5:1 con Delta
```

## KDB+ (Gold Standard)
```q
// In-memory, vectorized, 100M+ rows/second
tick: ([] time: `timestamp$(); symbol: `symbol$(); price: `float$())
select last price, sum size by symbol from tick
```
