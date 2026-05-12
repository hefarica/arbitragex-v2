---
name: arbx-rust-searcher-observability
description: Exportar telemetry de searcher-rs de manera segura usando mínima mutación.
---
# arbx-rust-searcher-observability

## Purpose
Abstraer señales de vitalidad e información de telemetría de los motores transaccionales de bajo nivel en Rust, sin perjudicar o comprometer la latencia del pipeline de búsqueda (Bellman-Ford, etc.).

## When to use
Cuando sea indispensable conocer el 'latido' y progreso de oportunidades fallidas o exitosas dentro del Scanner principal en memoria (`searcher-rs`) sin causar sobrecarga síncrona.

## Inputs needed
- Estructura atómica in-memory de `HeartbeatSnapshot` en Rust.
- Canal (channel o queue mpsc) o Redis async connection para descarga (fire-and-forget).

## Files usually touched
- `backend/searcher-rs/src/scanner.rs`
- `backend/searcher-rs/src/engines/*.rs`

## Commands
- Verificación del linter estricto: `cargo clippy -p searcher-rs`
- Compilación de validación: `cargo check -p searcher-rs`
- Auditoría viva de Redis: `ssh arbx "docker exec -i arbitragex-v2-redis-1 redis-cli GET arbx:heartbeat:scanner:1:latest"`

## Safety rules
- Nunca introducir sentencias de Bloqueo I/O de red de base de datos (`await` pesados de Postgres) dentro del hilo crítico sincronizado que evalúa swaps.
- Optar por la delegación asíncrona (threads asíncronos `tokio::spawn` o canales).
- Respetar el Principio de Mínimo Cambio Invasivo: Si el log es derivables desde métricas ya alojadas o BD, declinar la modificación a nivel Rust.

## Verification steps
1. Compilar y validar ausencia de warnings de `clippy`.
2. Lanzar en ambiente simulado / staging shadow y leer el valor de la métrica en la caché de Redis iterativamente.
3. Comparar el log time del `route_decoder` antes y después, garantizando que el delta es insignificante (< milisegundo extra).

## Failure modes
- Enviar cada registro en log y colapsar la tarjeta de red / memoria con miles de peticiones `SET` simultáneas por cada tick del loop.
- Desencadenar crashes y fallas abruptas introduciendo un `unwrap()` no evaluado.

## Golden output
Observar en Redis una llave JSON serializada estructurada (`HeartbeatSnapshot`) cambiando su timestamp eficientemente cada pocos segundos sin degradar el `throughput`.

## Anti-patterns
- Importar conectores de observabilidad síncronos (Grafana, Loki API direct) en el archivo crítico bloqueante para graficar en tiempo real.
- Mutar structs y referencias nucleares de las simulaciones y variables de estado unicamente con propósitos decorativos.

## Example prompt
"Aplica arbx-rust-searcher-observability para determinar si el contador de rejections debe sumarse de forma asíncrona hacia Redis a través de mpsc en lugar de meter I/O bruto."
