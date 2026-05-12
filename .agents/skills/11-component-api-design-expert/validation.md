# Validación y Auditoría

## 1. Criterios de Validación
- Seleccionar un componente de UI base (Ej. `Button.tsx`). Debe permitir pasarle un `data-testid="test-btn"`, y dicho atributo debe aparecer en el DOM renderizado (Prueba de propagación de props correcta).
- La inserción de una clase custom `<Button className="mt-4" />` no debe sobreescribir las clases base del botón, sino fusionarse armónicamente gracias a `tailwind-merge`.

## 2. Cómo Auditar en ARBITRAGEX
- Buscar la carpeta `/components/ui`. Analizar la definición de las interfaces de TypeScript. Si dicen `interface Props { className?: string }` pero omiten `extends React.HTMLAttributes<HTMLElement>`, marcarlo como fallo de escalabilidad.
