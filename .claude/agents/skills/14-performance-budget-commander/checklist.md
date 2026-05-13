# Checklist Operativo: Performance Budget

- [ ] Durante `npm run build`, ¿Next.js muestra que el bundle de la ruta principal (`/`) está en color verde (pequeño) y no en amarillo/rojo?
- [ ] ¿Los componentes de gráficos (Charts) están importados usando `next/dynamic` con un indicador de carga (`loading: () => <Skeleton />`)?
- [ ] ¿Se auditaron las dependencias de fechas (reemplazar Moment por date-fns o nativo)?
- [ ] ¿Las imágenes estáticas están utilizando el componente nativo `<Image>` de Next.js (`next/image`) con tamaños explícitos para evitar Cumulative Layout Shift (CLS)?
