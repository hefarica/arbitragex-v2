# Checklist Operativo: Testing & Quality

- [ ] Estático: ¿El comando `npm run typecheck` (`tsc --noEmit`) y `eslint` terminan con 0 errores?
- [ ] Unitario: ¿Los componentes utilitarios y formatters (Ej: `formatDate`, `calculateROI`) tienen tests estrictos (Vitest)?
- [ ] E2E: ¿Existe al menos un flujo de Playwright que levanta Next.js, entra a `/opportunities`, e inspecciona que cargue correctamente?
- [ ] ¿El CI pipeline (Ej. Github Actions o Husky pre-commit) aborta el build si alguna de estas capas falla?
