# Checklist Operativo: Rendering Strategy

- [ ] ¿Los endpoints de métricas en vivo declaran explícitamente su naturaleza dinámica en Route Handlers (`export const dynamic = "force-dynamic"` o `{ cache: 'no-store' }`)?
- [ ] ¿Los listados de logs históricos que se consultan frecuentemente tienen una estrategia de revalidación temporal (ISR) para salvar queries a la BD?
- [ ] ¿Hay validación por parte del build step? (Next.js te dice durante el build: `λ  (Server)  server-side renders at runtime` vs `○  (Static)  automatically rendered as static HTML`).
