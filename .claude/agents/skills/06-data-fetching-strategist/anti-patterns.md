# Antipatrones Prohibidos

## Antipatrón 1: Fetching Local Route Handlers en Server Components
Next.js bloqueará o lanzará errores si el servidor intenta hacerse un fetch HTTP a sí mismo durante el Build Time.

```tsx
// 🔴 PROHIBIDO
export default async function Page() {
  // NO HAGAS ESTO. El servidor no debe consumir su propio HTTP.
  const res = await fetch("http://localhost:3000/api/config");
  const data = await res.json();
  return <View data={data} />;
}

// 🟢 CORRECTO
import { getConfig } from '@/lib/config';
export default async function Page() {
  // Llama directamente a la función lógica.
  const data = await getConfig(); 
  return <View data={data} />;
}
```

## Antipatrón 2: Data Fetching en un useEffect Crudo
Esto provoca race conditions, memory leaks si el componente se desmonta rápido, y nulo manejo de caché.

```tsx
// 🔴 PROHIBIDO (Si la data muta a menudo o es compleja)
useEffect(() => {
   let active = true;
   fetch('/api/data').then(res => res.json()).then(data => {
      if (active) setData(data);
   });
   return () => { active = false; }
}, []);
```
