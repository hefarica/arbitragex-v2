# Patrones Correctos (Implementation)

## Patrón 1: Server-Side Parallel Data Fetching
Evita el Waterfall en vistas maestras (Ej: Dashboard).

```tsx
// 🟢 CORRECTO
import { db } from '@/lib/db';
import { getLiveMetrics, getUserConfig } from '@/lib/services';

export default async function DashboardPage() {
  // Lanzar en paralelo
  const metricsPromise = getLiveMetrics();
  const configPromise = getUserConfig();
  
  // Esperar a todas simultáneamente
  const [metrics, config] = await Promise.all([metricsPromise, configPromise]);

  return <DashboardView metrics={metrics} config={config} />;
}
```

## Patrón 2: Client-Side Data with TanStack Query
Manejo profesional del estado asíncrono en cliente.

```tsx
// 🟢 CORRECTO
'use client';
import { useQuery } from '@tanstack/react-query';

export function SystemHealth() {
  const { data, isLoading, error } = useQuery({
    queryKey: ['health'],
    queryFn: async () => {
      const res = await fetch('/api/readiness');
      if (!res.ok) throw new Error('Network error');
      return res.json();
    },
    refetchInterval: 5000, // Polling built-in
  });

  if (isLoading) return <Skeleton />;
  if (error) return <ErrorAlert error={error} />;
  return <HealthCard data={data} />;
}
```
