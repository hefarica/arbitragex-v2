---
name: arbx-live-engineering
description: Ingeniería integral de ArbitrageX para testnet live y mainnet live, evolución de .claude, agregación de blockchains/DEX/pools, mapeo VPS read-only antes de cambios y pruebas reales mediante Desktop Commander y herramientas verificadas.
disable-model-invocation: true
argument-hint: "[mapear | implementar | probar | preparar-testnet | preparar-mainnet] [alcance]"
---

# ArbitrageX — Ingeniería live con evidencia

## 1. Mandato y objetivo de salida

Construye, integra y verifica la DApp existente para soportar **testnet live y mainnet live**.
No sustituyas ese objetivo por un producto permanentemente paper-only, una maqueta,
un segundo frontend ni un informe sin implementación cuando el trabajo está autorizado.

Cada obstáculo técnico se convierte en reproducción, causa, corrección y nueva prueba.
Persiste mientras exista un siguiente paso autorizado y útil; no repitas intentos idénticos
sin nueva evidencia. Un bloqueo de acceso o una condición insegura limita esa acción,
no elimina el objetivo de ingeniería ni impide avanzar en los trabajos independientes.

Esta skill define un encargo y un procedimiento. No crea credenciales, no concede
permisos del sistema operativo, no sustituye una conexión MCP/SSH y no prevalece sobre
instrucciones superiores ni permisos administrados. No simules facultades inexistentes.
La firma, el envío de transacciones con valor y la habilitación de un bot que opere
fondos reales quedan en manos del operador. Entrega el software y el flujo de activación
para que el operador pueda utilizarlos; no impongas una prohibición permanente a mainnet.

Argumentos del encargo actual: $ARGUMENTS

## 2. Contexto de partida que debes verificar

- Repositorio esperado: `hefarica/arbitragex-v2`.
- Alias SSH esperado: `arbx`; ruta remota esperada: `/opt/arbitragex-v2`.
- Integración previa: PR #560. Consulta su estado y SHA actuales; no fijes un HEAD histórico.
- Arquitectura de referencia: Rust, API TypeScript, Edge, Next.js, PostgreSQL y Redis.
- Los informes de septiembre de 2026 son antecedentes, nunca telemetría actual.

Lee [procedencia y fuentes](references/procedencia-y-fuentes.md) al usar estos antecedentes.
Valida el dispositivo, el repositorio y el destino antes de cualquier acción.

## 3. Facultades de ingeniería sobre el proyecto

Dentro del repositorio identificado y los permisos disponibles puedes leer, crear,
corregir y refactorizar código, contratos, pruebas, configuración de desarrollo,
migraciones, contenedores, CI/CD y documentación relacionados con el objetivo.
Puedes modificar `CLAUDE.md`, `.claude/CLAUDE.md`, `.claude/rules/`, `.claude/agents/`,
`.claude/commands/` y `.claude/skills/` para mantener instrucciones coherentes.

Para `.claude/settings.json`, hooks y configuración MCP del proyecto: inspecciona
primero, conserva las entradas ajenas y separa un cambio funcional de un cambio de
permisos. Obtener permisos adicionales requiere el mecanismo real del cliente y la
aprobación correspondiente; no edites las barreras para autoautorizarte.

La política funcional del producto puede evolucionar de “solo pruebas/paper” a
“testnet/mainnet soportadas con activación explícita”, con diff y pruebas de regresión.
Documenta cuál decisión del operador resuelve la contradicción; no borres la historia.

No vacíes `.claude`, no reemplaces instrucciones enteras por una declaración de poder,
no desactives confirmaciones globales ni controles administrados, no retires pruebas
para producir un verde y no alteres esta skill o su validador para fingir cumplimiento.
No toques el `.claude` global del usuario ni otros proyectos por extensión del encargo.

Preserva cambios locales, incluidos untracked. Antes de sobrescribir, crea una copia
local recuperable o commit acotado; evita incluir secretos en esa copia o en Git.

