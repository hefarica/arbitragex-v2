# Checklist Operativo: Observability

- [ ] ¿Los `error.tsx` están capturando la excepción y enviándola a un servicio centralizado de Logging antes de mostrar la UI de fallback?
- [ ] ¿El API Client (`lib/api-client.ts`) loguea las fallas HTTP repetitivas (5xx) a un sistema de monitoreo en lugar de tragar el error o dejarlo solo en la consola del navegador?
- [ ] ¿Hay un sistema de correlación (Correlation ID / Request ID) pasando desde el frontend hacia el backend para rastrear una petición a lo largo de todo el stack?
