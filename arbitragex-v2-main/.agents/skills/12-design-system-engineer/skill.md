# Skill 12: Design System Engineer

## 1. Propósito
Establecer, documentar y defender una fuente única de verdad para la capa visual (Design Tokens, Typography, Espaciados, Colores) utilizando Tailwind CSS, CSS Variables y shadcn/ui. Asegurar coherencia visual absoluta en todas las pantallas del producto, promoviendo el rehuso y castigando fuertemente el "Magic Value CSS" (uso de colores hexadecimales o márgenes duros en el código).

## 2. Aplicación directa en ARBITRAGEX
El dashboard operativo de ArbitrageX maneja alta densidad de datos. Debe existir una paleta estricta de severidad: `emerald` para Profit/OK, `rose` para Fallos/Pérdidas, `amber` para Alertas/Slippage. Todos los fondos deben obedecer a escalas semánticas (`bg-background`, `bg-muted`, `bg-card`) permitiendo el soporte multi-tema (Dark/Industrial).

## 3. Problemas que resuelve
- Inconsistencia visual (Ej: 5 tonos de verde diferentes en la misma tabla).
- Mantenimiento imposible de temas (Imposibilidad de cambiar el branding sin hacer Buscar/Reemplazar 1000 veces).
- UI Infantil o falta de jerarquía que rompe la inmersión de un panel "HFT / Profesional".
- Tailwind CSS bloating (archivos gigantes llenos de clases utilitarias arbitrarias como `bg-[#1a1b1e]`).

## 4. Reglas Inmutables
- **Cero Hexadecimales Locales:** Prohibido usar clases arbitrarias como `text-[#ff0000]` dentro de los componentes. Todos los colores deben venir del `tailwind.config.js` (`text-rose-500` o `text-destructive`).
- **Escala de Espaciado Estricta:** Usar los rems estándar de Tailwind (`p-4`, `m-2`, `gap-8`). Prohibido usar píxeles fijos como `mt-[17px]`.
- Variables CSS para colores semánticos (`--background`, `--foreground`, `--destructive`) en `globals.css` inyectadas en la configuración base.
- Si un bloque de Tailwind supera las 6-7 clases y no es un componente base primitivo, evaluar si la lógica pertenece a un archivo de variantes (`cva`).

## 5. Nivel de Madurez
Senior - Crea la gramática visual de la empresa.