## 4. Máquina de estados obligatoria

| Estado | Trabajo permitido | Evidencia para avanzar |
|---|---|---|
| ACCESS_CHECK | Identificar herramientas, máquina, repo y destino | Acceso autenticado real, identidad y alcance |
| VPS_READ_ONLY | Inventariar y consultar estado sin mutaciones intencionales | Mapa completo del alcance operativo |
| VPS_MAPPED | Formular plan de estabilización, riesgos y reversión | Mapa revisado, acciones y autorización aplicable |
| CHANGE_AUTHORIZED | Implementar únicamente el plan acotado | Verificaciones por acción y rollback disponible |
| ENGINEERING_VERIFIED | Integración, builds, pruebas y despliegue técnico | Evidencia del SHA y entorno realmente probados |
| TESTNET_LIVE_VERIFIED | Pruebas reales en red de pruebas sin valor económico | Identidad de red, recibos y trazabilidad |
| MAINNET_RELEASE_READY | Paquete mainnet y flujo de activación para el operador | Capacidad técnica comprobada, límites y pendientes explícitos |

`MAINNET_RELEASE_READY` no equivale a `MAINNET_ACTIVE`. No presentes un estado futuro
como observado. La activación financiera del operador se registra solamente si existe
evidencia real, sin ejecutarla por delegación automática del agente.

El código local y las pruebas aisladas pueden avanzar mientras se completa el mapa.
Eso no autoriza escribir en el VPS ni tratar un entorno de producción como sandbox.

## 5. Acceso mediante Desktop Commander

Consulta [acceso y Desktop Commander](references/acceso-y-desktop-commander.md).
El operador inicia el dispositivo remoto en el equipo que ya tiene SSH autorizado:

```bash
npx @wonderwhy-er/desktop-commander@latest remote
```

Completa la autenticación interactiva del proveedor, conecta el cliente MCP y descubre
las herramientas realmente disponibles. Registra la versión resuelta; fija esa versión
para repetir pruebas. `latest` es bootstrap solicitado, no una versión reproducible.

No ejecutes este bootstrap en el VPS durante `VPS_READ_ONLY`. No inventes una sesión
SSH porque exista el nombre `arbx`. No solicites claves privadas en el chat.

Los permisos de carpetas de Desktop Commander no constituyen una jaula para su
terminal. Una cuenta SSH restringida o un mecanismo de lectura ya provisionado por
el administrador puede imponer la frontera real. La skill no lo instala durante el mapa.

## 6. VPS: lectura primero, sin excepciones tácitas

Hasta cerrar el mapa solo se permiten consultas identificadas y acotadas. Quedan fuera:
instalar software, escribir temporales remotos, `touch`, cambiar permisos, limpiar,
reiniciar, recargar servicios, alterar Redis/SQL, migrar, hacer pull/fetch remoto,
construir imágenes, desplegar o modificar cron, túneles, firewall, SSH y credenciales.
No ejecutes scripts desconocidos “para ver qué hacen”.

Los artefactos del diagnóstico se guardan en la estación de trabajo autorizada,
fuera de datos sensibles y sin escribir archivos en el VPS. Filtra campos en origen:
**un secreto no debe llegar a la salida de la herramienta para redactarlo después**.

Lectura significa ausencia de mutaciones intencionales de aplicación/configuración.
SSH, HTTP y el sistema pueden registrar accesos o actualizar contadores/atime;
no prometas “cero bits cambiados”. Un endpoint GET o un comando llamado `--check`
no es automáticamente inocuo: comprueba su implementación antes de usarlo.

Procedimiento y dominios: [VPS read-only y mapa](references/vps-readonly-y-mapa.md).

## 7. Qué significa mapear el 100%

Significa cubrir el **100% del inventario operativo definido**, con denominador,
activos enumerados, dependencias y evidencias actuales. No significa leer secretos,
cada byte del disco ni garantizar que no existe un activo desconocido.

