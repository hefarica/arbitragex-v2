# Checklist Operativo: App Router

- [ ] ¿Están todas las rutas lógicas agrupadas en un Route Group si comparten Layouts (ej: `(dashboard)`)?
- [ ] ¿Existe un archivo `error.tsx` en el directorio raíz de la función protegida para atrapar panics en React sin botar la vista entera?
- [ ] ¿Existe un `loading.tsx` para aprovechar React Suspense e indicar carga entre navegaciones?
- [ ] ¿Se están utilizando Route Handlers (`route.ts`) adecuadamente para APIs en lugar de usar Server Actions si la intención es ser consumido por un cliente de terceros?
