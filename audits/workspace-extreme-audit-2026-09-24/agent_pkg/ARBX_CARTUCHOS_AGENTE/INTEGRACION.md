# Integración v4 — fronteras concretas y estado

**Este documento no afirma que estos cambios estén desplegados.** El staging es deliberado: activar un resultado v4 con el parser y route builder anteriores reproduciría los ceros y la pérdida de identidad detectados.

## Contrato del cartucho

Se mantienen las funciones contractuales `init_strategy()`, `evaluate_opportunity(ctx)` y `build_payload(opportunity)`. La identidad se construye dentro de funciones, compatible con `Scope::new()` vacío. El resultado v4 usa dinero decimal en strings, cantidades raw en strings y contexto/plan/revisión explícitos. `estimated_profit`/`confidence` v3 quedan nulos, nunca rellenos: el parser v4 es obligatorio.

La estructura compartida no convierte todas las familias en una sola estrategia. Cada archivo materializa sus propios ID, misión, clase, ecuación, nota, restricciones, operadores y recibos de validación. Las curvas, búsqueda de alto coste, optimización y operadores permanecen nativos; Rhai orquesta y selecciona, no reimplementa la EVM ni entrena un modelo por archivo.

## 1. Registrar los bindings y cargar sin cambiar configuración

Archivos entregados para incorporar como módulos del crate `searcher-rs`:

```
rhai_agent_bridge.rs
agent_graph.rs
snapshot_services.rs
context_router.rs
proposal_contract.rs
native_operator_adapter.rs
```

Sus `crate::...` paths asumen módulos en la raíz del crate. El integrador debe declararlos tanto en los targets que los consuman como en sus tests. Las dependencias centrales (`rhai`, `ethers`, `serde`, `serde_json`, `sha2`, `bigdecimal` y `math-engine`) ya aparecen en el workspace de referencia; no se parchea ningún Cargo existente.

En `cartridge/runner.rs`, mantener EXACTAMENTE los límites y el registro de bindings actuales. Registrar adicionalmente `rhai_agent_bridge::register(&mut engine, router.clone())`, donde `router` es un `Arc<ContextRouter>` creado por el propietario del contexto. Compilar los nuevos scripts con el mismo `rhai`/features/limitaciones del runner. No aumentar límites para ocultar un error.

`ContextRouter` evita fijar una instancia global al primer bloque: cada llamada resuelve su `context_id` a un `SnapshotServices` inmutable. El propietario retira snapshots vencidos o afectados por reorg. `with_payload_resolver` permite recuperar del registro canónico los resultados de simulación que lleguen después de elegir la propuesta, sin reescribir el snapshot de cotización. No se permite reemplazar silenciosamente un contexto; un cambio exige nueva identidad/revisión. El límite de contextos se proporciona desde la política de recursos del host.

## 2. Productores reales → SnapshotBundle

`SnapshotServices` es una implementación concreta sin red: trabaja con estado construido y validado por el backend. **No es una API para aceptar JSON arbitrario del navegador.** No deben crearse recibos `PASS` porque un objeto JSON diga `verified=true`.

| Entrada | Productor que debe conectarse | Condición |
|---|---|---|
| `edges` | grafo/registro de pools y cotizadores existentes | Token in/out y pool direccionados; reservas y fees reales; un snapshot/bloque consistente para rutas atómicas. |
| `prices` | exportación única del PriceBus | Dirección+chain, precio USD exacto, revisión, evidencia y vencimiento de la fuente. No lookup ambiguo símbolo/dirección, no stables=$1. |
| `size_schedule_raw` | SizeOptimizer/algoritmo por familia | Conjunto de tamaños positivos propuestos nativamente; límites existentes. No se etiqueta como óptimo continuo una selección discreta. |
| `exact_quotes` | V3 Quoter/ticks, Curve, Balancer, LB, hooks… | Clave incluye dirección, cantidad, bloque, snapshot y versión. No se admite single-tick upper bound como quote exacto. |
| `route_support` | contabilidad de costes + validadores + operadores | Por hash del plan, no únicamente ID de estrategia. |
| `domain_plans` | solver de la familia específica | CEX/libro firme, posiciones de lending, subasta, payoff, NFT, inventario o bridge reales. |
| `canonical_payloads` | encoder, simulador y risk engine existentes | Plan y cantidad idénticos; trace no vacío, gas/costes y neto simulados; revisiones y modo vigentes. |
| `active_revision` | estado de control y revisiones del backend | Debe invalidar cambios de política, toggles, PriceBus y reorg; no un `return true` fijo. |

La curva CPMM incluida utiliza `U256`/`U512` y división entera; valida reservas uint112 y denominador/fee explícitos. Los adaptadores de otros protocolos **no están implementados de nuevo**: el paquete consume resultados exactos de sus productores reales. Si faltan, el cartucho reporta la dependencia; no la sustituye por CPMM.

`required_cost_kinds` debe proceder de la composición real del plan. Para rutas atómicas se exigen al menos gas, financiación y fees de ejecución con tratamiento explícito, incluidos los no aplicables. Incorporar además L1/data fee, propina, bridge, hedge, royalties, custody, fallos/rebalanceos, etc., según la familia. No usar suma de gas por hop si el gas real incluye overhead/repayment/approve fuera de esos hops.

Los USD de principal y salida se recalculan contra los raw amounts, decimals y precios ligados al ledger: una cifra USD positiva que contradiga la salida raw no puede aprobarse. El beneficio mantenido después de repago no descuenta la financiación otra vez.