Enumera los 12 dominios del manifiesto; concilia sus fuentes de inventario, termina
la paginación y amplía la lista cada vez que aparezca un activo nuevo. No marques
completa una salida truncada. Toda ausencia necesita evidencia, no una suposición.

Un activo averiado puede estar identificado, pero un catálogo inaccesible sigue
siendo un hueco. Si PostgreSQL no permite consultar metadatos requeridos, informa
`BLOCKED` y no certifiques el mapa completo. No lo conviertas en “no aplica”.

Antes del mapa completo informa hallazgos y evidencia faltante; **no propongas ni
implementes un plan de modificación del VPS**. No reduzcas unilateralmente el alcance
para habilitar escrituras. Si avanzar requiere una decisión del operador, delimita
esa decisión y continúa con el trabajo independiente.

Usa `templates/MAPA_VPS.template.json`. El script local `scripts/validate_map.py`
comprueba estructura, cobertura declarada, referencias, antigüedad y hashes de archivos.
Su resultado **no demuestra por sí solo exhaustividad ni autenticidad de los hechos,
no concede permisos y no habilita escrituras ni live**. Revisa la evidencia real.

## 8. Estabilización e implementación después del mapa

Elabora `templates/PLAN_CAMBIO.template.md`: causa comprobada o hipótesis, activo,
acción exacta, prerequisitos, alcance de escritura, impacto, evidencia de backup,
reversión, verificación, presupuesto y criterio de interrupción por acción.

Ejecuta de forma autónoma las acciones reversibles ya autorizadas dentro de ese plan,
sin pedir otra vez lo mismo. Para nuevo alcance destructivo, cambios de acceso,
credenciales, costes o fondos se necesita una autorización específica aplicable.

Mide espacio e inodos, crecimiento, RAM y picos de build/migración/persistencia/rollback.
No uses un número fijo de GB como garantía universal. Revalida condiciones dinámicas
inmediatamente antes del cambio. Coordina el bloqueo compartido con deploy y watchdog.

No borres volúmenes, archivos de PostgreSQL, `pg_wal`, AOF, copias o históricos a ciegas.
No desactives persistencia para que un healthcheck se vea verde. Planifica retención y
recuperación de espacio con evidencia; un `VACUUM` ordinario no garantiza liberar disco
al sistema operativo. Respeta rollups, archivos y la política de conservación aprobada.

## 9. Integración sin reversiones silenciosas

Trabaja sobre la base y el SHA actuales mediante **base + delta**, no reemplazo total.
Contrasta individualmente WO-GAP2, BR-02, BR-03, BR-05, CB-02, BR-06 y WO-07:
`apply_gate_rejection_fields`, frescura/backfill, símbolos bytes32, `PriorsCache`,
`runtime_knobs`, control runtime/heartbeat, propiedades CFMM y `stage2_calibration`.

Busca el código original y sus pruebas en las fuentes autorizadas. Si falta un cambio
sin commit, no afirmes haberlo recuperado desde una rama remota. Una reimplementación
es una reimplementación y debe probar su contrato funcional y su procedencia.

Consulta [integración y validación](references/integracion-y-validacion.md).

## 10. Agregador de blockchains, DEX y pools

Extiende los registros, API, workers y pantallas existentes. Mantén una fuente de
verdad coherente y separa **registrado, descubierto, cotizable, simulable y ejecutable**.
No confundas un catálogo amplio con soporte productivo de todos sus protocolos.

Por red verifica `chainId`, tipo de red, RPC/WSS y estado del bloque; por DEX,
factory/router, bytecode, protocolo y adaptador; por pool, pertenencia a factory,
tokens, decimals, reservas/liquidez y frescura. Usa identidades por cadena y dirección,
no por símbolo. Usa aritmética entera adecuada para importes y economía de ejecución.

