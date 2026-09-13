# Completar el manifiesto sin fabricar evidencia

Copia `MAPA_VPS.template.json` a una carpeta LOCAL de evidencias saneadas. Nunca la
rellenes con ejemplos como si fueran lecturas reales. Conserva los 12 dominios y
sus 36 checks mínimos. El JSON no lleva comentarios.

## Campos de cabecera

`schema_version`: 1. `snapshot_id`: ID del snapshot, con letras, dígitos, punto,
guion o guion bajo. `captured_at_utc`: hora real en ISO 8601 con Z o +00:00.

`target.ssh_alias`: alias verificado; `identity`: identificador del host verificado
que no sea un secreto; `project_path`: ruta realmente identificada;
`identity_evidence_id`: ID del archivo que sustenta la identificación.
No copies llaves privadas ni secretos de conexión.

## Evidencia

Cada objeto dentro de `evidence` necesita estos campos (vacíos aquí, NO son evidencia):

```json
{
  "id": "",
  "path": "",
  "sha256": "",
  "captured_at_utc": "",
  "target_identity": ""
}
```

`path` es una ruta POSIX relativa a la carpeta del manifiesto, por ejemplo
`evidence/identidad.txt`; no una URL, ruta absoluta o ruta al VPS. El SHA-256 se calcula
sobre los bytes del archivo ya saneado y guardado. No sobre la salida cruda que
contenga secretos. `target_identity` debe coincidir con la cabecera. Los archivos
permitidos son regulares, no enlaces/junctions, de hasta 8 MiB por archivo.

El archivo debe incluir destino, comando/consulta sin secretos, fecha, código de salida
y resultados necesarios. El validador NO autentica esos contenidos ni detecta todos
los secretos; el recolector y el revisor son responsables de su veracidad y saneado.

## Activos

Cada objeto dentro de `assets` necesita:

```json
{
  "id": "",
  "domain_id": "",
  "label": "",
  "verified": false,
  "depends_on": [],
  "evidence_ids": []
}
```

Usa un ID único y uno de los 12 dominios existentes. `depends_on` contiene IDs de
otros activos, no descripciones libres. El mismo activo tiene una sola fila canónica;
otras relaciones lo referencian. No inventes ciclos ni ocultes dependencias externas.

`verified: true` solo tras revisar la evidencia de identificación, estado y relaciones.
Un activo puede tener un estado averiado observado. Eso no acredita sus metadatos
internos inaccesibles ni que esté apto para recibir cambios.

## Dominios

`enumeration_complete` solo cambia a true al terminar la enumeración y conciliación;
`enumeration_evidence_ids` identifica su respaldo. `asset_ids` debe coincidir con todos
los activos del dominio, sin omitidos ni duplicados.

Cuando genuinamente no exista ningún activo en un dominio, documenta `absence_reason`
y una evidencia que lo pruebe. No uses ausencia para ocultar un acceso denegado.
Si hay activos, `absence_reason` queda null.

Cada check debe tener `status`, `finding` y `evidence_ids`. Durante trabajo usa
`unverified` o `blocked`; solo `verified` permite completitud formal. `finding` explica
lo observado, incluida ausencia o avería. Un estado no evaluado nunca se vuelve PASS
porque se haya escrito una descripción.

El validador permite checks adicionales: también deben estar verificados para cerrar.
No quites obligatorios ni reduzcas el inventario para obtener el resultado deseado.

## Huecos y declaración

`open_gaps` mantiene cada falta de acceso/evidencia. No se cierra un hueco hasta resolverlo.
`mapper_attestation` registra responsabilidad por conciliación, exclusión de secretos
y ausencia de mutaciones remotas intencionales. Son declaraciones, no controles de OS.

## Resultado

Exit 0: estructura y archivos cumplen las comprobaciones del validador.
Exit 1: manifiesto formal incompleto o inconsistente.
Exit 2: no se pudo leer/interpretar el manifiesto.

El script no se conecta al VPS, no ejecuta comandos externos ni escribe archivos.
Incluso con exit 0, la calidad/exhaustividad real del mapa requiere revisión separada.
No es una autorización de intervención ni una prueba de disponibilidad o trading live.
