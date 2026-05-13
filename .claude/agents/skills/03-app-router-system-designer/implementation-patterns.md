# Patrones Correctos (Implementation)

## Patrón 1: Aislamiento por Route Groups
```text
/app
  /(dashboard)
    layout.tsx (Sidebar y Navbar globales)
    /opportunities
      page.tsx
      loading.tsx
      error.tsx
    /settings
      page.tsx
  /(auth)
    layout.tsx (Layout limpio sin sidebar)
    /login
      page.tsx
```

## Patrón 2: Error Boundary Granular
```tsx
// /app/(dashboard)/opportunities/error.tsx
'use client'; // Error boundaries obligatoriamente son Client Components

import { useEffect } from 'react';

export default function ErrorBoundary({ error, reset }: { error: Error; reset: () => void }) {
  useEffect(() => {
    // Loguear el error a un servicio de observabilidad
    console.error("Critical Route Error:", error);
  }, [error]);

  return (
    <div className="bg-rose-950 text-rose-300 p-8 rounded border border-rose-800">
      <h2>Módulo de oportunidades inoperable.</h2>
      <button onClick={() => reset()}>Reintentar Renderizado</button>
    </div>
  );
}
```
