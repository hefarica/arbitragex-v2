# Antipatrones Prohibidos

## Antipatrón 1: Poisoning the Client
Poner `use client` muy arriba en el árbol y perder las ventajas del SSR para componentes estáticos masivos.

```tsx
// 🔴 PROHIBIDO: Ensuciar todo el árbol
'use client';
// Todo este archivo y sus subcomponentes importados forzosamente serán empaquetados y enviados al cliente.

import { HugeD3Library } from 'd3';
import { StaticFooter } from './footer';

export default function Dashboard() {
   const [state, setState] = useState(0);
   return <div><HugeD3Library /><StaticFooter /></div>;
}
```

## Antipatrón 2: Propagación de Objetos Ricos (Unserializable Props)
```tsx
// 🔴 PROHIBIDO
// En un Server Component
export default async function Page() {
  const date = new Date(); // Objeto fecha
  return <ClientComponent currentDate={date} /> // Error: Cannot serialize Date object in RSC payload.
}
```
