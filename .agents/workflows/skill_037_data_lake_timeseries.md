# SKILL 037 — Data lake & time-series storage

## 1. Propósito superior
Construir y mantener el repositorio inmutable de toda la historia del agente. Almacenar los miles de millones de eventos (Prices, Spread Gaps, Execution Logs, Latencies, Reverts) producidos diariamente en una base de datos de Series Temporales (Time-Series Database, ej. InfluxDB, ClickHouse, TimescaleDB, QuestDB) sin bloquear los hilos operativos. Provee la "Materia Oscura" (Big Data) para que los motores de IA y Machine Learning posteriores puedan extraer patrones, entrenar modelos probabilísticos (Skill 9) y auditar hasta el último Wei generado en PnL.

## 2. Nivel de conocimiento requerido
Arquitecto de Bases de Datos de Rendimiento Extremo. Entendimiento profundo del almacenamiento columnar vs orientada a filas, algoritmos de compresión TSDB (Gorilla compression), indexación por tiempo, Buffering & Batching writes, e I/O Asíncrono puro.

## 3. Capacidades principales
1. Ingesta Asíncrona Masiva: Recibir 5,000 puntos de datos por segundo y agruparlos en un buffer de memoria, enviándolos a la base de datos en lotes (Batches) gigantes (Ej. cada 1 segundo o cada 10MB) mediante un hilo (Background Worker) para no frenar la CPU central.
2. Indexación Automática por Tags: Etiquetar cada registro con Cardinalidad Controlada (Ej. `Exchange`, `Pair`, `Strategy_Id`) y guardar la métrica dinámica en los Fields (Ej. `Spread_Pct`, `Latency`, `Profit`).
3. Estructuración Columnar: Diseñar esquemas que permitan a las queries de Inteligencia Artificial ("Dame el spread promedio de los últimos 6 meses en ETH/USDT") resolverse en 50 milisegundos en lugar de 5 horas leyendo filas individuales.
4. Downsampling (Agrupación Continua): Tareas automáticas en la BD que toman la data bruta de "1 tick" y la agrupan en velas (Klines) de 1 Segundo, 1 Minuto y 1 Hora para ahorrar espacio de disco de manera autónoma.
5. Políticas de Retención Cíclicas (Retention Policies): Borrar data cruda de Order Books de hace más de 7 días (que ocuparía Terabytes inútiles) reteniendo únicamente los rollups (datos comprimidos).
6. Separación de Telemetría (Metrics) de Auditoría de Ejecución (Ledger): Los datos del mercado pueden purgarse, pero las ejecuciones reales de Arbitraje son inmutables de por vida para fines contables y de cumplimiento fiscal (Compliance).
7. Tolerancia a Fallos de Conexión: Si InfluxDB / ClickHouse se cae por 5 minutos, el Buffer RAM del bot debe serializar los logs temporales al disco (WAL - Write-Ahead Logging local) para no perder un solo tick de historia, y re-ingestarlo cuando la BD vuelva.
8. Optimización del tamaño del JSON: Utilizar formatos binarios (Protobuf, Influx Line Protocol, Parquet) para comprimir los datos antes de enviarlos a la BD.
9. Consultas de Backtesting Instantáneas: Proveer una API de lectura nativa para los módulos internos del bot que requieran consultar el pasado (Skill 9 - Probabilidad Bayesiana).
10. Monitoreo del "Cardinality Explosion": Prevenir que se inyecten UUIDs aleatorios como Tags, lo cual destrozaría los índices RAM de InfluxDB y colapsaría el clúster.

## 4. Entradas requeridas
- `data_point`: Un evento atómico (Latencia, Spread, Trade, Balance Change).
- `timestamp`: Marca de tiempo en precisión Nanosegundos (Unix Nanoseconds).
- `tags`: Meta-datos de baja cardinalidad (Indexados).
- `fields`: Datos cuantitativos (No indexados).

## 5. Salidas esperadas
- `batch_write_receipt`: Confirmación de 200 HTTP o ACK binario de la base de datos.
- `disk_usage_alert`: Alerta si la BD de Series Temporales se acerca al límite de disco asignado.
- `historical_query_result`: Array masivo de datos para módulos de ML y Backtesting.

## 6. Reglas inmutables
- Nunca escribir en la base de datos utilizando comandos síncronos o de una-en-una inserción (Single Writes). Esto aniquilará el disco NVMe o SSD (Write amplification) y paralizará al Orquestador. SIEMPRE usar Micro-Batching (Lotes de 5,000-10,000 registros).
- NUNCA usar Bases de Datos Relacionales Clásicas (MySQL, PostgreSQL) para guardar Data de Nivel de Tick (OrderBooks y Trades cada 10ms). PostgreSQL colapsará rápidamente o consumirá recursos colosales frente a soluciones especializadas como QuestDB o ClickHouse. (Usar SQL clásico solo para la contabilidad y estados de cuentas).
- Evitar usar UUIDs generados por transacción como Tags indexables en InfluxDB, ya que esto crea Cardinalidad Infinita y consume el 100% de la memoria RAM del servidor de datos en horas.

