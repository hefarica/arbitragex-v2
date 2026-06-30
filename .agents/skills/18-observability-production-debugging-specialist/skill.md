# Skill 18: Observability & Production Debugging Specialist

## 1. Propósito
Instrumentar la aplicación React/Next.js para tener visibilidad atómica en Producción. Integrar trazabilidad (OpenTelemetry, Sentry, Datadog), métricas de Core Web Vitals (Next.js Analytics), recolectar registros de error detallados (Source Maps decodificados) y generar alertas de salud en tiempo real.

## 2. Aplicación directa en ARBITRAGEX
Si el frontend del VPS (<VPS_IP>) colapsa, tira errores 500 silenciosos, o la hidratación falla masivamente para un operador externo, los logs en consola del navegador están perdidos. Se necesita capturarlos (Sentry / Datadog) o enviar los errores de React a un endpoint de logs de servidor (Loki/Promtail) para analizarlos en Grafana sin acceder al PC del usuario.

## 3. Problemas que resuelve
- "Funciona en mi máquina, pero en producción arroja pantalla blanca".
- Errores minificados crípticos de React (`Error #425`) imposibles de rastrear a qué archivo o componente pertenecen sin Source Maps.
- Fugas de memoria lentas (Memory leaks) detectables tarde.
- Imposibilidad de saber si las llamadas API están fallando por Timeouts en el cliente.

## 4. Reglas Inmutables
- Errores asíncronos y Crash Boundaries (archivos `error.tsx` en Next) **DEBEN** inyectar el objeto `error` hacia el logger de producción (Sentry/Loki). Imprimir a `console.error` no basta en producción.
- **Source Maps Ocultos:** Generar Source Maps durante el Build (`productionBrowserSourceMaps: true` o subidos a Sentry) para decodificar trazas de error, pero evitar exponerlos en la ruta pública si la IP (Propiedad Intelectual) del código es confidencial.
- Toda medición crítica de performance (`useReportWebVitals`) debe recolectarse y agregarse estadísticamente.

## 5. Nivel de Madurez
Senior - "Si no puedes observarlo, no está en producción".
