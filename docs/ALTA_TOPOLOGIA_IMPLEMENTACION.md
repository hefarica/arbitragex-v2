# ArbitrageX v2 — Implementación de alta topología

Entrega sobre el ZIP aportado por el usuario, 11 de septiembre de 2026.

Se implementaron y verificaron mejoras del núcleo existente. **Esta entrega no acredita el sistema completo de 264 estrategias ejecutables ni habilita su operación en mainnet.** El inventario canónico se conserva; la cobertura y las conexiones pendientes se detallan en `ALTA_TOPOLOGIA_COBERTURA.md` y `.json`.

## Cambios conectados al código existente

| Componente | Cambio y comportamiento |
|---|---|
| Op32 NSGA-II | Nuevo operador registrado y disponible en la API de `math-engine`. Ordenación no dominada, crowding, selección elitista, torneos y API de evolución con validación y evaluación obligatorias de cada candidato nuevo. |
| Compatibilidad de operadores | Se mantienen los significados de Op01–31. El nuevo prompt utiliza otra numeración para varios conceptos; el informe incluye su equivalencia. La proyección histórica 264×31 conserva sus asignaciones, sin inventar una columna estratégica para Op32. |
| Grafo y pares | `build_dense` incorpora `PairBuckets<u32>` del módulo existente. Comparte IDs y snapshot con CSR y bitsets; conserva dirección y pools paralelos. Se utiliza para agrupar pools en el buscador de ciclos cortos. El tamaño estimado de la caché limita su construcción; los grafos grandes conservan la ruta CSR/HashMap. |
| Lectura de adyacencias | Nuevo iterador prestado de índices; evita construir un `Vec` de índices en cada visita. Ambos buscadores lo consumen. |
| Agrupación de pools | El buscador de ciclos cortos calcula los índices podados y ordenados una vez por token visitado y los reutiliza durante esa pasada. La caché es local al snapshot; la liquidez y las prioridades se vuelven a evaluar en la siguiente pasada. |
| Hash canónico | Se descartan antes de renderizar las rotaciones cuyo primer token no puede ser el mínimo lexicográfico. Los empates siguen comparando toda la tupla. Una prueba diferencial de 768 casos, incluyendo tokens repetidos, verifica los mismos hashes y campos que el algoritmo histórico. |
| Bitsets | Intersección de adyacencia, allowlist y visitados sin asignaciones, incluyendo N>64. La búsqueda utiliza además la poda por vecinos no visitados, permitiendo cerrar en el token inicial. La función de intersección está disponible; no sustituye la configuración de allowlist de la ingesta. |
| Trabajo de discovery | Los buscadores de ciclos cortos y multihop tienen un presupuesto predeterminado de 100.000 aristas recorridas por pasada, independiente del número de resultados. Las APIs permiten variar ese presupuesto. Se informa cuando el resultado es incompleto. |
| Hops | Intersección exacta de bits con el intervalo 2–7. Una máscara sin longitudes admisibles sale sin explorar. Los huecos de la máscara no se convierten en longitudes permitidas. |
| Validez numérica | Pesos NaN/infinito y sumas no finitas se excluyen y cuentan. El ruido de beneficio marginal se descarta antes de ocupar la capacidad de resultados. Ninguno de estos filtros equivale a validar beneficio neto. |
| Rotaciones | El paso de análisis deduplica rotaciones de la misma secuencia dirigida. El camino usado por el scanner ejecutable las conserva, porque el activo inicial puede cambiar las posibilidades de préstamo y financiación. |
| Tiempo | El paso multihop instrumentado del radar tiene un plazo cooperativo de 7 ms, revisado cada 64 aristas. Las llamadas puras conservan determinismo. Este plazo no garantiza el SLA de todo el pipeline y puede sobrepasarse por planificación del sistema o trabajo entre comprobaciones. |
| Telemetría y frontend | Se añaden contadores de aristas, límites de trabajo/tiempo, pesos inválidos y duplicados. Se actualizaron el esquema Zod, los fixtures emitidos por Rust y la prueba de contrato. Los campos nuevos son opcionales al leer productores anteriores y no se rellenan con ceros. |
| Benchmark existente | Usa la vista densa real, aplica la máscara de estrategia y muestra truncamiento y trabajo de ambas búsquedas junto con latencias. Sus datos son sintéticos y están etiquetados como tales. |

