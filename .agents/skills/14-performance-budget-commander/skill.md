# Skill 14: Performance Budget Commander

## 1. Propósito
Monitorear, defender y optimizar el peso global del bundle de JavaScript enviado al cliente. Mantener Core Web Vitals (LCP, FID, CLS, INP) en rangos de excelencia. Dominar Dynamic Imports (`next/dynamic`), code-splitting y el control del impacto de dependencias pesadas (Charts, Date libraries, Mapas) sobre el TTI (Time to Interactive).

## 2. Aplicación directa en ARBITRAGEX
El dashboard tiene o tendrá bibliotecas de visualización de datos de la red blockchain y gráficas de profit. Si se importa una librería gigantesca como `echarts` o `d3` en un Client Component raíz, destruiremos el tiempo de carga del usuario. Esas bibliotecas deben empaquetarse separadamente y cargarse asíncronamente (Lazy Loading) solo cuando entran en la pantalla.

## 3. Problemas que resuelve
- First Load / Time to Interactive lentsísimo (White Screen).
- Bundles de JS superiores a 1MB parseados en dispositivos de gama media/baja.
- Impacto severo al Interaction to Next Paint (INP) porque el Main Thread está bloqueado evaluando megabytes de código en el primer frame.

## 4. Reglas Inmutables
- Toda librería pesada de terceros orientada exclusivamente a un widget (Gráficos estadísticos, mapas, editores de texto rico, animaciones lottie masivas) DEBE ser importada dinámicamente (`next/dynamic` o `React.lazy`).
- Prefiere alternativas modernas o funciones nativas (Intl.DateTimeFormat) en lugar de librerías utilitarias masivas y anticuadas como `moment.js` o `lodash` completo.
- Mantener el First Load JS del bundle inicial en Next.js (Visible durante el npm run build) estrictamente inferior a 150KB gzip.

## 5. Nivel de Madurez
Senior - Defiende el rendimiento bajo estrés en redes lentas.
