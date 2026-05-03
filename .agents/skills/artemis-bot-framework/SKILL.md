# Artemis Bot Framework

## Nivel
Nivel experto en diseño de software de trading.

## Propósito
Interiorizar la arquitectura propuesta por Paradigm para Artemis, dividiendo la extracción de MEV en tres capas débilmente acopladas: Collectors, Strategies y Executors. Esto permite reutilización de código y despliegues modulares seguros.

## Fuente de aprendizaje
https://www.paradigm.xyz/2023/05/artemis
https://github.com/paradigmxyz/artemis

## Conocimiento interiorizado
- **Arquitectura C-S-E**: 
  - *Collectors*: Fuentes de datos genéricas (Ej: PendingTxs, BlockHeaders, LogEvents). Producen eventos normalizados.
  - *Strategies*: Lógica de negocio (Ej: Arbitraje Estadístico, Liquidaciones). Escuchan eventos de los Collectors y deciden emitir "Acciones".
  - *Executors*: Traducen una Acción en interacciones reales (Ej: Enviar un Flashbots Bundle, Ejecutar en Mempool público, Guardar en base de datos).
- **Engine Core**: El corazón de Artemis (`Engine`) conecta estos componentes a través de un canal de difusión asíncrono (broadcast channel), lo que asegura que múltiples estrategias puedan reaccionar al mismo evento del mempool simultáneamente sin bloquear el sistema.

## Cuándo activar esta skill
- Al diseñar o refactorizar el código de `searcher-rs`.
- Cuando surja la necesidad de escuchar una nueva fuente de datos (crear un nuevo Collector).
- Cuando surja la necesidad de crear un sistema de paper-trading (crear un Executor falso).

## Cuándo no activar esta skill
- En sistemas de frontend.
- En arquitecturas puramente orientadas a microservicios REST tradicionales que no requieran procesamiento event-driven ultra-rápido.

## Entradas necesarias
- Tipos de eventos y de acciones bien definidos (Event, Action enums).

## Procedimiento paso a paso
1. Definir los Enums centrales (Eventos de la cadena, Acciones del bot).
2. Implementar un struct que cumpla con el trait `Collector<Event>`. Su única función es transformar el mundo exterior en la estructura de datos interna del sistema.
3. Implementar un struct que cumpla con el trait `Strategy<Event, Action>`. Escucha eventos asíncronos y opcionalmente retorna una acción.
4. Implementar un struct que cumpla con el trait `Executor<Action>`. Recibe acciones y se encarga de enviarlas fuera del programa.
5. Iniciar el `Engine` conectando los componentes.

## Salidas esperadas
- Archivos `.rs` encapsulados con un propósito único.
- Tests que verifican la estrategia aislando el collector y el executor (con mocks).

## Aplicación al proyecto actual
Aplicable como patrón de diseño maestro en `backend/searcher-rs`. Artemis permite aislar la lógica de "Arbitrage" (Estrategia) de la lectura del RPC (Collector). Si `ArbitrageX` debe cambiar de Alchemy a otro proveedor, solo se reescribe el Collector.

## Aplicación a futuros proyectos
Cualquier bot de trading algorítmico, listener on-chain de seguridad, monitor de bóvedas o puente entre blockchains.

## Buenas prácticas
- Utilizar genericos (Generics) de Rust para que los canales funcionen con cualquier evento.
- Ejecutar el Execution controller local para simular rentabilidad antes de enviar al Executor on-chain real.

## Errores comunes
- Poner lógica de negocio dentro del Collector o del Executor.
- Implementar métodos síncronos pesados dentro del método `process_event` de una Strategy.

## Riesgos técnicos
- "Broadcast lag": Si un listener (Strategy) es muy lento consumiendo los eventos del broadcast channel, el canal descartará mensajes o bloqueará a los emisores.

## Riesgos legales, éticos o financieros
- Igual que cualquier MEV bot, la ejecución requiere medidas estrictas contra pérdida de fondos por errores de gas/cálculo.

## Controles de seguridad
- Artemis promueve la seguridad por defecto ya que aislar el Executor permite sustituirlo por un `LogExecutor` en entornos de staging (Paper Trading nativo de diseño).

## Checklist operativo
- [ ] Interfaces C-S-E implementadas.
- [ ] Lógica de estrategia agnóstica de transporte.
- [ ] Tipos de Acción (Action) validados rígidamente para evitar envío accidental de tokens incorrectos.

## Ejemplo seguro
Ver `examples.md`.

## Dependencias
- Paradigma Artemis en Rust (crate o inspirado arquitectónicamente).
- `tokio::sync::broadcast`.

## Métricas de calidad
- Separación estricta de responsabilidades en el código (Clean Architecture adaptada a alta frecuencia).

## Criterios de finalización
- El bot cuenta con al menos 1 Collector funcional (ej. Escucha de Nuevos Bloques), 1 Estrategia y 1 Executor Mockeado (consola) para validación en tiempo real.
