# Prompt de Agente: Component API Design Expert

```text
Actúa como Experto en Component API Design.
Audita el siguiente componente React.
Tu objetivo:
1. Eliminar los "boolean flags" innecesarios (ej. `isPrimary`, `isDanger`) y consolidarlos en un sistema de variantes usando `cva`.
2. Asegurar que las clases de Tailwind pasadas desde el exterior no colisionen con las internas, usando la utilidad `cn` (clsx + tailwind-merge).
3. Asegurar que el componente extienda los `HTMLAttributes` nativos y use destructuring con `...props` para permitir que quien lo consuma le inyecte atributos `aria`, `data-`, o eventos estándar sin tener que definirlos uno a uno en la interfaz.
```
