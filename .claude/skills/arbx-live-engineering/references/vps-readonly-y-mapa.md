# VPS: mapa operativo read-only antes de proponer modificaciones

## 1. Definir y cerrar el inventario

Un mapa “100%” necesita alcance, identidad y denominador. No se inspecciona contenido
privado ajeno ni se leen secretos para aumentar cobertura. Enumera todas las instancias
operativas de cada dominio y sus dependencias, incluyendo servicios apagados y jobs
capaces de modificar el mismo sistema. Concilia fuentes independientes: por ejemplo,
Compose, Docker y systemd; mounts, volúmenes y filesystem; crontabs y logs de ejecución.

Las listas truncadas, muestras de logs y un `head` sirven para diagnóstico inicial,
pero no prueban la enumeración completa de activos. Termina paginación y búsquedas
acotadas antes de cerrar el dominio. Un error de permisos debe quedar como hueco.

## 2. Dominios mínimos y comprobaciones obligatorias

| ID | Comprobaciones | Activos y evidencia esperada |
|---|---|---|
| identidad_acceso | host_y_huella; usuario_y_alcance; ruta_y_frontera | Equipo origen, destino, huella autorizada, usuario, contexto de permisos |
| recursos | cpu_memoria_swap; carga_y_procesos; limites_y_oom | CPU/RAM/swap/cgroups, carga, procesos por nombre, OOM, límites |
| almacenamiento | filesystems_espacio_inodos; volumenes_y_mounts; crecimiento_y_picos | Todos los FS/mounts/volúmenes, dueños, espacio, inodos, tasas y picos medidos |
| servicios | sistema_y_contenedores; salud_y_reinicios; dependencias_y_procesos | Activos/apagados, imágenes/digests, healthchecks saneados, redes, dependencias |
| datos | catalogos_y_tamanos; persistencia_y_durabilidad; locks_slots_y_archivado | Metadatos PG/Redis y demás stores; tamaños, persistencia, locks, WAL/AOF, slots |
| retencion_backups | jobs_y_politicas; historial_y_crecimiento; copias_y_restauracion | Scripts/cron/timers, permisos, resultados, conservación, copias y evidencia de restore |
| red_seguridad | listeners_y_firewall; rutas_tls_y_tuneles; acceso_y_exposicion | Puertos efectivos, reglas IPv4/IPv6, proxies, TLS, túneles, ACL; sin credenciales |
| despliegue | repo_y_cambios; imagenes_y_revision; ci_locks_y_watchdogs | Worktrees, cambios por nombres, SHA checkout versus imágenes, CI y exclusión mutua |
| configuracion | nombres_y_requisitos; origen_y_precedencia; permisos_y_referencias | Nombres, requerido/opcional/default, origen efectivo, permisos de archivos, referencias |
| observabilidad | metricas_y_logs; alertas_y_silencios; salud_extremo_a_extremo | Endpoints/retención/alertas, errores reales, API/WS/UI; no métricas inventadas |
| blockchain | redes_y_rpc; dex_y_pools; firmantes_y_limites | Redes verificadas, RPC/WSS, factories/pools/adaptadores, referencias a firmantes/límites |
| dependencias | grafo_y_propietarios; activos_huerfanos; conciliacion_y_paginacion | Grafo final, responsable técnico, servicios huérfanos, fuentes y páginas conciliadas |

Cada activo lleva ID estable dentro del snapshot, dominio, referencia de evidencia y
lista `depends_on`. Usa IDs opacos en un informe compartible cuando nombres revelen
información innecesaria. Relaciona el grafo sin incluir valores de secretos.

## 3. Comandos y acciones durante lectura

Consultas candidatas, que debes adaptar a permisos y versión: `date`, `uname`, `id`,
`df`, `findmnt`, `free`, `uptime`, `lscpu`, `docker ps`, `docker system df`,
`docker stats --no-stream`, formatos selectivos de `docker inspect`, inventario
systemd/cron y nombres de archivos; ninguna implica permiso para consultar secretos.

Los recorridos `du`/`find` se ejecutan una vez por alcance, con límites de profundidad,
tiempo y carga; no hagas full scans repetidos en un servidor degradado. No sumes
mounts overlay como si fueran discos distintos. No escribas temporales en `/tmp`.

