# Skill 19: Testing & Quality Gate Architect

## 1. Propósito
Crear una red de seguridad infalible y piramidal: Pruebas estáticas (TypeScript, ESLint), Pruebas Unitarias veloces para componentes sin efectos visuales (Vitest, React Testing Library), y Pruebas End-to-End (E2E) imitando navegadores reales (Playwright/Cypress). Bloquear cualquier despliegue que rompa la UI o retroceda funcionalidad.

## 2. Aplicación directa en ARBITRAGEX
Un error tipográfico en el CSS puede ignorarse, pero una falla de renderizado en el `LiveFeed` que impida al operador hacer clic en el botón `SIMULATE` cuesta dinero. Se deben ejecutar flujos E2E de simulación de oportunidad usando MSW (Mock Service Worker solo en Testing) o instancias limpias de backend.

## 3. Problemas que resuelve
- Despliegues rotos ("Push to Prod & Pray").
- Tests unitarios frágiles (Testear implementaciones internas en lugar de comportamiento).
- Falsos Positivos (Tests que pasan aunque la pantalla de login esté en blanco por error de hidratación).

## 4. Reglas Inmutables
- **Test Behavior, Not Implementation:** No testees cuántas veces se llamó a `useState`. Testea si el botón dice "Simulate", y si al hacer clic se muestra "Loading".
- Cobertura 100% de E2E en las "Happy Paths" críticas (Login, Lectura de Oportunidad, Botón Ejecutar).
- Fallar rápido (Fail Fast): TypeScript y ESLint son las primeras puertas (Gates). Si TS arroja error, no se corre Vitest. Si Vitest falla, no se corre Playwright.
- Jamás usar Mocks en Producción (Doctrina de ArbitrageX), pero SI está permitido usarlos exclusivamente dentro del marco de la suite unitaria o E2E simulado (`vi.mock` / MSW) para no depender del motor de Rust encendido durante CI.

## 5. Nivel de Madurez
Senior - Cultura de Calidad (Shift-Left Testing).
