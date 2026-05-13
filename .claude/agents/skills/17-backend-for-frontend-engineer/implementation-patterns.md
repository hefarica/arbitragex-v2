# Patrones Correctos (Implementation)

## Patrón 1: Data Shaping en el Server Component
El Server Component abstrae la complejidad de la BD y pasa props limpias.

```tsx
// 🟢 CORRECTO
import { db } from '@/lib/db';

export default async function TopOpportunitiesPage() {
  // El usuario nunca ve la query SQL ni recibe campos confidenciales.
  const rawData = await db.query('SELECT * FROM opportunities ORDER BY profit DESC LIMIT 10');
  
  // Data Shaping (BFF Logic)
  const cleanData = rawData.map(row => ({
    id: row.id,
    title: `${row.dex_a} ↔ ${row.dex_b}`,
    profit: row.profit_usd,
    // Ignoramos campos sensibles como 'arbitrage_path_bytecode'
  }));

  return <OpportunitiesList data={cleanData} />
}
```

## Patrón 2: API Route como Proxy Seguro
```ts
// /app/api/trigger/route.ts
import { NextResponse } from 'next/server';
import { rustEngineCall } from '@/lib/rust-api';

export async function POST(req: Request) {
  // Esconde la URL del servidor de Rust y los tokens de Auth de los ojos del navegador.
  const result = await rustEngineCall("/execute-trade", { auth: process.env.RUST_SECRET });
  return NextResponse.json({ success: true, txId: result.txId });
}
```