No uses `git status` con efectos opcionales de escritura del índice sin revisarlo;
para lectura usa `GIT_OPTIONAL_LOCKS=0` y desactiva integrations/hooks de monitorización
no revisados. No hagas `git fetch`, pull, checkout ni reset en el VPS durante esta fase.

No ejecutes un `--dry-run` de retención hasta leerlo: un nombre no garantiza que no
cree archivos, materialice rollups o cambie estadísticas. No lo incluyas automáticamente.

Las lecturas HTTP permitidas deben ser endpoints de salud o consulta con semántica
inspeccionada. Algunos GET activan backfills o caches: se consideran mutación y esperan
la fase de cambio autorizada. No hagas load tests contra producción durante el mapa.

## 4. PostgreSQL y Redis

Usa una identidad de base de datos de lectura ya autorizada. Para consultas de
metadatos, aplica transacción READ ONLY y límites de conexión/lock/statement; `psql -X`
evita cargar psqlrc y `ON_ERROR_STOP=1` hace visible el error [S6]. Selecciona columnas
concretas. No incluyas textos SQL de negocio de pg_stat_activity: pueden contener secretos.

Solo lectura no equivale a “cualquier SELECT”: no llames funciones con efectos externos,
extensiones o consultas que revelen datos privados. No crees tablas temporales para
probar escritura. No lances CHECKPOINT, VACUUM, ANALYZE, DDL, DELETE o cambio de roles.

Si PG no arranca, recoge estado/tamaños físicos autorizados, registra SQL inaccesible y
mantén esos checks `blocked`. No mapees OIDs a nombres por memoria o adivinación.

Redis: limita consultas a metadatos como versión y `INFO persistence`. No extraigas
claves o valores de negocio ni ejecutes SET, CONFIG SET, SAVE/BGSAVE o reescritura AOF.
Un PING/healthcheck sano no prueba que AOF se esté escribiendo correctamente.

## 5. Estados de evidencia y antigüedad

Distingue **hecho observado**, **hipótesis**, **no disponible** y **no probado**. Para
cada hecho registra comando/consulta, código de salida, destino y hora UTC. Una
conclusión no reemplaza la evidencia primaria.

El manifiesto adjunto verifica metadatos con una ventana de una hora, una política
inicial de esta skill, no una garantía de vigencia. La salud y el espacio se releen
inmediatamente antes de cambios. No cambies fechas, hashes o la ventana para pasar.
Si cambia el inventario, invalida y reconstruye los dominios afectados.

Mapear backups implica establecer si hay evidencia de restauración y su antigüedad,
no ejecutar un restore en producción durante lectura. “No hay evidencia de restore”
puede ser un hallazgo observado, pero no autoriza una intervención que lo requiera.

## 6. No convertir hipótesis en causa definitiva

Un contador de reinicios menor no identifica a quién actuó; el mtime de un lock no
prueba que esté tomado; un systemd oneshot `inactive` no prueba que no existan reglas
de firewall. Inspecciona configuración efectiva y evidencia causal, no solo etiquetas.

La saturación de un cap de purga exige comparar ingreso, borrado y espacio reutilizado.
No atribuyas todos los GB de un datadir a una tabla sin consultar sus metadatos.
`VACUUM` ordinario permite reutilizar espacio; `VACUUM FULL` reescribe y necesita
espacio adicional [S7]. No extrapoles “VACUUM falló” a una causa única demostrada.

## 7. Cierre del mapa y autorización separada

Conserva evidencia saneada solo en la estación de trabajo y enlázala desde el manifiesto.
Ejecuta el validador local y revisa manualmente la conciliación y la calidad del mapa.
Su PASS verifica estructura y archivos: el agente todavía debe comprobar los hechos.

**Solo después** presenta el plan de intervención con acciones identificadas. No
existe ascenso automático de privilegios ni ejecución de planes por el validador.
Una aprobación debe corresponder al plan concreto y sus efectos, no a “todo vale”.
No vuelvas a pedir autorización para acciones cuyo alcance ya está aprobado y vigente.

Fuentes S6–S7: [procedencia y fuentes](procedencia-y-fuentes.md).
