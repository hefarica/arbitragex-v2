# Configurar Kimi para Claude Code

## Paso 1: Obtener API Key de Kimi

1. Ve a https://platform.kimi.ai/
2. Crea una cuenta o inicia sesión
3. Ve a **API Keys** → **Create New Key**
4. Copia la key (empieza con `sk-...`)

## Paso 2: Configurar settings.json

Edita el archivo `C:\Users\HFRC\.claude\settings.json`:

```json
{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "env": {
    "ANTHROPIC_BASE_URL": "https://api.moonshot.ai/anthropic",
    "ANTHROPIC_AUTH_TOKEN": "sk-TU_API_KEY_AQUI",

    "ANTHROPIC_MODEL": "kimi-k2.5",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "kimi-k2.5",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "kimi-k2.5",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "kimi-k2.5",
    "ANTHROPIC_SMALL_FAST_MODEL": "kimi-k2.5",
    "CLAUDE_CODE_SUBAGENT_MODEL": "kimi-k2.5",

    "ENABLE_TOOL_SEARCH": "false",
    "CLAUDE_CODE_AUTO_COMPACT_WINDOW": "262144",
    "API_TIMEOUT_MS": "3000000"
  },
  "model": "opus",
  "theme": "light"
}
```

## Paso 3: Verificar la configuración

Abre PowerShell y ejecuta:

```powershell
# Limpiar cualquier sesión anterior
claude --help

# Crear carpeta de prueba
mkdir C:\tmp\kimi-test
cd C:\tmp\kimi-test

# Iniciar Claude Code
claude
```

Dentro de Claude Code ejecuta:
```
/status
```

Debería mostrar que está conectado a Kimi.

## Paso 4: Probar en tu proyecto

```powershell
cd "C:\Users\HFRC\Desktop\arbitragex-v2-main (17)"
claude
```

## Modelos disponibles en Kimi

| Modelo | Descripción |
|--------|-------------|
| `kimi-k2.5` | Modelo principal (recomendado) |
| `kimi-k2.7-code` | Versión optimizada para código |

## Solución de problemas

### Error "Prompt is too long"
```json
"CLAUDE_CODE_AUTO_COMPACT_WINDOW": "262144"
```

### Error de conexión
Verifica que la API key sea correcta y no haya espacios.

### Cambiar entre Kimi y Anthropic
Solo cambia las variables en `settings.json`:
- Kimi: `https://api.moonshot.ai/anthropic`
- Anthropic: `https://api.anthropic.com`
