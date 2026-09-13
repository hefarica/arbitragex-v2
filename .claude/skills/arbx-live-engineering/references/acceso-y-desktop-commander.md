# Acceso real y Desktop Commander

## Fronteras distintas

La autorización funcional del propietario permite trabajar en su proyecto. La cuenta
MCP/SSH, el sistema operativo y los permisos del cliente determinan qué puede ejecutar
realmente una herramienta. Un archivo Markdown no instala ni concede esas capacidades.
Las reglas de permiso se aplican en el cliente [S2].

Desktop Commander proporciona terminal y herramientas de archivos en el equipo
conectado. `allowedDirectories` limita operaciones de archivos, pero no aísla los
comandos de terminal; no se debe presentar como una garantía read-only del VPS [S4].

Para una restricción técnicamente impuesta, el administrador debe haber provisionado
una identidad o pasarela de consulta con privilegio mínimo. Una shell con root o acceso
irrestricto al daemon Docker tiene facultades de escritura aunque el agente prometa
solo leer. Registra esa diferencia en el informe. No intentes probar la frontera
real ejecutando una escritura prohibida; inspecciona permisos/políticas autorizados.

## Arranque en el equipo del operador

Ejecutar en la estación de trabajo donde ya están el repositorio y el acceso SSH,
no en el VPS durante su fase de lectura:

```bash
npx @wonderwhy-er/desktop-commander@latest remote
```

El flujo oficial requiere autenticación en navegador y conexión del cliente al servicio
MCP; el proceso trabaja bajo los permisos del usuario local y se detiene con Ctrl+C [S3].
El destino documentado del servicio es:

```text
https://mcp.desktopcommander.app
```

No reutilices una sesión de otro equipo. Descubre los nombres/esquemas actuales de
las herramientas y realiza primero una consulta inocua de identidad/directorio.
No deduzcas que existe acceso al VPS por ver un terminal local.

Anota Node/npm, versión realmente resuelta de Desktop Commander y versión del cliente.
Después del bootstrap, para reproducibilidad sustituye `latest` por la versión exacta
observada y revisada. No escribas un número supuesto ni instales una actualización
diferente a mitad de una campaña de pruebas.

## SSH: cadena de confianza

Confirma el alias, usuario, host y huella previamente aprobada sin mostrar claves ni
la configuración completa. Antes de ejecutar SSH inspecciona localmente las opciones
aplicables: ProxyCommand, LocalCommand y perfiles pueden ejecutar acciones adicionales.
No uses `StrictHostKeyChecking=no`, no reemplaces known_hosts y no hagas trust-on-first-use
sin la comprobación del operador. No publiques hostname/IP privados innecesariamente.

Ejemplo de consulta simple, solo una vez validado el alias y su configuración:

```bash
ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=10 arbx \
  'date -u +%Y-%m-%dT%H:%M:%SZ; id -un; uname -sr; df -P; df -Pi'
```

La salida debe capturarse en la estación de trabajo. No añade redirecciones ni archivos
remotos. Usa los límites de tiempo del cliente para acotar el conjunto de la llamada.
No añadas `sudo` no autorizado, reenvío del agente, listeners públicos o túneles persistentes.

## Secretos: evitar la captura, no solo corregir el informe

No envíes a las herramientas secretos como argumentos: pueden quedar en historiales
locales de Desktop Commander [S4]. No imprimas `.env`, `docker inspect` completo,
`docker compose config` expandido, `systemctl cat cloudflared`, líneas completas de
procesos, cookies, tokens de URL ni cabeceras Authorization.

Extrae únicamente los campos necesarios antes de que salgan del equipo de origen.
En registros potencialmente sensibles usa un filtro revisado y probado con casos
sintéticos; si no puedes garantizar un saneado suficiente, omite el cuerpo y declara
el campo no recolectado. No desactives el historial para ocultar actividad.

Nombres de variables y presencia/ausencia bastan para inventario. La validación de
valores sensibles debe ejecutarse sin mostrarlos, con resultados booleanos/códigos.
La obligación de una variable debe derivarse del código o esquema efectivo; una
entrada en `.env.example` puede ser opcional o tener default.

## Navegador y ejecución

Desktop Commander dirige comandos; Playwright proporciona navegador y aserciones [S5].
Comprueba el directorio de trabajo, el proceso, la versión y el servidor realmente
levantado antes de correr un test. No supongas que el proyecto tiene una orden npm
`e2e` ni un proyecto Playwright llamado `chromium`: lee su configuración.

No arrancar un proceso equivale a no probar. Un proceso iniciado pero cuyo resultado
no se leyó queda `RUNNING/RESULT_PENDING`, nunca PASS. Al cerrar, detén únicamente los
procesos de prueba que creaste y registra los que deban seguir activos por el operador.

Fuentes S2–S5: [procedencia y fuentes](procedencia-y-fuentes.md).
