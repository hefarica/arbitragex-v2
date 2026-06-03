# Checklist de Ejecución — Agente Resolutivo Total

## Antes de Cada Intervención

- [ ] Leer el estado actual del proyecto (`git status`, `git log --oneline -10`)
- [ ] Identificar el nivel de prioridad (P0–P4)
- [ ] Mapear dependencias del módulo a modificar
- [ ] Verificar que no hay cambios ajenos sin revisar

## Durante la Implementación

- [ ] No hardcodear valores — usar variables de entorno o configuración dinámica
- [ ] No usar mocks en producción — datos reales siempre
- [ ] No exponer secretos en logs, commits ni documentación
- [ ] Aplicar cambios idempotentes cuando sea posible
- [ ] Mantener compatibilidad con módulos existentes
- [ ] Documentar cada cambio con propósito claro

## Validación Obligatoria Post-Cambio

### Stack Node.js / Frontend
```bash
npm run lint
npm run typecheck
npm run build
npm run test
```

### Stack Rust / Backend
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace --no-fail-fast
cargo audit
```

### Stack Solidity / Contratos
```bash
forge build
forge test
forge coverage
```

### Stack Python
```bash
python -m mypy .
python -m pytest --tb=short
```

### Infraestructura / Docker
```bash
docker compose config --quiet
docker compose build --no-cache
docker compose up -d && docker compose ps
```

## Seguridad y Secretos

- [ ] Ningún secreto en plaintext en el código
- [ ] Variables de entorno documentadas en `.env.example`
- [ ] Credenciales rotadas si se detectó exposición
- [ ] Dependencias auditadas (`npm audit`, `cargo audit`)

## Git y Trazabilidad

- [ ] `git status` limpio antes de commit
- [ ] Commit atómico: una unidad funcional validada por commit
- [ ] Mensaje de commit claro: `tipo(alcance): descripción`
- [ ] ADR creado si se cambió un contrato de interfaz

## Definición de Terminado

- [ ] Compila sin errores
- [ ] Corre en entorno local/staging
- [ ] Integrado con el sistema existente
- [ ] Pruebas mínimas pasan
- [ ] No rompe módulos existentes
- [ ] Evidencia de validación documentada
- [ ] Ruta de despliegue clara o ya desplegado

## Frases Prohibidas

No usar nunca:
- "Sería recomendable…"
- "Podrías intentar…"
- "Tal vez el problema sea…"
- "Necesitaría más contexto…"
- "No puedo avanzar sin confirmación…"
- "Esto debería revisarse luego…"

En su lugar: revisar, ejecutar, corregir, documentar, validar, entregar evidencia.
