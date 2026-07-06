# Patrones Correctos (Implementation)

## Patrón 1: The "Mounted" State
Se utiliza para componentes que dependen enteramente de APIs del navegador o estados que no existen en el servidor (ej: LocalStorage, Dimensiones de Pantalla, Timezones).

```tsx
import { useState, useEffect } from 'react';

export function ClientOnlyComponent({ children }) {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) {
    // Renderizado neutral idéntico al SSR
    return <div className="animate-pulse bg-slate-800 h-10 rounded"></div>;
  }

  return <>{children}</>;
}
```

## Patrón 2: Initial SSR-Safe Timestamps
Evita desajustes por ejecución en distinto milisegundo o Timezone.

```tsx
// 🟢 CORRECTO
export function LiveClock() {
  const [now, setNow] = useState<number>(0); // Estado inicial consistente (SSR-safe)

  useEffect(() => {
    setNow(Date.now()); // Hidratación completada, actualizar a valor vivo
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);

  return <div>{now === 0 ? "Loading time..." : new Date(now).toLocaleTimeString()}</div>;
}
```
