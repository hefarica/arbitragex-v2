Activa el **OMEGA TEAM COMPLETO** — 10 subagentes PhD/Nobel trabajando como equipo interdisciplinario.

## Protocolo de activación

Lee CLAUDE.md §15 para entender la estructura completa del equipo. Hay 3 workflows predefinidos. Pregunta al operador cuál ejecutar:

### Workflow A — Feature Nueva (9 fases)
```
Phase 1: STRATEGY    → /project:agent-strategy (evalúa viabilidad)
Phase 2: MATH        → /project:agent-math (valida algoritmos)
Phase 3: BUILD       → /project:agent-rust o agent-solidity (implementa)
Phase 4: VERIFY-CS   → /project:agent-cs (corrección formal)
Phase 5: UI          → /project:agent-frontend (interfaz si aplica)
Phase 6: SECURITY    → /project:agent-security (auditoría)
Phase 7: ECONOMICS   → /project:agent-economics (profit real post-costos)
Phase 8: DEPLOY      → /project:agent-devops (VPS + E2E)
Phase 9: DATA        → /project:agent-data (métricas y pipeline)
```

### Workflow B — Fixing Crítico (4 fases)
```
Phase 1: DIAGNOSE  → /project:agent-cs (root cause formal)
Phase 2: FIX       → /project:agent-rust o agent-frontend (corrección)
Phase 3: AUDIT     → /project:agent-security (sin nuevas vulnerabilidades)
Phase 4: DEPLOY    → /project:agent-devops (deploy + verify)
```

### Workflow C — Validación Pre-Mainnet (4 fases)
```
Phase 1: MATH      → /project:agent-math (corrección algorítmica)
Phase 2: ECONOMICS → /project:agent-economics (P&L real)
Phase 3: SECURITY  → /project:agent-security (auditoría completa)
Phase 4: CS        → /project:agent-cs (invariantes del sistema)
```

## Reglas del equipo
1. Cada subagente opera en su dominio. NO cruza responsabilidades sin motivo.
2. Todo builder tiene un validator asignado (ver matriz §15 CLAUDE.md).
3. Un validator puede BLOQUEAR el avance si encuentra un error crítico.
4. El OMEGA PROTOCOL aplica a cada subagente: verificar antes de declarar éxito.
5. R8 para todos: si no puedes validar algo, di "NO VERIFICABLE" — no asumas.

¿Qué workflow quieres ejecutar? (A, B, o C) ¿Qué tarea específica?
