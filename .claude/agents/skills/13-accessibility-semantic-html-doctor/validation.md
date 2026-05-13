# Validación y Auditoría

## 1. Criterios de Validación
- Navegar la aplicación entera usando SOLAMENTE el botón `Tab`, `Enter` y `Espacio` en el teclado. Todos los modales, botones, inputs y filtros deben ser alcanzables y activables.
- Inspeccionar el DOM con una herramienta validadora W3C o Lighthouse (Accessibility Score > 90).

## 2. Cómo Auditar
- Buscar `onClick` asociado a etiquetas `<div>`, `<span>`, `<a>` sin `href`, o `<svg>`. Exigir la transformación a `<button>`.
- Buscar `<p>` en los códigos y verificar visualmente que no encierren `div`, `table` ni `ul`.
