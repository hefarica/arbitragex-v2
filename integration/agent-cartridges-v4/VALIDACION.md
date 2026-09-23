# Validación — alcance comprobado

## Ejecutado en este entorno

- **387 pruebas Python**: 102 del modelo de referencia numérico/contable/grafo, 278 de paridad y estructura (incluyen los 264 cartuchos), 7 del stager.
- **16 pruebas Node/TypeScript** del contrato de card: strings sin pérdida, negativos/cero, revisiones, ausencia y no autoautorización.
- `tsc --strict --target ES2022` del módulo nuevo: correcto.
- Comprobación estructural de **264/264 scripts**: delimitadores, firmas, bindings entregados, identidad, capas y manifiestos.
- Regeneración determinista y hashes de 528 archivos generados (264 scripts + 264 manifiestos).
- Staging completo probado sobre un repositorio temporal de prueba: ejecución aditiva, segunda pasada sin cambios y las 387 pruebas repetidas desde la copia instalada. No es un despliegue en el repositorio del usuario.
- **590 archivos de referencia** comparados byte a byte con el ZIP original; los siete raíz permanecen iguales.
- Fuentes originales conservadas: 17 hojas, 276.225 celdas no vacías, 24.552 fórmulas; SHA256 de ambos XLSX.

Los resultados están en `validation/python-all.log`, `typescript-tests.log`, `typescript-compile.log`, `structural.json` y `FINAL_REPORT.json`. No sumar el chequeo estructural de 264 como 264 ejecuciones extra de la estrategia: forma parte también de las pruebas de conformidad.

## Qué prueban los ejemplos

Los casos económicos y de rutas son **sintéticos e identificados como TEST_ONLY**. Incluyen el contraejemplo donde dos cotizaciones de ida difieren positivamente pero los dos ciclos de ida y vuelta pierden; casos de 2 a 7 hops; dinero >2^53; decimales heterogéneos; gas que vuelve negativo el resultado; comisión ya embebida; coste faltante; incoherencia entre USD y ledger raw; revisión de precios errónea; coste de financiación ya repagado; operador apagado; datos parcialmente calculados.

Estos casos verifican el modelo Python independiente y el contrato TS. **No ejecutan el binario Rust, los nuevos scripts en Rhai, ni un contrato EVM.**

## No ejecutado / no certificado

- Rust no estaba instalado en el entorno; no se ejecutó `cargo check/test/clippy/rustfmt`.
- No se compiló ni invocó ningún cartucho con un intérprete Rhai real en esta entrega.
- `validation/rust_harness` es código de prueba entregado, **no evidencia de que haya pasado**. Compila los módulos y pretende compilar/llamar los 264 scripts con el límite actual y un backend vacío de prueba; los errores de compilación que detecte deben repararse antes de activar.
- `native_operator_adapter.rs` requiere además compilar dentro del workspace real con `math-engine`; no forma parte del harness independiente.
- No hay replay de todos los protocolos, ningún RPC/PriceBus real conectado por este paquete, ninguna simulación REVM/Anvil ni prueba de contratos de ejecución.
- No hay mediciones reales de latencia/p95, cobertura de los 31/32 operadores ejecutados, calibración, CPU, memoria o garantías de entrega.
- No hay cambio de repo remoto, PR, commit, push, GitHub Actions, despliegue ni certificación del dominio vivo.

## Definición de finalización

**Generación terminada; activación operativa no terminada.** El paquete contiene las piezas de código y el inventario para integrar, no una afirmación de que los 264 detectores cuentan ya con todos sus adapters, feeds y solvers reales. No activar con el parser v3 ni con PlanSupport/receipts fabricados para hacer pasar los gates.
