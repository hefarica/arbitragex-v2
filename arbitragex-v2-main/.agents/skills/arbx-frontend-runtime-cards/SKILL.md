---
name: arbx-frontend-runtime-cards
description: Crear cards frontend para runtime status sin engañar al operador.
---
# arbx-frontend-runtime-cards

## Purpose
Diseñar interfaces React (Next.js) de visualización de métricas y semáforos de diagnóstico, basándose estrictamente en lo provisto por la capa de red, sin inferir falsos positivos y manejando vacíos informativos con franqueza.

## When to use
Cuando desarrolles o modifiques los módulos de UI en frontend que exponen salud (e.g., tarjetas visuales de estado de motores) usando `/api/strategies/runtime-status`.

## Inputs needed
- Proxy de Worker (URL de Edge Cloudflare).
- React FC definitions and State hooks (`useQuery` / `useEffect`).
- Componentes base del Dashboard (Cards, Badges, Loaders).

## Files usually touched
- `frontend/src/app/operations/page.tsx` (u otras rutas maestras)
- `frontend/src/components/cards/RuntimeStatusCard.tsx`

## Commands
- `pnpm --filter frontend run dev`
- `pnpm --filter frontend build`

## Safety rules
- Nunca insertar tiempos de retardo cosméticos (`setTimeout` visuales) para encubrir llamadas veloces. 
- Semáforo canónico inquebrantable:
  - VERDE: engine_loaded = true y candidates > 0
  - AMARILLO: loaded = true, dependencies ok pero candidates = 0
  - GRIS: loaded unknown o sin señales
  - ROJO: critical deps missing / DB unavailable

## Verification steps
1. Mockea intencionalmente un status rojo (apagando el server local o devolviendo 500) y corrobora que la tarjeta se vuelva visiblemente roja de inmediato.
2. Comprueba un status donde candidatos = 0 pero dependencies están ok (AMARILLO).

## Failure modes
- Quedarse pegado en loading permanente infinito (`<Spinner />`) ignorando el renderizado gris (o vacío) ante respuestas null del Edge.

## Golden output
UI de tarjeta React que despliega claramente: "Triangular Arb | Icono Amarillo | Armado, esperando impacto rentable".

## Anti-patterns
- Embeber respuestas predeterminadas en el hook inicial `[data, setData] = useState(MOCK_DATA)` que se queden parpadeando brevemente de color verde.
- Validar "éxito" meramente sobre HTTP 200 sin revisar la estructura de payload de DB y semánticas dependientes.

## Example prompt
"Usa arbx-frontend-runtime-cards para crear el grid visual de 4 tarjetas semaforizadas para las estrategias en la vista de operations, mapeando el estado de candidatos según reglas."
