# Patrones Correctos (Implementation)

## Patrón 1: Lazy Loading Heavy Dependencies
```tsx
// 🟢 CORRECTO
import dynamic from 'next/dynamic';

// Este componente gigante no bloqueará la carga de la página principal.
const HeavyChart = dynamic(() => import('@/components/HeavyChart'), {
  ssr: false, // Opcional: Deshabilita el render en servidor si depende de 'window' o Canvas
  loading: () => <div className="h-[400px] bg-slate-900 animate-pulse rounded-xl" />
});

export default function AnalyticsDashboard() {
  return (
    <div>
      <MetricsHeader />
      <HeavyChart />
    </div>
  );
}
```

## Patrón 2: Tree-shakable Imports
```tsx
// 🟢 CORRECTO: Solo importa la utilidad exacta, permitiendo descartar el resto del paquete.
import { formatDistanceToNow } from 'date-fns/formatDistanceToNow';
// o en librerías modernas como lodash-es:
import debounce from 'lodash-es/debounce';
```
