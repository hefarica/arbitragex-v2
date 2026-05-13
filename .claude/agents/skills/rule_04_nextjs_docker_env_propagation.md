# RULE 04: NEXT.JS DOCKER COMPOSE ENV PROPAGATION STRICTNESS

## 1. DEFINICIÓN DE LA REGLA

Esta regla complementa la [RULE 03](./rule_03_nextjs_docker_build_strictness.md) resolviendo la causa raíz de la "fuga de localhost" (localhost leakage) en ambientes productivos VPS. Establece el mecanismo obligatorio de cómo Docker Compose debe inyectar las variables de entorno `.env` durante la fase de compilación (`build.args`) para evitar que Next.js "queme" valores por defecto.

## 2. EL PROBLEMA DEL CONTEXTO Y LA INTERPOLACIÓN

En una arquitectura de contenedores, existen **dos niveles críticos de variables de entorno**:
1. **Runtime Env** (Declarado vía `env_file: ["../.env"]`): Estas variables están disponibles *dentro* del contenedor una vez que se inicia. Node.js las lee correctamente, pero para Next.js es demasiado tarde si son variables de cliente (`NEXT_PUBLIC_*`).
2. **Build Args** (Declarado bajo la directiva `build:`): Estas variables deben inyectarse al momento de hacer el `docker compose build` para que Next.js las "hornee" (bake) en el código estático.

El bug crítico (ej. `localhost:8787` apareciendo en la cabecera CSP o en peticiones fetch de producción) ocurre por la **Interpolación de Docker Compose**:
```yaml
      args:
        NEXT_PUBLIC_EDGE_URL: ${NEXT_PUBLIC_EDGE_URL:-http://localhost:8787}
```

### Por qué falla:
Cuando se ejecuta `docker compose -f docker/compose.dev.yml up -d frontend` desde la raíz `/opt/arbitragex-v2`:
- Docker Compose asume automáticamente que el archivo `.env` se encuentra en el **mismo directorio** que el archivo `docker-compose.yml` especificado.
- Va a buscar `docker/.env` y, como no existe (está en `/opt/arbitragex-v2/.env`), la variable `$NEXT_PUBLIC_EDGE_URL` queda vacía a nivel del shell de Compose.
- Docker Compose resuelve la interpolación aplicando el fallback hardcodeado: `http://localhost:8787`.
- Next.js compila el frontend productivo creyendo que el VPS está en localhost.

## 3. PROTOCOLO OBLIGATORIO DE COMPILACIÓN (COMMAND DOCTRINE)

Para evitar que el contexto de Compose ignore el `.env` raíz, se debe forzar explícitamente la inyección del archivo mediante el flag `--env-file`.

**ESTÁ PROHIBIDO** ejecutar `docker compose build` o `up` en producción sin declarar de dónde leer las variables si el archivo YAML está anidado.

**Comando Obligatorio Exacto para Despliegues en el VPS:**
```bash
docker compose --env-file .env -f docker/compose.dev.yml build --no-cache frontend
docker compose --env-file .env -f docker/compose.dev.yml up -d frontend
```

## 4. VALIDACIÓN DE EVIDENCIA

Tras cada compilación, el contenedor debe validarse revisando que la inyección fue correcta. Esto se logra examinando el encabezado de respuesta HTTP (ej. CSP - Content Security Policy) devuelto por el Next.js Node Server:

```bash
curl -I http://127.0.0.1:5173/opportunities
```
Si el resultado de `connect-src` contiene `localhost`, **LA REGLA FUE VIOLADA** y la inyección falló. Debe contener URLs productivas absolutas (ej. `https://edge-arbx.ape-tv.net`).
