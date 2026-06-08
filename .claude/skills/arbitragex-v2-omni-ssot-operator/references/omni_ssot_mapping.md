# Referencia Omni-SSOT Frontend para ArbitrageX v2

Usar esta referencia cuando el usuario pida auditar, mapear o consolidar la fuente única de verdad del frontend de **ArbitrageX v2**. El objetivo es transformar páginas, hooks, stores, componentes, endpoints y eventos WebSocket en un mapa operativo que permita reducir duplicidad, estabilizar datos dinámicos y planificar refactors seguros.

## Dominios dinámicos principales

El chat fuente identificó nueve dominios dinámicos que deben tratarse como unidades de análisis. Cada dominio debe mapear sus entidades, consumidores, productores, endpoints y política de cacheo.

| Dominio | Alcance operativo | Elementos a inspeccionar |
|---|---|---|
| DEX | Registros, adaptadores, chain IDs, liquidez por exchange y estado de conectividad. | `DexRegistryClient`, APIs de registry, parser RPC, fixtures de cadenas. |
| Pools | Pools dinámicos, liquidez, pares, TVL, estado por chain y fuente de datos. | `PoolsTab`, hooks de pools, fetchers, store compartido. |
| Assets | Tokens, metadatos, precios, enrichers y normalización multi-chain. | Token enricher, assets store, componentes de selección y visualización. |
| Opportunities | Oportunidades live, simulación, scoring, estado de ejecución y feed. | `OpportunitiesClient`, endpoint `/api/opportunities/live`, caché y polling. |
| Strategies | Configuración, activación, resultados, backtests y compatibilidad con oportunidades. | Strategy pages, forms, stores, API de estrategia. |
| Wallets | Conectividad, saldos, permisos, redes y estado de sesión. | Wallet hooks, providers, conectores y componentes de cuenta. |
| Omega | Métricas, topología, vaults o vistas avanzadas de sistema. | `TopologyVaultClient.tsx`, componentes de topología y parsing de RPC. |
| Apex | Capas ejecutivas o paneles de control avanzado. | Páginas Apex, formularios y dependencias sobre otros dominios. |
| WebSocket | Ingesta en vivo, mempool, eventos, reconexión y fan-out de datos. | WS clients, suscripciones, stores de eventos y fallback HTTP. |

## Procedimiento de auditoría

Comenzar con una inspección estática del repositorio. Localizar rutas de páginas, componentes `Client`, hooks, stores, servicios API y utilidades de red. Registrar cada hallazgo en una tabla por dominio, no como una lista plana, porque el propósito del Omni-SSOT es revelar dependencias cruzadas.

| Paso | Acción | Evidencia esperada |
|---|---|---|
| Inventario | Buscar páginas, componentes, hooks, stores y servicios relacionados con cada dominio. | Lista de archivos con función probable. |
| Extracción de endpoints | Localizar `fetch`, `axios`, clientes internos, URLs y rutas API. | Tabla endpoint → consumidor → dominio. |
| Extracción de entidades | Identificar tipos, interfaces, schemas, keys y objetos normalizados. | Entidad → origen → consumidores. |
| Estado y caché | Detectar store global, estado local, polling, SWR, React Query o caché manual. | Fuente de verdad candidata y duplicados. |
| Flujo de datos | Reconstruir productor → transporte → store → componente → UI. | Diagrama textual o tabla de flujo. |
| Consolidación | Proponer SSOT canónico y migración incremental. | Plan por fases, riesgos y pruebas. |

## Señales de deuda técnica

Tratar como hallazgos de alta prioridad cualquier endpoint duplicado, normalización divergente de cadenas, parsing repetido de RPC, estados paralelos para oportunidades live, polling sin invalidación, componentes pesados mezclando UI y fetch, o stores que mezclan datos de distintos dominios sin límites claros.

## Entregable recomendado

El resultado debe incluir una matriz por dominio, una tabla de endpoints, un mapa de dependencias, una propuesta de fuente única de verdad y un plan de refactor por fases. Si el usuario solicita implementación, primero producir el plan y luego ejecutar cambios pequeños, verificables y reversibles.

| Sección | Contenido mínimo |
|---|---|
| Resumen ejecutivo | Root cause arquitectónico, riesgos y prioridad. |
| Mapa por dominio | Archivos, endpoints, stores, entidades y consumidores. |
| Matriz de duplicidad | Datos repetidos, fetch redundante y estados paralelos. |
| SSOT propuesto | Módulo canónico, contrato de datos y política de caché. |
| Plan de migración | Cambios incrementales, pruebas y validación de build. |

## Comandos útiles de inspección

Usar estos comandos solo dentro de una copia segura del repositorio y adaptar los paths al proyecto real.

```bash
# Localizar clientes/componentes por dominios principales
grep -RIn "DexRegistryClient\|PoolsTab\|OpportunitiesClient\|TopologyVaultClient" .

# Localizar llamadas de red y rutas API
grep -RIn "fetch(\|axios\|/api/\|WebSocket\|wss://\|ws://" frontend src app components lib backend 2>/dev/null

# Localizar stores, hooks y estado compartido
grep -RIn "use[A-Z].*Store\|create(\|zustand\|redux\|useQuery\|SWR\|useEffect" frontend src app components lib 2>/dev/null

# Localizar chain IDs y variables RPC expuestas accidentalmente
grep -RIn "chainId\|RPC_HTTP\|RPC_WS\|alchemy\|infura\|quicknode" .
```

## Regla de seguridad

No exponer claves RPC ni credenciales si aparecen en `.env`, logs o código. Si se encuentran secretos en el repositorio, detener la auditoría funcional y recomendar rotación de claves, purga del historial si aplica y reemplazo por variables de entorno seguras.
