# Fase 0 — Diseño seguro de Topology Vault, perfil staging y hot-reload por Redis Pub/Sub

**Autor:** Manus AI  
**Fecha:** 2026-05-23  
**Rama objetivo:** `feature/topology-vault-rpc-mux`  
**Entorno objetivo:** VPS `arbx-v2-clean`, repositorio `/opt/arbitragex-v2`

## Resumen ejecutivo

La Fase 0 queda definida como una fase de **aislamiento, diseño verificable y preparación de staging**, no como una intervención directa sobre producción. El objetivo es permitir que el frontend administrativo controle cambios de topología RPC/WS, umbrales de riesgo y kill-switches mediante un backend server-side, sin exponer secretos al navegador ni reiniciar contenedores completos para cada ajuste.

La arquitectura correcta es un **plano de control transaccional** compuesto por el frontend, el `api-server`, Vault o un almacén cifrado, Redis Pub/Sub interno y consumidores Rust con hot-swap. Redis Pub/Sub es adecuado como canal efímero de notificación porque publica mensajes a suscriptores conectados en canales internos, mientras que la persistencia autoritativa del estado debe residir fuera del canal, en Vault, base de datos o archivo cifrado versionado.[1]

> El frontend no debe escribir `.env`, reiniciar Docker ni emitir secretos. El frontend debe enviar una intención autenticada al `api-server`; el backend valida, persiste la versión de topología y publica un evento interno para que los motores recarguen estado en memoria.

## Línea base observada

La inspección de Fase 0 confirmó que el VPS tiene contenedores Docker activos y que Redis ya existe como servicio interno saludable. El repositorio se aisló en la rama `feature/topology-vault-rpc-mux` desde el commit base `f5f03462f070108472b91a7606a9670be9dafe93`. No se reinició ningún contenedor durante esta preparación.

| Área | Estado observado | Implicación para la implementación |
|---|---:|---|
| Rama de trabajo | `feature/topology-vault-rpc-mux` | Los cambios pueden prepararse sin tocar `main`. |
| Redis | Contenedor `arbitragex-v2-redis-1` saludable | Se puede usar como bus interno de mutaciones. |
| Docker Compose | `docker/compose.prod.yml` y `docker/compose.dev.yml` válidos | Se puede añadir un override de staging sin alterar producción. |
| `searcher-rs` | Contiene `chain_client.rs`, `scanner.rs`, `rpc_multiplexer.rs`, `config_reload.rs` y `config_reload_omni.rs` | Hay puntos naturales para implementar hot-reload y reconexión WS. |
| Backend API | Contiene rutas admin, credenciales, readiness y trading config | El `Topology Vault` debe implementarse como ruta admin server-side. |
| Frontend | Contiene `/admin/chains`, `/settings/credentials`, `/rpcs`, `/killswitch` y paneles operativos | La UI puede adaptarse sin rediseñar toda la estructura. |

## Contrato de propagación atómica

El contrato debe separar estrictamente **intención**, **persistencia**, **evento** y **aplicación en memoria**. El frontend solo origina una intención administrativa; el `api-server` aplica validación, control de permisos y auditoría; Redis emite un pulso interno; cada consumidor decide si puede aplicar el cambio en caliente o si debe marcarlo como pendiente.

| Capa | Responsabilidad | Prohibición explícita |
|---|---|---|
| Frontend | Mostrar topología, editar formularios, pedir confirmación y enviar intención. | No debe recibir secretos RPC completos ni escribir `.env`. |
| `api-server` | Validar, cifrar/persistir, versionar, auditar y publicar evento. | No debe confiar en valores sin validación ni saltarse RBAC. |
| Redis Pub/Sub | Notificar `TopologyMutationCommitted` en canal interno. | No debe ser fuente autoritativa ni transportar secretos en claro. |
| `searcher-rs` | Recargar snapshots, reconectar WS, reconfigurar multiplexor y reportar ACK/NACK. | No debe bloquear el loop crítico ni dejar dos streams duplicados activos. |
| Observabilidad | Emitir métricas, logs estructurados y auditoría de versión aplicada. | No debe imprimir claves, tokens ni URLs completas. |

## Canal Redis propuesto

El canal primario debe usar un namespace consistente con la gobernanza existente del proyecto: `arbx:topology:mutation`. Para compatibilidad con la nomenclatura solicitada por el operador, puede añadirse alias temporal `arbx_topology_mutation`, pero el canal canónico recomendado es el namespace con prefijo `arbx:`.

```json
{
  "schema_version": "topology.mutation.v1",
  "mutation_id": "uuid-v7-or-ulid",
  "committed_at": "2026-05-23T19:10:00Z",
  "environment": "staging",
  "actor": "admin:<redacted>",
  "scope": ["rpc", "mempool"],
  "topology_version": 7,
  "requires": {
    "reload": true,
    "restart": false
  },
  "secret_refs": {
    "rpc_http_1": "vault://arbx/staging/rpc/http/1",
    "rpc_ws_1": "vault://arbx/staging/rpc/ws/1"
  },
  "public_shape": {
    "chain_id": 1,
    "mempool_mode": "filtered",
    "ws_providers": ["alchemy", "publicnode"],
    "http_providers": ["alchemy", "drpc", "lava"]
  },
  "checksum": "sha256-of-canonical-public-shape"
}
```

