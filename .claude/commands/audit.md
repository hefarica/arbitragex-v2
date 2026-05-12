Ejecuta una auditoría OMEGA completa del proyecto. Verifica cada capa sin modificar nada.

## Capa 1 — TypeScript/Frontend
- `cd frontend && npx tsc --noEmit` → ¿cero errores?
- `npm run lint` → ¿cero warnings críticos?
- `npm run build` → ¿build exitoso?
- Busca violaciones de RULE 00 (mocks/hardcode): `grep -rn "mock\|dummy\|fake\|hardcode\|localhost:8787" frontend/app/`
- Verifica R1 (Mounted Snapshot): ¿todos los page.tsx usan Server Component + *Client.tsx con useState(initialSnapshot)?

## Capa 2 — Rust/Backend
- `cd backend && cargo check --workspace` → ¿compila?
- `cargo clippy --workspace -- -D warnings` → ¿cero warnings?
- Busca hardcodes: `grep -rn "0x2222\|0x3333\|dummy\|todo!\|unimplemented!" backend/`
- Verifica que `Cargo.toml` no tenga dependencias obsoletas con `cargo outdated` si disponible.

## Capa 3 — Docker/Infra
- `docker compose config --quiet` → ¿configuración válida?
- Verifica que `.env` tiene TODAS las variables referenciadas en docker-compose.yml (R6).
- Busca violaciones de RULE 02: ¿algún WebSocket pasa por Edge Worker?

## Capa 4 — Datos (si VPS accesible)
- `ssh arbx "docker exec postgres psql -U arbx -c \"SELECT count(*), max(detected_at) FROM opportunities\""` → ¿datos recientes?
- `ssh arbx "docker exec redis redis-cli XLEN opportunities_stream"` → ¿pipeline activo?

## Reporte
Presenta un resumen con ✅/❌ por capa y lista de findings ordenados por severidad (CRITICAL → WARNING → INFO).
Aplica R8: si no puedes verificar algo, di "NO VERIFICABLE" en vez de inventar.
