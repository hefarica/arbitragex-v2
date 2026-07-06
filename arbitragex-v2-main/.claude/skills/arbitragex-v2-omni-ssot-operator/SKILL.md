---
name: arbitragex-v2-omni-ssot-operator
description: "Auditoría y operación técnica de ArbitrageX v2. Use for: crear o actualizar mapas Omni-SSOT del frontend, diagnosticar operación en vivo, revisar RPC_HTTP_1/RPC_WS_1, aplicar disciplina RPC Fail-Honest/G-RPC-1, optimizar frontend Next/React, validar TopologyVaultClient, DEX Registry, Pools, Opportunities, Assets, Strategies, Wallets, Omega, Apex, WebSocket, contenedores Docker y despliegues VPS."
---

# ArbitrageX v2 Omni-SSOT Operator

Usar esta skill para convertir una solicitud sobre **ArbitrageX v2** en un procedimiento técnico ejecutable, trazable y seguro. La skill integra el contenido de los chats compartidos de Manus sobre mapeo **Omni-SSOT**, auditoría frontend, optimización fase 8, validación de VPS y diagnóstico **RPC Fail-Honest / G-RPC-1**.

## Flujo de decisión inicial

Primero clasificar la solicitud del usuario. Si el usuario pide mapear, auditar o consolidar el frontend, cargar `references/omni_ssot_mapping.md`. Si el usuario indica que el motor no opera en vivo, que `searcher-rs` está en `paper_mode`, o menciona `RPC_HTTP_1`, `RPC_WS_1`, mempool, Alchemy, Infura o QuickNode, cargar `references/rpc_fail_honest_g_rpc_1.md`. Si el usuario pide optimizar performance, bundle, lazy loading o build de frontend, cargar `references/frontend_performance_phase_8.md`. Si el usuario pide validar el VPS, contenedor, puerto 5173, Docker o parser RPC en producción, cargar `references/vps_validation_checklist.md`.

| Solicitud del usuario | Referencia principal | Entregable esperado |
|---|---|---|
| Mapa SSOT, dependencias, endpoints o dominios dinámicos | `omni_ssot_mapping.md` | Inventario por dominio, entidades, dependencias, flujos y plan de consolidación. |
| Motor inactivo, `paper_mode`, RPC, mempool u operación en vivo | `rpc_fail_honest_g_rpc_1.md` | Diagnóstico root cause, formato de variables, plan de inyección y verificación. |
| Optimización frontend, lazy loading, Suspense o build | `frontend_performance_phase_8.md` | Plan de code splitting y validación con build. |
| Validación de VPS, Docker, puerto 5173 o TopologyVault | `vps_validation_checklist.md` | Checklist de despliegue, logs, parser RPC y verificación funcional. |

## Reglas de ejecución

No inventar rutas, comandos, endpoints ni resultados no observados. Usar los comandos de referencia como hipótesis verificables y confirmarlos contra el repositorio o el VPS antes de hacer cambios. Cuando falte acceso al repositorio, al VPS, a logs o a variables reales, pedirlos de forma explícita y mínima.

Tratar las URLs RPC de Alchemy, Infura, QuickNode u otros proveedores como **secretos**. No imprimir claves completas en respuestas finales, logs compartidos o documentos de usuario. Si se requiere modificar `.env`, reiniciar contenedores, ejecutar despliegues, hacer commits o tocar infraestructura en vivo, solicitar confirmación antes de la operación de escritura.

## Procedimiento base

1. Identificar el modo de trabajo: auditoría, operación RPC, optimización frontend o validación VPS.
2. Cargar solo la referencia necesaria para preservar contexto.
3. Inspeccionar el estado real del repositorio o servidor antes de proponer cambios definitivos.
4. Producir un diagnóstico con root cause, evidencia, impacto y plan de corrección.
5. Ejecutar cambios solo si el usuario dio acceso y confirmación suficiente.
6. Validar con logs, build, pruebas, endpoints o contenedores, según aplique.
7. Entregar un resumen con archivos tocados, comandos ejecutados, resultados y próximos pasos.

## Modo Omni-SSOT

En auditoría frontend, estructurar el trabajo por dominios: **DEX**, **Pools**, **Assets**, **Opportunities**, **Strategies**, **Wallets**, **Omega**, **Apex** y **WebSocket**. Extraer entidades, endpoints, stores, hooks, componentes, páginas, términos dinámicos, dependencias y política de caché. Si se detectan llamadas duplicadas o estados paralelos, proponer una fuente única de verdad y una ruta de migración gradual.

## Modo RPC Fail-Honest

En operación en vivo, verificar la cadena desde `/opt/arbitragex-v2/.env` hasta Docker y Rust. Confirmar que `RPC_HTTP_1` y `RPC_WS_1` existan, que usen formato CSV `nombre=url`, que `docker/compose.prod.yml` inyecte el `.env`, y que `searcher-rs` emita señales como `http rpc pool initialized` y `filtered mempool subscription active`. Si falta RPC, explicar que la doctrina **R8 Fail-Honest** bloquea workers y puede activar `paper_mode` para evitar corrupción.

## Modo performance frontend

En optimización frontend, priorizar `React.lazy`, `Suspense`, separación de componentes pesados y validación con `npx next build`. Tratar como candidatos principales `OpportunitiesClient`, `DexRegistryClient` y `PoolsTab`, además de módulos de imágenes, fuentes y bundles dinámicos.

## Referencias integradas

La referencia `source_chat_index.md` conserva el índice de replays y documentos utilizados para crear esta skill. Cargarla solo si el usuario pide trazabilidad, procedencia documental o ampliación de la skill.
