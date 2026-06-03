# Referencia de Optimización Frontend Fase 8

Usar esta referencia cuando el usuario pida optimizar el frontend de **ArbitrageX v2**, reducir peso de bundle, aplicar lazy loading, revisar componentes pesados o validar el build de producción.

## Objetivo operativo

El chat fuente menciona una fase de optimización frontend centrada en separación de componentes pesados, `React.lazy`, `Suspense`, code splitting, optimización de imágenes/fuentes y verificación con `npx next build`. Los candidatos principales observados son `OpportunitiesClient`, `DexRegistryClient` y `PoolsTab`.

| Área | Acción recomendada | Validación |
|---|---|---|
| Componentes pesados | Separar vistas o paneles que no se necesitan en el primer render. | Build exitoso y reducción de carga inicial. |
| Lazy loading | Usar `React.lazy` o carga dinámica compatible con el framework real. | No romper SSR/CSR ni rutas existentes. |
| Suspense | Añadir fallback ligero y estable. | UI sin pantalla en blanco. |
| Imágenes y fuentes | Revisar formatos, carga diferida y fuentes bloqueantes. | Lighthouse o inspección de network si aplica. |
| Build | Ejecutar `npx next build` o comando real del proyecto. | Build sin errores, warnings revisados. |

## Procedimiento

Primero confirmar si el proyecto usa Next.js, Vite u otra configuración. Aunque el chat fuente menciona `npx next build`, no asumir Next si el repositorio real muestra Vite u otro stack. Leer `package.json`, scripts, estructura `app/`, `pages/`, `src/` y configuración antes de cambiar imports.

```bash
cat package.json
find . -maxdepth 3 -name 'next.config.*' -o -name 'vite.config.*' -o -name 'package.json'
grep -RIn "OpportunitiesClient\|DexRegistryClient\|PoolsTab\|React.lazy\|Suspense\|dynamic(" frontend src app components 2>/dev/null
```

## Patrón de implementación

Usar carga diferida solo para componentes que no son críticos para el above-the-fold. Mantener datos y contratos tipados fuera del componente lazy si son compartidos por otros módulos. Si el framework es Next.js, evaluar `next/dynamic`; si es React puro/Vite, usar `React.lazy` y `Suspense`.

```tsx
import { Suspense, lazy } from 'react';

const OpportunitiesClient = lazy(() => import('./OpportunitiesClient'));

export function OpportunitiesSection() {
  return (
    <Suspense fallback={<div>Loading opportunities...</div>}>
      <OpportunitiesClient />
    </Suspense>
  );
}
```

Si el componente depende de APIs solo disponibles en navegador, proteger accesos a `window`, `localStorage`, WebSocket o wallets dentro de efectos o componentes client-side.

## Checklist de seguridad de refactor

| Riesgo | Mitigación |
|---|---|
| Romper SSR por uso de `window` | Revisar imports laterales y moverlos a runtime client-side. |
| Duplicar fetch por remount | Centralizar datos en store/cache o memoizar loader. |
| Fallback pobre | Usar fallback de altura estable para evitar layout shift. |
| Tipos rotos | Ejecutar TypeScript y build completo. |
| Rutas rotas | Probar navegación manual o pruebas e2e si existen. |

## Validación mínima

Ejecutar los scripts disponibles en el proyecto real. No imponer `npx next build` si no existe Next; usar el script de `package.json`.

```bash
npm run lint --if-present
npm run typecheck --if-present
npm run build
# Si el proyecto realmente es Next.js y no hay script alternativo:
npx next build
```

## Entregable recomendado

Entregar una tabla con componentes modificados, motivo de la separación, impacto esperado, comandos ejecutados y resultado. Si se detectan mejoras adicionales, separarlas como próximos pasos para evitar refactors demasiado amplios.