## 3. Operadores: ejecutar y preservar

`native_operator_adapter::evaluate_declared` utiliza el `OperatorRegistry` actual. El callback `is_disabled` puede apuntar a `operator_toggles::is_disabled` sin escribir sus valores. Debe conectarse mediante `SnapshotServices::with_operator_dispatch`; de lo contrario se leerán únicamente los recibos precalculados del snapshot, que también deben proceder de la ejecución real.

Antes de cada dispatch, `OperatorInputAdmission::validate` comprueba los campos, unidades, frescura y procedencia que ese operador consume. **No está implementado para todos los operadores en este paquete:** es la frontera para el validador de features del proyecto. Es necesario porque varios operadores antiguos tienen defaults que no satisfacen la regla actual. No pasar `HashMap::new()` vacío para hacer funcionar una fórmula que requiere features.

Escalares, vectores y matrices se conservan sin convertir un resultado cero en ausencia. Fallos, operadores apagados y no aplicables tienen estados distintos. Se preserva la fase fuente de cada operador; el coordinador nativo debe respetar las dependencias entre fases antes de validar su entrada. No se suministran pesos entrenados ni calibración ficticia.

## 4. Sustituir la frontera de resultado, no solo los scripts

En el punto donde `runner.rs` recibe el `Map`, reconocer `contract_version = arbx.cartridge.agent/4`, convertir sin pérdida y validar con `ProposalV4::parse`. **No llamar al parser v3 que hace `unwrap_or(0.0)` para estos resultados.**

En `cartridge_boot.rs`/consumidor activo, la propuesta se traduce desde el registro nativo utilizando `(plan_hash, snapshot_id, amount_in_raw)`. No volver a crear las piernas desde el `intent` que disparó el análisis. El `intent` permanece como origen causal, no como sustituto del plan seleccionado.

Si el SizeOptimizer cambia cantidad o ruta después de la propuesta, crear una revisión nueva, cotizar todo el recorrido y volver a simular. No conservar el neto ni el Sim PASS del tamaño anterior. El plan canónico se valida contra allowlists/capital/riesgo/slippage/impacto/control antes de ejecutarse.

## 5. Simulación y payload

El cartucho propone, no transmite. `build_payload` solo puede recuperar bytes producidos por el encoder canónico y asociados a una simulación real del mismo plan, importe, snapshot, revisión de precios/política y modo. `CANONICAL_PLAN_VALIDATED` no reemplaza la autorización del signer/custodia.

El registro de simulación debe entregar `net_profit_usd` después del gas y demás costes aplicables, además de `simulation_passed` y `canonical_risk_passed`. Un éxito EVM con margen bruto positivo puede no ser rentable después de gas. Un trace hash demuestra identidad de evidencia; no demuestra por sí mismo realidad del RPC.

No se emiten calldata placeholders, EXECUTOR ficticio, capital inventado ni se reutiliza una simulación con storage overrides para afirmar autorización LIVE. `build_payload` mantiene `approved_for_execution=false` porque la aprobación pertenece al mecanismo existente, no al cartucho.

## 6. Persistencia → API → frontend

Persistir completo el sobre v4 y su revisión en la estructura JSON existente adecuada, o mediante una migración revisada aparte. Este paquete no ejecuta migraciones. Pasar a Redis/API/WS y al store sin coerción decimal, sin borrar pérdidas, y sin mezclar la economía de una observación con la simulación de otra.

`card_contract.ts` valida y conserva el payload; `agentCardRevision` incluye estrategia, contexto, plan, snapshot, precios, política e importe. El frontend debe mostrar valores del productor; no calcular beneficio nuevamente con un precio distinto. Campos aplicables ausentes son fallas de completitud, no un resultado aceptable. Un campo N/A requiere su propia razón de aplicabilidad.

**No se han cambiado los componentes React existentes, ni conectado PG/Redis/Socket ni implementado una nueva garantía de entrega en vivo.** Los hooks concretos y la igualdad por revisión deben verificarse contra esos componentes al integrar.

## 7. Evidencia de entrega y doble vía requerida antes de certificar

Guardar identificador/revisión, hash del sobre y confirmación por etapa: productor, commit durable, consumidor, API, recepción cliente y aplicación al store. ACK de Redis después del commit; deduplicación idempotente; recuperación de PEL; retención mayor que la ventana de recuperación; reconexión por cursor. PEL vacío o conteos iguales aislados NO prueban entrega de cada evento.

Para toggle: comando → persistido → aplicado en motor → revisión confirmada → UI. No habilitar un check de cumplimiento únicamente por HTTP200 o animación del switch. Estas pruebas de integración/recuperación están PENDIENTES y no se declaran ejecutadas por haber generado el contrato.

## Criterios para activar

1. Compilación y llamada real de los 264 scripts con el runner/config actual.
2. Tests de cada productor/cotizador/solver nativo, entradas reales y replay fijado a bloque.
3. Propiedades por plan: raw continuidad, unidades, costes una vez, misma revisión, no precios frescos con reservas vencidas, y tratamiento correcto de reorg.
4. E2E: seguir el mismo plan hasta la card y el toggle de vuelta al motor; caídas, duplicados, recuperación y actualización sin congelar valores.
5. Branch/commit/CI/merge/deploy usando las protecciones y workflows existentes, nunca saltándose sus gates.

**Estado de esos cinco criterios: no certificados aquí.** El staging y las pruebas locales permiten integrar sobre una base trazable sin afirmar una producción que no se observó.
