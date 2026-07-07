# Patrones Correctos (Implementation)

## Patrón 1: Force Dynamic (Cero Cache Estático)
Usado en `/opportunities/page.tsx` si hubiera fetch de backend directo.

```tsx
// Indicar al motor de Next.js que NUNCA genere un snapshot en Build time.
export const dynamic = 'force-dynamic';
export const fetchCache = 'force-no-store';

export default async function DashboardPage() {
  // Garantizado que se ejecutará en TIEMPO DE REQUEST, no de BUILD.
  const data = await db.getCurrentState(); 
  return <LiveFeed initial={data} />;
}
```

## Patrón 2: Fetch con `no-store`
Si no exportas la directiva global, hazlo a nivel de petición.

```tsx
const res = await fetch("http://api/metrics", {
    cache: "no-store", 
    // Opcionalmente en Next 15: { next: { revalidate: 0 } }
});
```
