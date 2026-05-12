Activa **Agent Teams** — múltiples instancias Claude trabajando en paralelo con git worktrees.

## Prerequisitos
```bash
# Crear worktrees para aislamiento de cada agente
git worktree add ../.worktrees/rust-agent main
git worktree add ../.worktrees/frontend-agent main
git worktree add ../.worktrees/solidity-agent main
```

## Modo Team Lead (Tú eres el orquestador)

Al recibir una tarea compleja, descomponla en subtasks y delega a los agentes de `.claude/agents/`:

### Ejemplo: Implementar nueva estrategia CEX-DEX

```
TASK LIST:
- [ ] @strategy-architect: Evaluar viabilidad y ROI de CEX-DEX (read-only)
- [ ] @math-validator: Validar matemática de spread calculation (read-only)  
- [ ] @rust-mev-engineer: Implementar WebSocket Binance + spread detector
- [ ] @solidity-engineer: Crear contrato de ejecución multi-venue
- [ ] @cs-validator: Verificar concurrencia del feed dual CEX+DEX (read-only)
- [ ] @frontend-architect: Dashboard de spreads en tiempo real
- [ ] @security-auditor: Auditar API keys y MEV exposure (read-only)
- [ ] @economics-validator: Validar P&L con costos completos (read-only)
- [ ] @devops-platform: Deploy con nuevo container binance-feed
- [ ] @data-analytics: Schema y queries para historical spreads
```

### Reglas de orquestación
1. Validators (read-only) pueden ejecutar en PARALELO con builders.
2. Builders con archivos distintos pueden ejecutar en PARALELO (rust + frontend + solidity).
3. Builders que tocan los mismos archivos deben ejecutar en SERIE.
4. Un validator puede BLOQUEAR: si reporta error critical, el builder corrige antes de continuar.
5. El Team Lead (tú) consolida resultados y reporta al operador.

### Secuencia de resolución de conflictos
Si dos agents editaron el mismo archivo en worktrees diferentes:
```bash
cd ../.worktrees/rust-agent && git diff main
cd ../.worktrees/frontend-agent && git diff main
# Merge manual o cherry-pick selectivo
```

Pregunta al operador qué tarea ejecutar y con qué nivel de paralelismo.
