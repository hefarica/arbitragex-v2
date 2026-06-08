# ArbitrageX v2 — Dev Container Frontend en Windows

Este directorio permite abrir el repositorio en **Visual Studio Code sobre Windows** con la extensión oficial **Dev Containers** y ejecutar el **Frontend Next.js** en un contenedor reproducible. El objetivo es validar visualmente el Control Plane en `http://localhost:5173/executions` sin instalar Node.js directamente en Windows y sin tocar servicios productivos.

> El contenedor ejecuta el Frontend en modo desarrollo. Para consumir datos reales de staging, abre un túnel SSH local hacia los servicios loopback del VPS; de este modo, el navegador y los Server Components alcanzan Edge/API sin exponer puertos públicamente.

## Requisitos en Windows

| Requisito | Propósito |
|---|---|
| Docker Desktop con backend WSL 2 | Ejecutar el contenedor de desarrollo. |
| Visual Studio Code | IDE local. |
| Extensión `Dev Containers` de Microsoft | Abrir el repo dentro del contenedor. |
| Git para Windows | Clonar y actualizar la rama. |
| OpenSSH Client | Abrir túneles contra staging si quieres datos reales. |

## Flujo recomendado

Primero clona el repositorio y entra en la rama de trabajo.

```powershell
git clone https://github.com/hefarica/arbitragex-v2.git
cd arbitragex-v2
git checkout feature/topology-vault-rpc-mux
code .
```

En VS Code, ejecuta **Dev Containers: Reopen in Container**. El primer arranque construirá la imagen y ejecutará `npm install --workspaces --include-workspace-root` dentro del contenedor. Cuando termine, abre una terminal integrada del contenedor y ejecuta:

```bash
npm run -w @arbx/frontend dev:container
```

El Frontend quedará disponible en:

```text
http://localhost:5173/executions
```

## Conectar contra staging mediante túnel SSH

Si quieres que el Frontend local consuma Edge/API/WS de staging, abre una ventana de PowerShell en Windows y ejecuta:

```powershell
.\scripts\devcontainer-tunnel-staging.ps1
```

El script asume que tienes un alias SSH llamado `arbx`. Si no lo tienes, puedes pasar un destino explícito:

```powershell
.\scripts\devcontainer-tunnel-staging.ps1 -SshTarget root@195.201.235.70
```

Mientras esa ventana permanezca abierta, el Frontend usará estas rutas locales:

| Servicio | URL local | Uso |
|---|---:|---|
| Frontend Next.js | `http://localhost:5173` | Interfaz visual. |
| Edge API | `http://localhost:8787` | REST consumido por el Frontend. |
| WebSocket / Socket.IO | `http://localhost:3000` | Streams y eventos en tiempo real. |

## Variables de entorno del contenedor

| Variable | Valor por defecto | Razón |
|---|---|---|
| `NEXT_PUBLIC_EDGE_URL` | `http://localhost:8787` | El navegador llama al túnel local de Edge. |
| `NEXT_PUBLIC_WS_URL` | `http://localhost:3000` | El navegador conecta WS/Socket.IO por túnel local. |
| `NEXT_PUBLIC_WSS_URL` | `http://localhost:3000` | Compatibilidad con hooks que leen esta variable. |
| `INTERNAL_EDGE_URL` | `http://host.docker.internal:8787` | Server Components dentro del contenedor alcanzan el túnel del host Windows. |

## Comandos útiles dentro del contenedor

| Comando | Resultado |
|---|---|
| `npm run -w @arbx/frontend dev:container` | Arranca Next.js en `0.0.0.0:5173`. |
| `npm run -w @arbx/frontend typecheck` | Valida TypeScript del Frontend. |
| `npm run -w @arbx/frontend test` | Ejecuta Vitest del Frontend. |
| `npm run -w @arbx/frontend lint` | Ejecuta ESLint del Frontend. |

## Notas de seguridad

No guardes tokens GitHub, claves privadas ni credenciales de blockchain dentro de `.devcontainer`, `.env` o scripts versionados. Este Dev Container está diseñado para depender de túneles locales y variables públicas de desarrollo, no de secretos persistidos en el repo.

## Troubleshooting

| Síntoma | Corrección |
|---|---|
| `localhost:5173` no abre | Verifica que el comando `dev:container` siga corriendo y que Docker Desktop permita puertos publicados. |
| La UI carga pero no hay datos | Abre el túnel PowerShell hacia staging o levanta el stack local de backend/edge. |
| Server Components fallan al llamar Edge | Confirma que `INTERNAL_EDGE_URL=http://host.docker.internal:8787` y que el túnel local está activo. |
| Cambios de archivos no refrescan | El Compose activa `CHOKIDAR_USEPOLLING` y `WATCHPACK_POLLING`; reinicia el servidor Next si persiste. |