Aplica paginación, límites de concurrencia, backpressure, deduplicación y presupuestos
RPC configurables. Registra reorgs, errores, fuentes y marcas temporales sin inventar
datos. Distingue datos ausentes de cero calculado. No aceptes RPC arbitrarios que
permitan acceder a destinos internos no autorizados.

Contrato detallado: [arquitectura live y agregador](references/arquitectura-live-y-agregador.md).

## 11. Testnet live y mainnet live: capacidad real, permisos explícitos

Implementa ambos modos en el producto. No rechaces una red únicamente porque es
mainnet. Exige activación explícita por cadena, configuración válida y evidencia
fresca; no conviertas `chain_id != 1` en sinónimo de testnet.

Aísla configuraciones, wallets, claves, colas y datos de prueba. No reutilices una
credencial de producción en un test ni conviertas tests de UI en órdenes financieras.
Las pruebas testnet con envío requieren una red y activos sin valor económico real.

Revalida cadena, bloque, contrato, firmante, roles, fondos, allowance, calldata,
nonce, financiación, oráculos, slippage, gas y límites antes del envío en el software.
El kill-switch debe verificarse antes del trabajo costoso y nuevamente en el límite
de firma/envío. Prueba también rechazo, RPC caído, datos viejos y reinicios.

Entrega controles de activación utilizables por el operador y el acta
`templates/ACTA_CAPACIDAD_LIVE.template.md`. Preparar mainnet no autoriza al agente a
firmar, enviar operaciones de valor o encender un ejecutor que negocie fondos reales.
La aprobación técnica y la decisión financiera son registros diferentes.

## 12. Pruebas reales mediante herramientas reales

Usa Desktop Commander para dirigir el proceso en el dispositivo verificado; usa
Playwright u otra herramienta de navegador instalada para ejecutar las interacciones.
No llames “probado con Desktop Commander” a escribir un comando sin ejecutarlo.

Lee los scripts y lockfiles antes de elegir comandos. Ejecuta build, typecheck,
lint y pruebas unitarias/integración del alcance afectado, más regresiones del sistema.
Los tests aislados pueden usar fixtures explícitas; nunca sirven como cifras productivas
ni como prueba de que un endpoint mainnet funciona. No inyectes mocks en la DApp real.

Prueba botones, formularios, toggles, filtros, registros de redes/DEX/pools, navegación,
actualización de datos, WebSocket, errores y recuperación. Comprueba efecto persistido,
no solo click o toast. Aísla las mutaciones de UI y los dispositivos de firma reales.

Guarda comandos, SHA, máquina, URL, fechas, exit codes, resultados, trazas y capturas
saneadas. Si no hay conexión, navegador o datos, marca `NOT_RUN` o `BLOCKED`, nunca PASS.

## 13. Entrega y criterio de cierre

Entrega código integrado, diff y commits; inventario y mapa; cambios `.claude`;
plan y bitácora; configuración sin secretos; matriz de soporte; pruebas y evidencias;
identidad de imágenes desplegadas; instrucciones de operación, rollback y activación.

Reporta por separado: implementado, probado localmente, probado en CI, desplegado,
verificado en testnet y preparado para mainnet. Un verde antiguo no valida un SHA nuevo;
un HTTP 200 de frontend no acredita backend, persistencia, WS ni rentabilidad.

No prometas cero fallos o beneficios. No declares “listo para operar” mientras haya
pruebas obligatorias pendientes, fuentes inaccesibles o un componente no conectado.
Declara el alcance exacto que sí completaste y el siguiente requisito concreto.

## 14. Cadencia de trabajo

Al iniciar, identifica objetivo, permisos y siguiente verificación. Durante tareas
largas comunica avances breves con resultados, no operaciones de bajo nivel repetidas.
No prometas trabajo en segundo plano: conserva estado y termina cada ejecución con
el resultado obtenido y sus límites. Una tarea no termina en un plan si aún puedes
implementar y verificar trabajo autorizado dentro de la sesión.
