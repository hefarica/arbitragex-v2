# Instalación de Claude Code con Kimi K2.7 en Windows

Guía completa para configurar Claude Code usando el modelo Kimi K2.7 Code en Windows mediante PowerShell.

---

## 1. Instalar Node.js

Abre **PowerShell** y ejecuta:

```powershell
winget install OpenJS.NodeJS
```

Cierra PowerShell y ábrelo de nuevo. Luego valida:

```powershell
node -v
npm -v
```

---

## 2. Instalar Claude Code

Ejecuta:

```powershell
npm install -g @anthropic-ai/claude-code
```

Si falla por red o lentitud, usa el registry alternativo:

```powershell
npm install -g @anthropic-ai/claude-code --registry=https://registry.npmmirror.com
```

---

## 3. Crear Archivo de Configuración

Para configuración global del usuario en Windows, abre:

```powershell
notepad "$env:USERPROFILE\.claude\settings.json"
```

Claude Code usa `~/.claude/settings.json` para configuración de usuario. En Windows, `~/.claude` equivale a `%USERPROFILE%\.claude`.

Pega la siguiente configuración:

```json
{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "env": {
    "ANTHROPIC_BASE_URL": "https://api.moonshot.ai/anthropic",
    "ANTHROPIC_AUTH_TOKEN": "PEGA_AQUI_TU_API_KEY_DE_KIMI",

    "ANTHROPIC_MODEL": "kimi-k2.7-code",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "kimi-k2.7-code",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "kimi-k2.7-code",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "kimi-k2.7-code",
    "ANTHROPIC_SMALL_FAST_MODEL": "kimi-k2.7-code",
    "CLAUDE_CODE_SUBAGENT_MODEL": "kimi-k2.7-code",

    "API_TIMEOUT_MS": "3000000",
    "CLAUDE_CODE_AUTO_COMPACT_WINDOW": "262144",
    "ENABLE_TOOL_SEARCH": "false"
  }
}
```

### Variables de Entorno Importantes

| Variable | Descripción |
|----------|-------------|
| `ANTHROPIC_BASE_URL` | Endpoint de Kimi API para Claude Code: `https://api.moonshot.ai/anthropic` |
| `ANTHROPIC_AUTH_TOKEN` | Tu API key personal de Kimi |
| `ANTHROPIC_MODEL` | Modelo por defecto: `kimi-k2.7-code` |
| `API_TIMEOUT_MS` | Timeout de API: 3,000,000ms (50 minutos) |
| `CLAUDE_CODE_AUTO_COMPACT_WINDOW` | Ventana de compactación automática: 262,144 tokens |
| `ENABLE_TOOL_SEARCH` | Desactiva búsqueda de herramientas: `false` |

---

## 4. Iniciar Claude Code

En PowerShell:

```powershell
claude
```

Dentro de Claude Code, valida el estado con:

```
/status
```

---

## 5. Versión Rápida (Opcional)

Para usar la versión de alta velocidad, cambia todos los:

```json
"kimi-k2.7-code"
```

por:

```json
"kimi-k2.7-code-highspeed"
```

---

## ⚠️ Seguridad Importante

**NUNCA** guardes este archivo dentro de un repositorio Git si contiene tu API key.

Si necesitas configuración por proyecto, usa:

```
.claude/settings.local.json
```

Y confirma que esté en `.gitignore`:

```
.claude/settings.local.json
```

---

## Referencias

- [Kimi API Platform - Claude Code Support](https://platform.moonshot.ai/docs/guide/agent-support)
- [Claude Code Settings - Anthropic Docs](https://docs.anthropic.com/en/docs/claude-code/settings)

---

*Documento generado para ArbitrageX v2 - OMEGA CORTEX*
