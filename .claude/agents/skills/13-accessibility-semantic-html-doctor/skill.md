# Skill 13: Accessibility & Semantic HTML Doctor

## 1. Propósito
Garantizar que el HTML emitido por la aplicación React/Next sea sintácticamente correcto, semánticamente rico, y accesible (a11y) tanto para Screen Readers (Lectores de pantalla), indexadores (SEO/Bots) y navegadores de teclado. Prevenir fallos de hidratación causados por nido ilegal de etiquetas.

## 2. Aplicación directa en ARBITRAGEX
El uso correcto de roles de tabla en la tabla de Oportunidades, botones interactivos bien etiquetados (`aria-label` en botones con solo íconos), manejo correcto del Focus en los modales de configuración, y estructura limpia (`<main>`, `<aside>`, `<header>`) en el Layout para que sea navegable rápidamente sin ratón.

## 3. Problemas que resuelve
- Errores graves de hidratación en React por meter `<div>` dentro de `<p>`. (El navegador corrige el HTML, React detecta mismatch y estalla).
- Interfaz inutilizable por teclado (Tab no enfoca, Enter no activa).
- Modales que no atrapan el Focus (Focus Traps) permitiendo interactuar con el fondo.
- Botones compuestos por `<div>onClick</div>` en lugar de `<button>`.

## 4. Reglas Inmutables
- Nunca envolver elementos block-level (`<div>`, `<ul>`) dentro de elementos inline estrictos (`<p>`, `<span>`). React detesta esto durante la hidratación.
- Un elemento interactivo que realiza una acción en la misma página SIEMPRE debe ser un `<button>`. Si navega a una ruta distinta, SIEMPRE debe ser un `<a href>` (o `<Link>`). No inventar botones hechos con `<div>`.
- Todo componente que solo contenga un Icono (ej. un basurero) DEBE tener un `aria-label` descriptivo o texto de lectura exclusiva de pantalla (`sr-only`).
- Las imágenes DEBEN contener el atributo `alt`.

## 5. Nivel de Madurez
Profesional - Estándar mínimo de calidad web y respeto al usuario y motor del navegador.
