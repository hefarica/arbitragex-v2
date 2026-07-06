# Referencia RPC Fail-Honest / G-RPC-1 para ArbitrageX v2

Usar esta referencia cuando el usuario indique que **ArbitrageX v2** no opera en vivo, que el motor `searcher-rs` está inactivo, que el sistema entró en `paper_mode`, que no hay mempool, o que faltan `RPC_HTTP_1` y `RPC_WS_1`.

## Root cause documentado

El diagnóstico del chat fuente identificó que el motor principal `searcher-rs` puede quedar inactivo si no encuentra `RPC_HTTP_1`. Bajo la doctrina **R8 Fail-Honest**, el sistema se niega a iniciar workers de Ethereum Mainnet si faltan variables críticas. Sin workers, puede activar `paper_mode = true` y detener la ingesta para evitar corrupción de datos.

| Elemento | Diagnóstico integrado |
|---|---|
| Worker Ethereum | Requiere `RPC_HTTP_1` para iniciar la cadena 1. |
| Mempool | Requiere `RPC_WS_1` para suscribirse a `alchemy_pendingTransactions`. |
| Archivo host | `/opt/arbitragex-v2/.env`. |
| Docker Compose | `docker/compose.prod.yml` usa `env_file: ["../.env"]`. |
| Rust config | `backend/shared-rs/src/rpc_failover.rs` lee variables RPC. |
| Componentes dependientes | `searcher-rs` y `token-enricher`. |
| Señal de seguridad | `paper_mode = true` puede activarse cuando no hay workers funcionales. |

## Formato G-RPC-1 requerido

La disciplina G-RPC-1 espera formato CSV con pares `nombre=url`. Un solo proveedor puede funcionar, pero el diagnóstico fuente indica que el sistema puede advertir `rpc_pool.single_vendor` y pedir redundancia.

```text
RPC_HTTP_1=alchemy=https://eth-mainnet.g.alchemy.com/v2/<TU_API_KEY>,infura=https://mainnet.infura.io/v3/<TU_API_KEY>
RPC_WS_1=alchemy=wss://eth-mainnet.g.alchemy.com/v2/<TU_API_KEY>,infura=wss://mainnet.infura.io/ws/v3/<TU_API_KEY>
```

## Validación de Docker Compose

Buscar en `docker/compose.prod.yml` o archivo equivalente que los servicios declaren variables obligatorias. El chat fuente mostró este patrón:

```yaml
RPC_WS_1: ${RPC_WS_1:?RPC_WS_1 required for mainnet detection}
RPC_HTTP_1: ${RPC_HTTP_1:?RPC_HTTP_1 required for mainnet receipts}
```

El operador `:?` hace que Docker falle si la variable está vacía. Si el contenedor no arranca, revisar primero `.env`, interpolación de Compose y nombres reales de servicios.

## Plan de implementación end-to-end

Ejecutar solo con acceso autorizado al VPS y después de recibir las URLs reales del proveedor RPC. No pedir claves completas si el usuario prefiere pegarlas manualmente en el navegador o terminal. Si el agente va a modificar el VPS, solicitar confirmación antes de escribir.

### Paso 1: Inyección en `.env`

```bash
ssh root@195.201.235.70
nano /opt/arbitragex-v2/.env
```

Reemplazar o crear las líneas `RPC_HTTP_1=` y `RPC_WS_1=` usando el formato `nombre=url`. No registrar claves completas en la respuesta final.

### Paso 2: Reinicio seguro

```bash
cd /opt/arbitragex-v2
./infra/vps/deploy.sh prod
```

El documento fuente indica que este script realiza `docker compose pull` y `up -d --remove-orphans`, inyectando el `.env` actualizado en `searcher-rs` y `token-enricher`.

### Paso 3: Verificación de ingesta

```bash
# Verificar que el pool RPC se inicializó correctamente
docker logs arbitragex-v2-searcher-rs-1 | grep "http rpc pool initialized"

# Verificar que la suscripción al mempool está activa
docker logs arbitragex-v2-searcher-rs-1 | grep "filtered mempool subscription active"

# Verificar que las oportunidades empiezan a llegar al API Server
curl -s http://localhost:8080/api/opportunities/live | grep -q '"count": 0' && echo "Aún vacío" || echo "Oportunidades fluyendo"
```

Si los nombres de contenedor difieren, listar servicios antes de ejecutar logs:

```bash
docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'
```

## Checklist de diagnóstico rápido

| Pregunta | Comando o evidencia |
|---|---|
| ¿Existe `.env` en el host esperado? | `ls -la /opt/arbitragex-v2/.env` |
| ¿Están presentes ambas variables? | `grep -n "^RPC_\(HTTP\|WS\)_1=" /opt/arbitragex-v2/.env` |
| ¿El formato es `nombre=url`? | Revisar que haya proveedor antes del signo `=` interno: `alchemy=https://...`. |
| ¿Compose inyecta el env file correcto? | `grep -RIn "env_file\|RPC_HTTP_1\|RPC_WS_1" /opt/arbitragex-v2/docker /opt/arbitragex-v2/infra 2>/dev/null` |
| ¿Arrancó el pool HTTP? | Log `http rpc pool initialized`. |
| ¿Arrancó la suscripción WS? | Log `filtered mempool subscription active`. |
| ¿Sigue vacío el feed live? | Consultar `/api/opportunities/live` y revisar logs de `searcher-rs`. |

## Reglas de confidencialidad

Las URLs RPC contienen claves o tokens. No escribirlas completas en documentos, issues, commits, chats públicos ni logs finales. Para evidencias, enmascarar con formato `https://.../v2/<redacted>` o mostrar solo proveedor y tipo de protocolo.
