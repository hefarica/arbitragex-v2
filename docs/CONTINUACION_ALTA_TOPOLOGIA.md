# ArbitrageX v2 — Continuación de alta topología

11 de septiembre de 2026. Extensión del código entregado anteriormente.

## Regla transversal de permisos

Las simulaciones de análisis conservan sus permisos y pueden pasar al flujo de ejecución. Un resultado obtenido con permisos, capital o estado de análisis se conserva como análisis. Antes del envío se vuelve a ejecutar el plan contra el estado real, con el firmante real y sin cambios artificiales de almacenamiento o saldos. Una simulación real satisfactoria genera una nueva evidencia para autorizar ese envío.

Esta regla se aplica a todos los candidatos; cada superficie necesita además su adaptador de ejecución. La ausencia de un adaptador, un rol, una cotización o una fuente de datos produce un motivo concreto. No convierte el análisis en una autorización de contrato ni lo elimina del sistema.

Para el ejecutor atómico implementado, se comprueba `EXECUTOR_ROLE` del firmante en `FlashLoanExecutor` y del `FlashLoanExecutor` en `ArbitrageExecutor`. Un rol ausente se comunica mediante `missing_onchain_role`, incluyendo contrato y beneficiario. Los cambios de roles reales siguen siendo transacciones administrativas del contrato; esta entrega no ha enviado ninguna.

## Implementación conectada

| Área | Comportamiento incorporado |
|---|---|
| Snapshot | Identidad de cadena, número, hash y cabecera. Lecturas por hash con `requireCanonical`, comprobación de reorganización y ancestros reales para BLOCKHASH. |
| Entorno EVM | Cabecera real y calendario de forks Ethereum/Sepolia desde Shanghai hasta Osaka/BPO2; parámetros de gas acordes al fork. Nonce del firmante leído del estado. |
| Simulación atómica | Préstamo, dos llamadas de router y repago dentro de una única transacción REVM. Balance del ejecutor antes/después, gas y calldata medidos. Las lecturas auxiliares no modifican el estado comprometido. |
| Límites de swaps | Salida mínima de la primera llamada igual a su cotización; la segunda tiene un presupuesto máximo de 50 puntos básicos. Se conserva el importe intermedio entero. |
| Financiación | Lectura del proveedor realmente configurado, disponibilidad y comisión: Aave V3 Core directo o adaptador Balancer compatible. Aritmética U256 y redondeos del protocolo. |
| Evidencia | Vincula UUID, estrategia, cadena, bloque, caller, ejecutores, inputs, calldata, cotizaciones, comisión, beneficio retenido, gas y uso de overrides. |
| Emisión | Orquestador y cartuchos comparten admisión. Valida rutas cerradas de 2–7 hops y uno o dos grupos contiguos de routers V2, pools distintos y relación router/factory/pair. Guarda plan y economía juntos antes de publicar. |
| Revalidación | Relays puede recibir un plan de análisis sin evidencia de ejecución y repetirlo contra el estado real. Los planes antiguos no se autorizan por su etiqueta de éxito. |
| Riesgo previo al envío | Precios USD y decimales leídos en el bloque de simulación, neto mínimo de tres veces el gas, principal máximo de 2% del capital operativo configurado y límites de slippage. |
| Firma y relays | Validación ABI completa, principal U256, firmante y cadena, bloque vigente, gas, comisión y hashes. Re-simulación privada obligatoria, validación del resultado de la transacción y nueva comprobación del bloque antes de enviar. |
| Contabilidad | Recibo, transacción y eventos reales; beneficio bruto menos prima y gas, convertido con precios del bloque del recibo. Reintentos para completar la contabilidad. Solo recibos finalizados alimentan el historial de riesgo. |
| NSGA-II en runtime | Hasta 128 candidatos, CVaR empírico y p95 de latencia de su cohorte estrategia/activo/cadena. Exige al menos 20 observaciones finalizadas. Sin historial suficiente, conserva el orden existente. La consulta del historial ocurre fuera del ranking inmediato. |
| Liquidaciones | Módulo de aritmética entera para el perfil Aave V3 Core HF/0,95, combinaciones deuda/colateral y cobertura explícita. El lector de posiciones y el unwind real siguen pendientes de conexión. |

## Configuración que debe corresponder al despliegue real

