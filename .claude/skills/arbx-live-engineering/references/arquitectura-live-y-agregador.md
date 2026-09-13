# Contrato de producto: agregador y operación live

Este documento define requisitos de ingeniería nuevos de la skill, no afirma que
estén implementados en el repositorio ni activos en el VPS.

## Fuente de verdad e identidad

Reutiliza los registros y servicios actuales después de inspeccionarlos. No crees una
segunda DApp, registros paralelos incompatibles o contadores que no correspondan al
runtime. Usa IDs de red verificables y direcciones normalizadas; el símbolo de token
es presentación, no clave de identidad.

Representa por separado `network_type`, capacidades del adaptador y `execution_state`.
Un RPC mainnet de solo lectura no es ejecución live. Un endpoint configurable no
prueba que la red sea compatible. Mantén estados no ambiguos: UNKNOWN, DISCOVERED,
VERIFIED_READ, QUOTABLE, SIMULATABLE, EXECUTION_CAPABLE y activación observada por operador.
No uses un estado “LIVE” simplemente porque la UI tiene un toggle encendido.

## Blockchains

Comprueba `eth_chainId` o mecanismo equivalente, estado/bloque y frescura en los RPC
configurados. Red de pruebas y red principal tienen registros, límites, colas y wallets
diferenciados. Verifica familia de ejecución, finality/reorg, token nativo y decimals.
No conviertas un ID desconocido en testnet ni habilites todas las cadenas por wildcard.

No aceptes URLs aportadas sin validación de scheme/destino y política anti-SSRF. Evita
redirects que transporten credenciales. La lista de RPC debe referenciar secretos, no
exponerlos en UI/telemetría. Backoff, failover y cuotas no deben mezclar identidades de red.

## DEX y pools

Por DEX registra cadena, protocolo/versión, factories/routers verificadas, bytecode o
identidad equivalente y estado del adaptador. Por pool valida pertenencia a factory,
tokens y sus decimales; registra reservas, tick/liquidez u otra estructura según protocolo,
con bloque y timestamp. No reutilices matemática V2 como sustituto de V3/Curve/Balancer.

Distingue descubrimiento de soporte de ejecución. Un protocolo sin adaptador probado
puede estar registrado, pero no se marca ejecutable. Importación y backfill son acotados,
idempotentes y recuperables; eliminaciones de catálogo son preferiblemente soft-disable
sin pérdida de trazabilidad. Las mutaciones de configuración quedan auditadas.

La UI permite agregar y editar redes/DEX/pools a través del control-plane existente,
validar conexión/identidad, inspeccionar errores y ver el efecto en workers y API.
Una confirmación visual no basta: verifica la escritura, relectura y aplicación en runtime.

## Economía y rutas

Importes y transformaciones económicas de ejecución usan aritmética entera y escalas
explícitas. Los oráculos tienen fuente, quote currency, decimales, bloque y max_age.
El rendimiento neto integra costes reales relevantes: gas, primas, fees y otros costes
verificados. Ausencia de precio, reservas o financiación no se sustituye por cero
para fabricar viabilidad. Simulación exitosa no garantiza rentabilidad futura.

Dedup antes de trabajo costoso, prioridades con backpressure, presupuesto de RPC,
latencia y errores por causa. Un límite de capacidad se informa como tal, no como
rechazo económico. Prueba rutas soportadas y explicita las que todavía no lo están.

## Flujo de ejecución que debe implementar el producto

1. Intento con identidad y modo explícitos; kill-switch temprano.
2. Datos verificados, frescos y coherentes en bloque; cantidades y contrato exactos.
3. Revalidación de roles, wallet, financiación, allowance, nonce, gas y límites.
4. Simulación del calldata exacto contra el estado permitido, sin fabricar privilegios.
5. Comprobación final de kill-switch/expiry/identidad en el límite de firma y envío.
6. Transporte aprobado y política privada cuando corresponda al proyecto; idempotencia
   y prevención de duplicados al perder respuestas.
7. Receipt verificado y finalización según la red; reconciliación de éxito, revert,
   reorg, gas y balance, con diferencias explícitas y reintentos acotados.

Los límites existentes son parte del contrato; no los debilites para que pase un test.
Eliminar una prohibición funcional permanente de mainnet no elimina estas condiciones.

## Testnet live

El agente puede ejecutar pruebas de ingeniería autorizadas en una red de pruebas
identificada con activos sin valor económico. Registra transacción, receipt, chainId,
contrato, bloque y camino completo DApp → backend → red → reconciliación → DApp.
No traslades el éxito a mainnet ni uses activos de valor en esas pruebas.

## Mainnet live

El agente construye y verifica soporte mainnet: configuración por cadena, lectura de
estado real, fork/simulación sin envío, controles de activación, runbooks y entrega.
El despliegue de software en un servidor debe mantenerse separado de encender el
negociador: comprueba que ese despliegue no active implícitamente un ejecutor financiero.

El operador conserva la habilitación de trading con fondos reales, gestión de claves y
firmas/envíos de valor. No produzcas un comando de autoactivación para que el agente
eluda esa frontera. La interfaz debe permitir al operador realizar su decisión de
forma explícita y registrar su resultado; no debe impedir mainnet de manera absoluta.

## Casos mínimos

Prueba chainId equivocado, contrato sin código, factory ajena, decimals no disponibles,
reserva vieja, reorg, cotización inválida, financiación insuficiente, rol ausente,
calldata divergente, nonce duplicado, kill-switch activado durante simulación,
timeout de relay, respuesta perdida, receipt revertido, y persistencia degradada.

Si una auditoría afirma algo técnico que condiciona un cambio, verifica el código y
la fuente primaria. Ejemplo: EIP-140 descarta logs de una ejecución revertida; no
cambies contabilidad para aceptar un supuesto receipt canónico imposible solo porque
un informe lo solicita [S8]. Distingue logs de receipt de trazas de depuración.

Fuentes técnicas citadas: [procedencia y fuentes](procedencia-y-fuentes.md).
