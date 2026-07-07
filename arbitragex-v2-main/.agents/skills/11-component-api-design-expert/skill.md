# Skill 11: Component API Design Expert

## 1. Propósito
Diseñar APIs de componentes React (Props) que sean predecibles, escalables, autodocumentadas y resistentes a cambios futuros. Evitar el antipatrón de componentes monolíticos con 50 props ("Prop Drilling Hell") favoreciendo la composición, la inversión de control (IoC) y el patrón Polymorphic Components.

## 2. Aplicación directa en ARBITRAGEX
La construcción de componentes reutilizables como `Card`, `Badge`, `DataTable`, `Modal` y `MetricBox` utilizados a lo largo del dashboard, configuración, y paneles de logs. Si el componente de tabla de oportunidades (`OppTable`) requiere múltiples variantes (Ej: modo compacto, modo extendido), en lugar de agregar banderas booleanas como `isCompact={true}`, se utiliza composición.

## 3. Problemas que resuelve
- Componentes Frankenstein: Un componente `Button` que recibe 30 props booleanas (`isRed`, `isLarge`, `isLoading`, `hasIconLeft`, `hasIconRight`).
- Dificultad para testear componentes debido al acoplamiento duro de sus partes internas.
- Rigidez visual: Imposibilidad de alterar una parte pequeña del componente sin añadir otra prop más.

## 4. Reglas Inmutables
- **Regla del Máximo de Props:** Si un componente de presentación (UI) tiene más de 7 props exclusivas de diseño, probablemente necesite separarse usando el patrón de Composición (pasar `children` o `slots`).
- **Inversión de Control (IoC):** En vez de pasar `data` a un `Card` y que el `Card` decida cómo renderizar el título y el cuerpo, pasa el título y el cuerpo pre-renderizados como `children`.
- Uso de `clsx` o `tailwind-merge` (`cn`) para la combinación dinámica de clases en lugar de condicionales con literales es obligatorio.
- Los componentes interactivos (Botones, Inputs) deben propagar atributos HTML nativos mediante `...props` (Rest parameters) extendiendo interfaces como `React.ButtonHTMLAttributes<HTMLButtonElement>`.

## 5. Nivel de Madurez
Senior - Diseño de SDKs de UI mantenibles a largo plazo.
