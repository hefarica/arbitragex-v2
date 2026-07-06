# Checklist Operativo: Hydration Forensics

- [ ] Revisión del uso de `Date.now()`, `Math.random()` o generación de UUIDs directamente en el cuerpo del JSX. Deben moverse a un `useEffect` o estar inicializados estáticamente.
- [ ] Verificación de que no haya acceso a `window`, `document`, o `localStorage` fuera de un `useEffect` o sin un chequeo explícito `typeof window !== "undefined"`.
- [ ] Asegurarse de que el formateo de horas locales (`toLocaleString`) suceda únicamente cuando el componente reporta estar "mounted".
- [ ] Comprobar que todos los tags semánticos en HTML son válidos (ej. `<p>` no puede contener `<div>`, `<tbody>` requiere estar dentro de `<table>`). Reacciona fuertemente a HTML malformado durante la hidratación.
- [ ] Validación de los Server Components para evitar enviar Date objects puros por props a Client components. Pasarlos siempre como `string` o `number`.
- [ ] Si se renderizan tooltips de terceros que inyectan clases de manera impredecible en SSR, usar `typeof window` para evitar su carga en SSR.
