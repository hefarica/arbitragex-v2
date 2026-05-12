ANTES de ejecutar `/compact`, persiste el contexto crítico de esta sesión para que sobreviva la compactación.

## Paso 1 — Captura el estado actual

Ejecuta estos comandos y guarda los resultados:

```bash
# Último commit
git log -1 --oneline

# Commits de hoy
git log --oneline --since="midnight" | head -20

# Estado del working tree
git status --short

# Containers corriendo (si hay acceso VPS)
ssh arbx "docker ps --format 'table {{.Names}}\t{{.Status}}'" 2>/dev/null || echo "VPS no accesible"

# Branch actual
git branch --show-current
```

## Paso 2 — Persiste en memoria

Crea o actualiza el archivo `.agents/memory/session_state.md` con este formato:

```markdown
# OMEGA CORTEX — Estado de sesión persistido
> Generado automáticamente antes de /compact

## Última sesión: {FECHA_ISO}

### Commits de esta sesión
{lista de commits de hoy}

### Estado del sistema
- Branch: {branch}
- Último commit: {hash + mensaje}
- Working tree: {limpio | N archivos modificados}
- VPS containers: {running | no accesible}

### Decisiones tomadas
{lista de decisiones arquitectónicas de la sesión}

### Trabajo en progreso
{lo que se estaba haciendo cuando se invocó /compact}

### Sprint / Phase actual
{sprint y phase actual del roadmap}

### Bugs conocidos activos
{lista de bugs abiertos con severidad}

### Próximo paso
{la siguiente tarea que el operador pidió o que sigue en el plan}
```

## Paso 3 — Actualiza anti-reincidencia si aplica

Si durante la sesión se descubrió un bug nuevo con patrón repetible, agrégalo a `.agents/memory/anti_reincidencia.md`.

## Paso 4 — Ejecuta el compact

AHORA sí ejecuta `/compact` con un resumen personalizado que incluya:
- Total de commits de la sesión
- Capas defensivas activas
- Sprint/Phase actual
- Próximo paso claro

## Paso 5 — Post-compact

Después del compact, la primera acción debe ser: leer `.agents/memory/session_state.md` para recuperar el contexto completo.

> **REGLA**: El chat es volátil. Los archivos persisten. NUNCA compactar sin persistir primero.
