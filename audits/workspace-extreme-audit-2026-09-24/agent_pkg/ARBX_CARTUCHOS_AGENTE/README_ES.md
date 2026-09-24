# ArbitrageX — Cartuchos Agente v4

## Entrega

**264 cartuchos Rhai nuevos, uno por ID numerado, con 264 manifiestos completos.** Conservan las capas A/B/C/D, las misiones, clases, ecuaciones y requisitos de ambos Excel. Los siete scripts raíz existentes NO se sustituyen ni se eliminan. El censo original sigue siendo 264 numerados + 7 raíz = 271 archivos, no 270.

Se entrega código, no únicamente una auditoría: generación determinista; búsqueda nativa acotada de ciclos; cotización CPMM entera; adaptador de cotizaciones exactas por protocolo; valoración monetaria decimal; contrato de decisión y de plan inmutable; adaptador a los operadores Rust existentes; DTO frontend; staging seguro y pruebas.

**Estado: generado y verificado parcialmente en este entorno. No compilado con Rhai/Rust, no integrado en el pipeline activo, no desplegado y no certificado en el dominio vivo.** No es un reemplazo directo para el parser v3 ni una implementación completa de todos los feeds/solvers de 60 familias. Véanse `VALIDACION.md` e `INTEGRACION.md`.

## Fuentes conservadas

- `ArbitrageX_264_Cartridge_Math_Architecture(1).xlsx`: 6 hojas.
- `ArbitrageX_Master_264x31_LIVE_First(2).xlsx`: 11 hojas.
- 17 instancias de hoja; 276.225 celdas no vacías; 24.552 fórmulas originales preservadas por separado.
- 264 identidades; 60 familias detectoras; 8.184 relaciones estrategia–operador.
- Pesos `Resolved_Weight`: 8.184 sin calibrar. No se inventa confianza ponderada.
- Copias originales de ambos Excel, exportaciones JSON, fórmulas y hashes en `sources/`.
- Código de referencia contrastado con el ZIP aportado. `main` consultado: `2245eeb325ab7a3d0e40f09cfeca357369975d1a`. NO se afirma que todo el ZIP sea idéntico a ese commit.

**Preservar el 100% de las celdas fuente no significa haber implementado el 100% de las dependencias externas de sus detectores.** La cobertura de código, interfaces y validación está separada en los informes.

## Estructura

- `generated/cartridges/strategies/`: los 264 `.rhai` con sus nombres originales.
- `generated/manifests/`: identidad, misión, ecuación, dependencias, riesgos, modos, 31 filas de operadores, referencias por celda y conflictos de cada ID.
- `runtime/agent_template.rhai.in`: estructura común; las capas A/B se materializan por ID.
- `runtime/agent_graph.rs`: enumeración acotada y quotes V2 U256/U512; ledger y métricas racionales por hop; caché exacta para otros protocolos, nunca sustitución CPMM.
- `runtime/rhai_agent_bridge.rs`: las siete funciones nativas usadas por los scripts; suma exacta de flujos, costes y validación de recibos.
- `runtime/snapshot_services.rs`: implementación concreta de `AgentServices` sobre snapshots inmutables del backend; selección ruta × tamaños suministrados por el optimizador; planes de dominio y payloads canónicos.
- `runtime/context_router.rs`: registro por contexto para instalar bindings una vez sin congelar el motor en un único snapshot.
- `runtime/native_operator_adapter.rs`: invoca el registro de `math-engine` existente y consulta switches; no reimplementa fórmulas ni modifica pesos.
- `runtime/proposal_contract.rs` y `runtime/card_contract.ts`: lectura sin pérdida del resultado v4.
- `runtime/reference_engine.py`: modelo independiente ejecutable para verificación; no reemplaza al runtime Rust.
- `tools/`: construcción, generación, comprobaciones y reportes.
- `tests/`, `validation/`: pruebas y resultados con su alcance.
- `reports/COBERTURA_264.csv`, `ADAPTADORES_60.csv`, `OPERADORES_8184.csv`: mapas para revisar en Excel.
- `INDEX.html`: índice navegable local.

## Semántica operativa

El scanner entrega un contexto e ingredientes. El cartucho selecciona una propuesta entre rutas admisibles. La búsqueda nativa no presupone que Dijkstra resuelva objetivos no lineales dependientes del tamaño. Examina ciclos dirigidos simples, evita reutilizar una pool sin simulación de estado y declara los presupuestos agotados.

`output_raw[i]` alimenta exactamente `input_raw[i+1]`. Los importes del token son strings enteros; los USD son strings decimales. No se usan doubles para capital o beneficio. El resultado está ligado a `context_id`, `plan_hash`, `snapshot_id`, `price_revision`, `policy_revision` y `amount_in_raw`.

Las comisiones y el impacto ya embebidos en el quote NO se vuelven a descontar. Gas, financiación y otros costes externos solo se descuentan una vez; los no aplicables exigen razón y procedencia. Las métricas CPMM de comisión/impacto se expresan como racionales exactos, identificando su base, no como una resta inventada entre tokens distintos.

Se conserva un neto negativo o exactamente cero. Si falla un hop posterior, queda el ledger del prefijo calculado, sin valorarlo como salida final de toda la ruta. Si falta un coste aplicable, no se declara neto completo. Cada fallo de dependencia se conserva como diagnóstico; no se disfraza de dato no aplicable.

`CANDIDATE` NO significa `Sim PASS`, `APPROVE LIVE` ni ganancia realizada. El payload solo puede provenir del codificador/simulador canónico para el mismo plan, monto y revisiones. No hay signer, broadcast ni cambio a PAPER obligatorio en este paquete. Las autorizaciones existentes continúan siendo la autoridad final para LIVE/TESTNET/PAPER.

## Uso local

Los exports corresponden a las versiones SHA256 incluidas. Si cambias un Excel, el generador se detiene y exige reimportarlo; no utiliza silenciosamente una exportación antigua. No modifica los libros.


```bash
python tools/build_spec.py
python tools/generate.py
python tools/structural_checks.py
python -m unittest discover -s tests -p "test_*.py" -v
```

Validación nativa pendiente, en un equipo con Rust y dependencias:

```bash
cargo test --manifest-path validation/rust_harness/Cargo.toml
```

El harness no incluye `native_operator_adapter.rs`, porque ese archivo necesita el `math-engine` del workspace real. Debe compilarse también allí, como parte del cambio integrado.

## Incorporación sin sobrescrituras

```bash
python stage_update.py --repo "/RUTA/arbitragex-v2"
python stage_update.py --repo "/RUTA/arbitragex-v2" --apply
```

Esto copia a `integration/agent-cartridges-v4/`, **fuera del directorio que el cargador activo escanea**. Si un archivo de destino difiere, se detiene; no lo sobrescribe. No modifica `.github`, `.env`, Docker, Cargo/lockfiles existentes, modos, límites ni configuraciones de GitHub. No crea commits ni ejecuta despliegues.

**No copies estos scripts sobre los cartuchos activos sin incorporar el parser/bindings v4 y validar sus productores.** El parser anterior convierte ausencia en `0.0`; además, el adaptador anterior podía reconstruir la ruta desde `intent.legs`. Ninguno de esos comportamientos es compatible con este contrato.
