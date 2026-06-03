# Diseño consolidado de la skill ArbitrageX v2 Omni-SSOT

**Autor:** Manus AI  
**Fecha:** 2026-05-24  
**Skill propuesta:** `arbitragex-v2-omni-ssot-operator`

## Propósito

La skill debe convertir el contenido de la cadena de chats compartidos de Manus en una guía operacional reutilizable para auditar, mapear, optimizar y operar el frontend y el backend de **ArbitrageX v2**. Su núcleo combina tres ejes: el mapeo **Omni-SSOT** de dominios dinámicos del frontend, la operación **RPC Fail-Honest / G-RPC-1** para activar el motor en vivo, y la validación de rendimiento, build y despliegue mediante flujos end-to-end.

| Eje | Contenido integrado | Fuente dentro del chat compartido |
|---|---|---|
| Omni-SSOT frontend | 9 dominios dinámicos: DEX, Pools, Assets, Opportunities, Strategies, Wallets, Omega, Apex y WebSocket. Incluye entidades, dependencias, endpoints, flujos de datos y estrategia de caché. | Replay `FMEEl1Hd5sqac8bzF62wNz` y `YPY26LAdlblJNAjXeMMit4`. |
| Auditoría de dependencias | Inventario de páginas, hooks, stores, componentes y librerías; extracción de endpoints, chain IDs, términos dinámicos y uso del store; generación de JSON de entidades y dependencias. | Replay `YPY26LAdlblJNAjXeMMit4`. |
| Optimización frontend | Lazy loading con `React.lazy`, `Suspense`, code splitting, optimización de imágenes/fuentes y verificación con `npx next build`. Componentes pesados: `OpportunitiesClient`, `DexRegistryClient`, `PoolsTab`. | Replay `FMEEl1Hd5sqac8bzF62wNz`. |
| Operación RPC en vivo | Diagnóstico de `RPC_HTTP_1` y `RPC_WS_1`, doctrina R8 Fail-Honest, formato CSV `nombre=url`, despliegue y verificación de logs. | Documento descargado desde `OmFVJGiNPRh8XFOoWOYT05`. |
| Validación VPS | Confirmación de parser RPC de 12 proveedores en `TopologyVaultClient.tsx`, contenedor `arbitragex-v2-frontend-1` listo y respuesta en puerto 5173. | Replay `FMEEl1Hd5sqac8bzF62wNz`. |

## Estructura propuesta de la skill

La skill usará una estructura con divulgación progresiva para que `SKILL.md` permanezca conciso y los detalles técnicos estén en archivos de referencia.

```text
arbitragex-v2-omni-ssot-operator/
├── SKILL.md
└── references/
    ├── omni_ssot_mapping.md
    ├── rpc_fail_honest_g_rpc_1.md
    ├── frontend_performance_phase_8.md
    ├── vps_validation_checklist.md
    └── source_chat_index.md
```

No se requieren scripts obligatorios porque el material obtenido es principalmente procedimental. Sin embargo, la skill incluirá comandos verificables en los archivos de referencia para que otra instancia de Manus pueda ejecutarlos cuando el usuario proporcione acceso al repositorio, VPS o variables reales.

## Activadores de uso

La descripción de la skill debe activarla cuando el usuario solicite cualquiera de estos trabajos: auditar ArbitrageX v2; crear o actualizar un mapa Omni-SSOT; diagnosticar variables RPC para operación en vivo; verificar `searcher-rs`, `paper_mode`, mempool, `RPC_HTTP_1` o `RPC_WS_1`; optimizar frontend Next/React con lazy loading; validar `TopologyVaultClient.tsx`; revisar DEX Registry, Pools, Opportunities, Assets, Strategies, Wallets, Omega, Apex o WebSocket.

## Flujo operacional central

La skill debe indicar que el agente primero debe determinar si el usuario pide auditoría, operación en vivo, optimización de frontend o validación de VPS. Luego debe cargar solo la referencia necesaria. Para auditoría Omni-SSOT, la referencia primaria será `omni_ssot_mapping.md`; para problemas de RPC, `rpc_fail_honest_g_rpc_1.md`; para rendimiento frontend, `frontend_performance_phase_8.md`; y para despliegue/validación, `vps_validation_checklist.md`.

| Tipo de solicitud | Referencia a cargar | Resultado esperado |
|---|---|---|
| “Mapea el SSOT del frontend” | `references/omni_ssot_mapping.md` | Inventario por dominio, dependencias, endpoints, caché y plan de consolidación. |
| “El motor no opera en vivo” | `references/rpc_fail_honest_g_rpc_1.md` | Diagnóstico root cause, formato de variables, plan de inyección y verificación. |
| “Optimiza performance del frontend” | `references/frontend_performance_phase_8.md` | Plan de lazy loading, Suspense, code splitting y build validation. |
| “Valida el VPS / contenedor frontend” | `references/vps_validation_checklist.md` | Checklist de parser RPC, contenedor, puerto, build y logs. |

## Restricciones importantes

La skill no debe inventar rutas, endpoints o comandos no observados. Cuando falten repositorio, VPS, claves RPC o contexto privado, debe pedirlos explícitamente antes de ejecutar cambios. Las URLs de Alchemy, Infura o QuickNode son credenciales sensibles y deben tratarse como secretos. Cualquier operación que modifique VPS, contenedores, repositorios o servicios debe requerir confirmación del usuario si implica escritura, despliegue, reinicio o commit.

## Fuentes usadas

[1]: https://manus.im/share/FMEEl1Hd5sqac8bzF62wNz "Replay Manus: skill arbitragex-v2-omni-ssot-mapping y validación de VPS"
[2]: https://manus.im/share/YPY26LAdlblJNAjXeMMit4 "Replay Manus: auditoría frontend Omni-SSOT"
[3]: https://manus.im/share/OmFVJGiNPRh8XFOoWOYT05 "Replay Manus: diagnóstico RPC_HTTP_1 y RPC_WS_1"
