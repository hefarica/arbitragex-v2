# Skill 01: Hydration Forensics Expert

## 1. Propósito
Convertir al agente en especialista capaz de detectar, aislar, explicar y corregir cualquier error de hidratación en React/Next, incluyendo los errores minificados #418, #423 y #425. Garantiza que el árbol DOM generado por SSR sea un reflejo exacto del árbol DOM en la fase inicial del CSR.

## 2. Aplicación directa en ARBITRAGEX
Esta skill se aplica en los dashboards en tiempo real, las tablas de oportunidades MEV, los paneles WebSocket, los componentes con datos vivos (`/opportunities/page.tsx`), las cards de métricas, los gráficos de rendimiento y los sistemas de inicialización.

## 3. Problemas que resuelve
- Hydration mismatch (El HTML del server no coincide con el render inicial del client).
- Text content mismatch (Ej: Fechas renderizadas con huso horario UTC vs Local).
- Diferencias estructurales entre Server Component payload y Client Component hydration.
- WebSocket iniciado o renderizando datos antes de la fase de montaje (`mounted = true`).
- Ejecución de APIs de navegador (`window`, `localStorage`, `navigator`) durante el render estático o SSR.
- Parpadeos visuales (Layout shift) causados por renderizados asíncronos incorrectos en la hidratación.
- Pérdida de rendimiento extremo por el fallback a renderizado completo de cliente (`Root fallback to Client rendering`).

## 4. Reglas Inmutables
- El primer render del cliente DEBE ser bit-a-bit idéntico al HTML generado por el servidor.
- Todo dato "vivo" y asíncrono inicia estrictamente DESPUÉS de `useEffect` (fase de montaje).
- Todo valor `browser-only` (localStorage, sessionStorage, window) debe evaluarse tras `mounted = true`.
- Todo timestamp generado en el servidor debe serializarse como ISO String y sólo formatearse tras la hidratación.
- **NO USAR `suppressHydrationWarning`** salvo en la etiqueta `<html>` para atributos externos inyectados por extensiones, o casos 100% justificados de discrepancia intencional aislada.
- No ocultar los errores de hidratación; se debe corregir la discrepancia raíz.

## 5. Casos de Uso dentro del Proyecto
- Componente `<LiveFeed />` donde la fecha inicial (`now`) debe estar congelada a 0 (o null) en SSR y actualizarse a la fecha real de la máquina local en la fase de montaje para evitar error #425.
- Renderizado condicional basado en métricas cacheadas en `localStorage` (Ej: preferencias de tema oscuro).

## 6. Nivel de Madurez
PhD / Clase Mundial - El conocimiento impartido previene fallos a nivel de Engine Rendering de React 18+.
