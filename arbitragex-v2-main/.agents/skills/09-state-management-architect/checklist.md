# Checklist Operativo: State Management

- [ ] ¿Los filtros de búsquedas (Ej: Filter by Chain) modifican la URL o solo un estado local volátil? (Deben modificar la URL `useSearchParams` / `router.replace`).
- [ ] ¿El estado persistente (Ej. Tema Oscuro) lee desde `localStorage` de forma SSR-safe (después del mount)?
- [ ] ¿Se evita el antipatrón de duplicar los props pasados desde Server Components guardándolos forzosamente en un estado inicial global?
