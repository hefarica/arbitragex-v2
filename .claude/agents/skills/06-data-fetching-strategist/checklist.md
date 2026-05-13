# Checklist Operativo: Data Fetching

- [ ] En Server Components: ¿Se usa acceso directo a la DB o Servicios en lugar de `fetch("http://localhost:3000/api/...")`?
- [ ] En componentes asíncronos complejos: ¿Se usa `Promise.all` para lanzar requests en paralelo si no dependen uno de otro?
- [ ] En Client Components: Si hay polling o estados complejos de request, ¿Se usa TanStack Query / SWR en lugar de `useEffect + useState`?
- [ ] En APIs de Terceros: ¿Se provee un `AbortSignal.timeout(X)` para evitar conexiones colgadas infinitamente?