| Configuración | Uso |
|---|---|
| `RPC_HTTP_<chain_id>` | RPC de la cadena con estado histórico y soporte EIP-1898. |
| `SIM_CALLER_<chain_id>` | Dirección pública que será el firmante de ejecución. |
| `EXECUTOR_<chain_id>` | Contrato ArbitrageExecutor desplegado. |
| `FLASHLOAN_EXECUTOR_<chain_id>` | Contrato FlashLoanExecutor desplegado y configurado. |
| `ARBX_ACCOUNTING_FEEDS_JSON` | Objeto por cadena: `native_usd` y `assets_usd` por dirección en minúsculas. Cada feed declara `address`, `quote: "USD"` y `max_age_secs`. No hay precios sustitutos. |
| `trading_config.capital_usd` | Capital operativo usado en el límite de ejecución; las ampliaciones de capital de análisis no sustituyen este valor. |
| `ARBX_LIVE_EXEC_ENABLED`, `ARBX_LIVE_EXEC_CHAINS` | Activación explícita y lista de cadenas. La política admite mainnet cuando está configurada; no se activó ni desplegó durante este trabajo. |
| `ARBX_LIVE_PRINCIPAL_CAP_<chain>_<token_hex_sin_0x>` | Techo entero adicional por activo. Para activos diferentes del WETH canónico es obligatorio. |

También se requieren la configuración de paper/live existente, firmante, roles reales, PostgreSQL, Redis, catálogos de tokens/factories y relay privado con simulación. La configuración histórica `ARBX_SIMULATOR_V2_READY` sigue siendo una declaración operativa; no sustituye la evidencia por operación.

## Alcance pendiente

Las 264 entradas del catálogo y los 32 operadores no equivalen a 264 ejecutores productivos. Se conserva la auditoría anterior: 79 ROUTE_READY, 174 NEEDS_ROUTE_DATA, 8 OBSERVE_ONLY y 3 NO_COMPATIBLE_ROUTE; son estados de cobertura del catálogo, no transacciones acreditadas.

En esta extensión el productor atómico soporta routers V2 y financiación compatible. CLAMM, CLOB, liquidaciones completas, cross-chain, NFT, intents y otras superficies necesitan sus respectivos productores de evidencia y adaptadores de ejecución. El módulo de liquidaciones entero queda disponible, pero no se presenta como lector de posiciones ni ejecutor terminado. No se incorporó GPU/CUDA ni se desplegaron nodos, contratos o servicios.

Los benchmarks de discovery en `alta_topologia_evidencia` pertenecen a la entrega anterior, son sintéticos y excluyen RPC, simulación y envío. No acreditan un p95 extremo a extremo menor de 30 ms para esta extensión. Tampoco hay medición productiva de ingresos, win rate, Sharpe o throughput.

## Referencias técnicas consultadas

- [EIP-1898](https://eips.ethereum.org/EIPS/eip-1898): lecturas de estado por hash y comprobación de canonicalidad.
- [Configuración de cadenas de Geth](https://github.com/ethereum/go-ethereum/blob/master/params/config.go): calendario de forks y parámetros BPO.
- [PercentageMath de Aave V3 Core](https://github.com/aave/aave-v3-core/blob/master/contracts/protocol/libraries/math/PercentageMath.sol): redondeo de porcentajes.

Los resultados de validación de esta revisión se entregan en `CONTINUACION_VALIDACION.json`, junto con sus logs. El informe anterior se conserva como evidencia histórica.

## Repetir la validación local

Desde `backend`, con la versión Rust fijada en `rust-toolchain.toml`:

```bash
cargo check --locked --workspace --all-targets
cargo test --locked -p shared-rs -p math-engine -p searcher-rs -p prioritization-spine -p sim-core -p simulator-v2 -p relays-client --lib --bin relays-client
```

Desde la raíz, después de instalar las dependencias fijadas:

```bash
npm run build --workspace @arbx/shared
npm run typecheck
npm run test --workspace @arbx/frontend
npm run build
```

El build requiere las URLs públicas correctas del entorno. La comprobación local de esta entrega utilizó dominios `.invalid` únicamente durante el build; esos valores no son configuración de un despliegue. Los archivos generados y las dependencias descargadas no forman parte del ZIP de código.

Las pruebas de red externa expresamente ignoradas siguen pendientes: tres pruebas de Redis y una de simulación privada de staging. Las pruebas unitarias no acreditan una transacción rentable en una red real.

## Resultados de esta revisión

| Comprobación | Resultado |
|---|---:|
| Rust · math_engine | 124 aprobadas; 0 ignoradas |
| Rust · prioritization_spine | 123 aprobadas; 0 ignoradas |
| Rust · relays_client | 91 aprobadas; 1 ignoradas |
| Rust · searcher_rs | 1189 aprobadas; 3 ignoradas |
| Rust · shared_rs | 231 aprobadas; 0 ignoradas |
| Rust · sim_core | 73 aprobadas; 0 ignoradas |
| Rust · simulator_v2 | 26 aprobadas; 0 ignoradas |
| Rust · workspace y todos los targets | PASS |
| TypeScript · todos los workspaces | PASS |
| Frontend · 119 archivos de pruebas | 1.074 aprobadas |
| Build · servicios TypeScript y frontend | PASS, con advertencias heredadas de dependencias/lint |

No se ejecutaron las integraciones externas ignoradas ni se enviaron transacciones. El p95 sintético de la entrega anterior se conserva como histórico, sin atribuirlo a esta revisión.
