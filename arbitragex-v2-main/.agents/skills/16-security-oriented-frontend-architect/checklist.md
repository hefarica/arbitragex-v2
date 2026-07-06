# Checklist Operativo: Security & AppSec

- [ ] ¿Los tokens o claves sensibles (`ARBX_EDGE_TOKEN`, `DATABASE_URL`) carecen explícitamente del prefijo `NEXT_PUBLIC_`?
- [ ] ¿Los Server Actions (`'use server'`) validan la identidad / token del usuario o del request origin antes de procesar el input?
- [ ] ¿El archivo `next.config.js` incluye una lista estricta de `headers()` de seguridad (CSP, X-Frame-Options)?
- [ ] ¿Los formularios o Server Actions que aceptan payloads del exterior validan la data usando Zod antes de cruzar la capa del framework hacia la BD?