## 7. Algoritmos o métodos que debe conocer
- Line Protocol Formatting (InfluxDB) / SQL Columnar Insert.
- B-Tree vs LSM-Tree Databases (Log-Structured Merge-Tree para extrema velocidad de escritura).
- Buffered I/O Stream & WAL (Write-Ahead Logging).

## 8. Fórmulas críticas
- **Tamaño de Buffer de Ingesta**: `Buffer_Flush_Condition = Record_Count > MAX_BATCH_SIZE OR Time_Elapsed > MAX_FLUSH_INTERVAL`
- **Cálculo de Cardinalidad**: `Tags_Cardinality = Count(TagA_values) * Count(TagB_values) * Count(TagC_values)` (Mantener < 1,000,000 para salud del motor).

## 9. Casos extremos
- Interrupción Crítica de Red (No Route to Host): La Base de Datos (en otra máquina) se apaga para un update del SO. El bot sigue generando 10,000 registros por segundo en RAM. El límite de RAM se excede y NodeJS/Rust genera "OOM Kill". El módulo debe tener un mecanismo Spool-To-Disk (Guardar a archivo plano local `.log`) y volcar asíncronamente cuando la DB regrese.
- Ruido Excesivo (Dust Logging): El bot guarda todos los Bids y Asks a 10 niveles de profundidad de 500 pares, generando 1 Terabyte de datos al día de liquidez inoperable, llenando el disco en 3 días. (Filtrado heurístico antes de guardar).
- Fallo de Formato de Tiempo (Timestamp collision): Dos eventos HFT en el mismo milisegundo se sobreescriben mutuamente en la TSDB si no se usa precisión de Nanosegundos combinada con diferenciadores espaciales.

## 10. Validaciones obligatorias
- PRE: Validar que todos los Tags sean Strings y los Fields sean estrictamente numéricos (Float/Int). Tipos mezclados (ej. Precio como Float un día y String otro día) rompen las bases de datos de series temporales irremediablemente.
- CÁLCULO: Mantener métrica de "Dropped Points" (Puntos caídos por error de parseo o fallo de BD) para alertar sobre posible ceguera histórica.
- POST: Vaciar la memoria inmediatamente tras el `ACK` de la base de datos para habilitar la recolección de basura (Garbage Collection).

## 11. Criterios de aprobación
- La base de datos es capaz de tragar > 100,000 inserciones por segundo desde el bot usando < 5% de uso de CPU en la parte del emisor.
- Queries históricas agregadas (Ej. GROUP BY 1 Hour) de meses enteros de datos resuelven en milisegundos.

## 12. Criterios de rechazo
- El proceso de Buffer de inserción ralentiza el Event Loop principal (Node) o causa bloqueos Mutex que atrasan la matemática core del HFT.
- La base de datos arroja "Maximum Series Limit Reached" (Cardinality Explosion).

## 13. Riesgos que mitiga
- Riesgo Analítico Nulo (Blind AI): La Inteligencia Artificial y Machine Learning son inútiles sin datos históricos estructurados de alta resolución. Este Data Lake es el combustible pesado de toda evolución futura del bot.
- Pérdida de Auditoría Fiscal: Sin registros permanentes e inmutables, explicar millones de operaciones anuales a reguladores o inversores es imposible. (Skill 38 se apoya aquí).

## 14. Integración con otras skills
- Cliente consumidor de telemetría (Skill 34, Skill 39) y eventos de mercado puros (Normalizador - Skill 32).
- Proveedor directo de inteligencia al Motor Bayesiano (Skill 9).