El evento no debe incluir URLs completas, API keys ni secretos. Cada motor debe recibir el evento, consultar el snapshot autoritativo por referencia segura y validar que el `topology_version` sea más reciente que el aplicado actualmente. Si el evento se pierde, el servicio debe poder reconciliar estado mediante polling periódico o endpoint de snapshot al arrancar.

## Perfil staging propuesto

Staging debe ejecutarse como perfil aislado, con puertos locales alternativos, `COMPOSE_PROJECT_NAME` distinto, base de datos y Redis lógicamente separados, y `PAPER_MODE` o ejecución sin gasto real activada hasta completar pruebas. Docker Compose permite combinar archivos mediante overlays, lo que encaja con `docker/compose.prod.yml` más un override de staging para puertos, variables y nombres de proyecto.[2]

| Variable | Valor staging recomendado | Justificación |
|---|---|---|
| `ARBX_ENV` | `staging` | Permite aislar logs, métricas y permisos. |
| `ARBX_TOPOLOGY_CHANNEL` | `arbx:topology:mutation` | Canal canónico de hot-reload. |
| `ARBX_TOPOLOGY_SNAPSHOT_URL` | `http://api-server:8080/admin/topology/snapshot` | Fuente autoritativa server-side. |
| `ARBX_MEMPOOL_MODE` | `filtered` solo si el primer WS es Alchemy | Evita intentar `alchemy_pendingTransactions` sobre proveedores incompatibles. |
| `RPC_WS_1` | `alchemy=<wss>,publicnode=<wss>` | Alchemy habilita la ruta filtrada; fallback estándar conserva continuidad. |
| `RPC_HTTP_1` | `alchemy=<https>,drpc=<https>,lava=<https>` | Multiplexor HTTP con failover. |
| `ARBX_EXECUTION_ENABLED` | `false` | Pruebas end-to-end sin gasto ni firma real. |
| `ARBX_HOT_RELOAD_ENABLED` | `true` | Activa consumidores de mutaciones en staging. |

## Implementación mínima correcta

La implementación debe entrar en cuatro incrementos. Primero se crea el contrato y el perfil staging sin activar cambios en producción. Después se implementa el endpoint `Topology Vault` en `api-server` con validadores y publicación Redis. Luego se adapta `searcher-rs` con un `TopologyManager` basado en snapshot atómico, reconexión WS controlada y ACK/NACK. Finalmente se conecta el frontend con pruebas E2E.

| Incremento | Entregable | Validación requerida |
|---|---|---|
| A | `config/topology/staging.env.example`, `docker/compose.staging.override.yml`, ADR de arquitectura | `docker compose config` pasa sin secretos reales. |
| B | Rutas `GET /admin/topology/snapshot`, `POST /admin/topology/mutations`, auditoría y publicación Redis | Tests unitarios de validación y redacción de secretos. |
| C | `TopologyManager` Rust, subscriber Redis y reconexión WS idempotente | Test con proveedor falso: aplica versión 2, descarta versión 1, evita doble stream. |
| D | UI `Topology Vault` y Playwright E2E | Cambiar proveedor en UI produce evento, ACK y estado reflejado sin reiniciar contenedor. |

## Controles de seguridad y rollback

La operación debe mantener tres barreras: aislamiento de rama, staging separado y rollback explícito. Los secretos se validan y guardan server-side; el navegador solo ve nombres de proveedores, estado de salud, checksums y últimos cuatro caracteres si fuera imprescindible. El rollback funcional consiste en republicar la última versión estable o desactivar `ARBX_HOT_RELOAD_ENABLED`; el rollback de rama consiste en volver a `main` o descartar `feature/topology-vault-rpc-mux`.

| Riesgo | Mitigación |
|---|---|
| Evento Redis perdido | Reconciliación por snapshot al arrancar y polling periódico de versión. |
| Reconexión WS duplicada | Swap con generación monotónica y cancelación explícita del task anterior. |
| Proveedor incompatible con `alchemy_pendingTransactions` | Validación: modo `filtered` requiere primer proveedor WS `alchemy`. |
| Exposición accidental de secretos | Redacción obligatoria en logs, API y frontend. |
| Cambio aplicado parcialmente | ACK/NACK por servicio y dashboard de convergencia por `topology_version`. |
| Daño a producción | No usar `docker compose up` sobre prod durante Fase 0; solo staging y archivos en rama. |

## Criterios de salida de Fase 0

La Fase 0 se considera completa cuando existe una rama aislada, backup de `.env`, documentación de contrato, plantilla staging sin secretos, override Compose staging y una lista de próximos comandos que no reinician producción. En esta etapa no se debe exigir que hot-reload funcione todavía; eso pertenece a Fase 1.

## Referencias

[1]: https://redis.io/docs/latest/develop/interact/pubsub/ "Redis Pub/Sub documentation"  
[2]: https://docs.docker.com/compose/how-tos/multiple-compose-files/merge/ "Docker Compose: Merge Compose files"  
[3]: https://www.alchemy.com/docs/reference/alchemy-pendingtransactions "Alchemy pendingTransactions WebSocket subscription"
