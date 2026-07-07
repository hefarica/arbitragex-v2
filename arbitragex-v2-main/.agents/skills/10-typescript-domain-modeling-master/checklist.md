# Checklist Operativo: TS Domain Modeling

- [ ] ¿Existe la directiva `strict: true` y `noImplicitAny: true` en el `tsconfig.json` productivo?
- [ ] ¿Los payloads provenientes de las respuestas HTTP/Fetch se parsean a través de `schema.safeParse` (Zod) antes de tratarlos como el Tipo final?
- [ ] ¿Se eliminaron todos los type castings peligrosos (`response.json() as Opportunity[]`) reemplazándolos con guardias de tipo estables?
- [ ] ¿El catálogo de eventos del WebSocket es una Unión Discriminada estricta y predecible?
