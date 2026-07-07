# Checklist Operativo: Design System

- [ ] ¿El archivo `globals.css` y `tailwind.config.ts` contienen la definición de colores semánticos basados en variables CSS (`hsl`)?
- [ ] ¿Se purgaron todos los colores arbitrarios (`bg-[#xxxxxx]`) en las páginas (`page.tsx`) en favor de los design tokens del sistema?
- [ ] ¿Existe una semántica de severidad? (`destructive`, `warning`, `success`, `muted`, `accent`).
- [ ] ¿El panel se diseñó "Dark Mode First" o con soporte a alternancia dinámica a través de `next-themes`?