## 15. Modelo de datos sugerido
```json
{
  "DataLakeMetric": {
    "measurement": "spread_opportunity",
    "timestamp_ns": 1714521234105123456,
    "tags": {
      "pair": "BTC_USDT",
      "cex_buy": "kraken",
      "cex_sell": "binance",
      "strategy_id": "ST_01"
    },
    "fields": {
      "spread_bps": 12.5,
      "latency_ms": 35.1,
      "projected_profit_usd": 14.2
    }
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Un Background Thread/Worker completamente independiente que expone una función síncrona, asimétrica y no bloqueante (Fire-and-Forget): `DataLake.record(measurement, tags, fields)`. El Worker agrupa y vacía vía HTTP POST Masivo (Ej. InfluxDB Write API `_write?precision=ns`).

## 17. Logs obligatorios
- `[DEBUG] DataLake Worker: Flushed batch of 15,400 data points to ClickHouse in 45ms.`
- `[WARN] Time-Series DB Connection Timeout. Buffering data to local disk (WAL). Buffer size: 250MB.`
- `[CRITICAL] Dropped 500 records due to malformed Field Data Type (Expected Float, got String). Discarding poisoned batch.`

## 18. Métricas obligatorias
- `datalake_points_queued_count`.
- `datalake_batch_flush_latency_ms`.
- `database_dropped_points_total`.
- `wal_disk_spillover_bytes`.

## 19. Tests unitarios
- Buffer Auto-Flush: Inyectar 100 eventos, el límite de flush es 50. La cola de mock debe disparar 2 eventos de Write HTTP exactamente.
- Time Flush: Inyectar 1 evento y esperar el límite de tiempo de Flush (Ej. 1000ms). El buffer debe vaciarse automáticamente para no dejar el dato estancado en RAM horas si no hay más tráfico.
- WAL (Write-Ahead Logging): Simular error HTTP 503 desde la base de datos simulada. El bot debe crear un archivo `.wal` temporal en disco y encolar el Payload para un reintento futuro automático.

## 20. Tests de integración
- Levantar un contendor Docker real de InfluxDB v2 o QuestDB. El bot debe mandarle 5 millones de registros en 1 minuto, y un script de validación debe contar exactamente 5 millones en la BD, verificando que los nanosegundos no colisionan.

## 21. Tests E2E
- El bot ejecuta durante 1 hora. Realiza operaciones y recibe spreads. Un módulo de AI (Simulado) llama al Endpoint del Data Lake pidiendo el "Promedio móvil del Spread de Bybit/Binance en velas de 1 segundo". El Data Lake entrega la consulta consolidada en 15ms.

## 22. Checklist de producción
- [ ] Uso exclusivo de compresión (Gzip / Snappy / LZ4) en la petición HTTP POST hacia la Base de datos; 10,000 registros en texto plano pueden pesar 5MB, en GZIP pesan 200KB, salvando ancho de banda del datacenter.
- [ ] Protocolo "Line Protocol" o CSV nativo (No JSON masivos) para inyectar datos a Bases de datos Time-Series.
- [ ] Indexación estricta de Tags vs Fields (Ej. No poner el `TxHash` como Tag Indexable, pues el índice RAM colapsará en 1 hora. El `TxHash` va en Field como un string).

## 23. Ejemplo de configuración no hardcodeada
```yaml
datalake_engine:
  type: "influxdb_v2" # or "clickhouse"
  write_endpoint: "http://db-internal:8086/api/v2/write"
  batch_size_points: 10000
  flush_interval_ms: 1000
  wal_directory: "/var/log/arbitragex/wal"
  enable_gzip_compression: true
```

## 24. Ejemplo de pseudocódigo
```javascript
class TimeSeriesBuffer {
    constructor(config) {
        this.buffer = [];
        this.batchSize = config.batch_size;
        this.interval = setInterval(() => this.flush(), config.flush_interval_ms);
    }

    record(measurement, tags, fields) {
        // Formulate InfluxDB Line Protocol: measurement,tag1=a,tag2=b field1=1.5,field2=2.0 timestamp_ns
        const tagStr = Object.entries(tags).map(([k,v]) => `${k}=${v}`).join(',');
        const fieldStr = Object.entries(fields).map(([k,v]) => `${k}=${v}`).join(',');
        const timestamp = process.hrtime.bigint(); // Monotonic + Absolute combination approximation

        this.buffer.push(`${measurement},${tagStr} ${fieldStr} ${timestamp}`);

        if (this.buffer.length >= this.batchSize) {
            this.flush(); // Fire asynchronously
        }
    }

    async flush() {
        if (this.buffer.length === 0) return;
        
        const payload = this.buffer.join('\n');
        this.buffer = []; // Clear RAM instantly to catch new points
        
        try {
            await http.post(CONFIG.write_endpoint, compress(payload), { headers: { 'Content-Encoding': 'gzip' } });
        } catch (error) {
            // Write to disk (WAL) on DB failure to preserve historical integrity
            WAL.writeToDisk(payload);
        }
    }
}
```

## 25. Criterio final de excelencia
El motor de Data Lake es una aspiradora industrial implacable que retiene y almacena terabytes de información hiperdensa del mercado en tiempo real sin gastar un solo ciclo de CPU operativo crítico, construyendo silenciosamente la mina de oro analítica de todo el fondo cuantitativo.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Corrupción de bases de datos de alta velocidad en cortes de energía (Evitado usando WAL e instanciando la DB en nodos replicados Cloud).
- Dependencias: Orquestador Concurrente (Worker), Infraestructura TSDB externa.
- Próxima skill: Reconciliación de balances (Accounting) (Skill 38).
