# Patrones Correctos (Implementation)

## Patrón 1: Global Error Boundary Logger
```tsx
// app/error.tsx
'use client'; // Error boundaries obligatoriamente cliente

import { useEffect } from 'react';
import { captureError } from '@/lib/monitoring';

export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string }
  reset: () => void
}) {
  useEffect(() => {
    // Loguear asíncronamente a Grafana/Sentry
    captureError(error, { 
      digest: error.digest, 
      level: 'fatal', 
      context: 'GlobalRouter' 
    });
  }, [error]);

  return (
    <html>
      <body>
        <h2>Algo colapsó terriblemente.</h2>
        <button onClick={() => reset()}>Reintentar</button>
      </body>
    </html>
  );
}
```

## Patrón 2: Injecting Request IDs (Tracing)
```ts
// Al llamar a la API
const correlationId = crypto.randomUUID();
const res = await fetch('/api/action', {
  headers: {
    'X-Correlation-ID': correlationId
  }
});
// Si falla, el cliente sabe qué ID falló y el backend registró ese ID en Grafana.
```