## SIMD: disponible como primitiva de preselección

`backend/searcher-rs/src/batch_quote.rs` expone `batch_quote_cpmm_approx` y su equivalente escalar. Valida longitudes, importes, reservas y comisión; detecta AVX en ejecución; procesa grupos de cuatro y el remanente; evita desbordamientos del cociente CPMM y devuelve errores explícitos para resultados no representables. El uso de AVX sigue el mecanismo oficial de [detección de capacidades de Rust](https://doc.rust-lang.org/std/macro.is_x86_feature_detected.html).

**No se conectó SIMD al cálculo entero final de `amount_buckets`.** Ese cálculo exige U256, redondeos y simulación del protocolo; reemplazarlo por `f64` cambiaría la economía de las operaciones. La nueva primitiva está exportada y probada para preselección, pero aún requiere un consumidor de cotizaciones aproximadas que mida su efecto.

## Utilizar Op32

El módulo tipado es `math_engine::operators::op_32_nsga2`. `Nsga2Optimizer::select` recibe un vector de `Objectives` con `expected_profit`, `risk_cvar` y `latency_ms`; devuelve índices hacia los candidatos originales. El beneficio debe ser neto, el CVaR una pérdida no negativa y las latencias comparables. Todos los valores deben ser finitos, compartir unidades/horizonte y corresponder al estado que se está evaluando.

- `selected_indices`: supervivientes hasta `population_size`; puede incluir frentes posteriores.
- `pareto_indices()`: primer frente de la entrada completa; puede superar el límite de supervivientes.
- `evolve`: el llamador proporciona cruce/mutación con conocimiento de la ruta, valida factibilidad y vuelve a evaluar objetivos. Los cromosomas no se convierten automáticamente en transacciones.
- API tipada: hasta 1.024 candidatos; máximo 64 generaciones; valor predeterminado de generaciones 0.
- API HTTP existente: `POST /api/compute`, `operator_ids: [32]`, dentro de `market_state.features`: `nsga2.count`, `nsga2.population_size` opcional y, para cada índice i, `nsga2.i.net_profit_usd`, `nsga2.i.risk_cvar_usd`, `nsga2.i.latency_ms`. Máximo 512 candidatos.
- Sin objetivos completos, devuelve campos de resultado ausentes y `computed=0`. El escalar representa el tamaño del frente de Pareto, **no dólares de beneficio**.

El runtime aún necesita producir CVaR y latencia comparables por candidato para usar Op32 como ranking económico automático. Su incorporación al registro y a la API no equivale a conectar esos datos ni a entrenar los operadores que carecen de modelo.

## Verificación

| Comprobación | Resultado |
|---|---|
| `math-engine --lib --features api` | 132 aprobadas, 0 fallos |
| `searcher-rs --lib` | 1173 aprobadas, 0 fallos, 3 ignoradas por dependencia de Redis |
| Contratos y esquemas del frontend | 74 aprobadas en 7 archivos |
| `cargo check --workspace --all-targets` | PASS |
| `npm run typecheck` / `npm run build` | PASS / PASS con advertencias heredadas |

**Benchmark release:** 30 configuraciones, 400 muestras medidas y 50 calentamientos por configuración; sin reducir las muestras con variables de entorno. Linux x86_64, Rust 1.91.0, CPU AMD EPYC 9V74 (9 CPU visibles en el contenedor compartido).

El caso base obtuvo **p95 11.571 ms**, p50 8.088 ms y p99 12.689 ms. El gate formal existente devuelve **PASS**, y juzga únicamente ese caso base.

| Hops máximos | p95 antes de optimización final (ms) | p95 entrega (ms) | Límite de trabajo multihop agotado |
|---:|---:|---:|---|
| 2 | 1.478 | 1.252 | No |
| 3 | 15.250 | 9.143 | No |
| 4 | 17.795 | 9.557 | Sí |
| 5 | 25.042 | 10.570 | Sí |
| 6 | 29.808 | 11.888 | Sí |
| 7 | 44.460 | 14.769 | Sí |

Las 30 configuraciones observadas quedaron por debajo de 30 ms de p95; el máximo fue 14.769 ms (hop=7). Esto amplía la evidencia de esta corrida, sin cambiar el alcance del gate formal.

Las asignaciones por pasada base bajaron de 112.712 a 46.480, según la sonda de asignaciones fuera del tramo cronometrado. Los contadores de rutas, ciclos, cotizaciones, aristas recorridas y truncamientos coincidieron entre ambas mediciones en las 30 configuraciones.

**Límite de interpretación:** se miden `find + multihop + rank + refine` sobre datos sintéticos. El caso base usa 22 tokens, 66 pares presentes, 165 pools y 330 aristas dirigidas de pool; produce 500 candidatos topológicos y marca `capped=true`. Los puntos que agotan el presupuesto de 100.000 aristas también son incompletos. No es enumeración exhaustiva de 231 pares ni evidencia de oportunidades rentables.

Las dos mediciones release completas, el diagnóstico dev previo y los logs de validación se incluyen en `docs/alta_topologia_evidencia/`. `ALTA_TOPOLOGIA_VALIDACION.json` conserva resultados estructurados. Las cifras de throughput del harness cuentan rutas/cotizaciones sintéticas; no acreditan operaciones rentables por segundo.

Los tests usan casos controlados; no se inyectaron esos datos en producción. Los tres tests ignorados de la biblioteca del searcher se mantienen declarados como ignorados, sin contarlos como aprobados. El build web conserva advertencias heredadas de dependencias y lint; el detalle está en la evidencia.

Comandos reproducibles desde la raíz del proyecto, con Rust 1.91.0 y dependencias del lockfile:

```bash
cargo test --locked --manifest-path backend/Cargo.toml -p math-engine --lib --features api
cargo test --locked --manifest-path backend/Cargo.toml -p searcher-rs --lib
cargo check --locked --manifest-path backend/Cargo.toml --workspace --all-targets
npm ci --ignore-scripts --no-audit --no-fund
npm run typecheck
npm run build
npm run test --workspace=@arbx/frontend -- lib/apex/schemas/__tests__
```

Benchmark release con las muestras completas y sin variables `DM_SAMPLES`/`DM_WARMUP` definidas:

```bash
cargo bench --locked --manifest-path backend/Cargo.toml -p searcher-rs --bench discovery_matrix
```

Benchmark de desarrollo (no certifica SLA):

```bash
DM_SAMPLES=20 DM_WARMUP=3 cargo bench --locked --manifest-path backend/Cargo.toml -p searcher-rs --bench discovery_matrix --profile dev
```

La medición formal del benchmark existente requiere perfil release y sin reducir muestras. Incluso un PASS de ese benchmark solo acredita su carga sintética y su perímetro; el SLA operativo requiere replay de snapshots reales, hardware identificado y medición de colas, ingesta y concurrencia.

## Trabajo pendiente frente al prompt completo

El inventario tiene 79 entradas `ROUTE_READY`, 174 `NEEDS_ROUTE_DATA`, 8 `OBSERVE_ONLY` y 3 `NO_COMPATIBLE_ROUTE`. `ROUTE_READY` no acredita ejecución de capital.

Permanecen pendientes la conexión completa de fuentes, contratos y settlement por familia; correcciones monetarias en motores heredados de liquidación/cross-chain/flash loans; despliegue y comprobación del nodo/RPC; ejecución GPU real; datos de riesgo/latencia para ranking automático; y validación de extremo a extremo. El ejecutor heredado rechaza mainnet Ethereum y el constructor de bundles admite una clase limitada de estrategias. Se documentaron esos bloqueos sin afirmar que esta entrega los resolviera.

Para el envío de bundles, la interfaz del searcher es `eth_sendBundle`/`eth_callBundle` y exige transacciones firmadas y simulación para un bloque concreto, según la [documentación oficial de Flashbots](https://docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint). Su existencia no demuestra inclusión, settlement o rentabilidad.

No se desplegó, no se enviaron transacciones y no se midieron ganancias. USD 1.000–10.000/hora, win rate >70%, Sharpe >3 y los throughputs solicitados permanecen objetivos sin acreditar.
