# Patrones Correctos (Implementation)

## Patrón 1: Zod Schema as the Source of Truth
Validación runtime fuerte.

```tsx
import { z } from 'zod';

export const OpportunitySchema = z.object({
  id: z.string().uuid(),
  pair_symbol: z.string(),
  dex_a: z.string(),
  dex_b: z.string(),
  expected_profit_usd: z.number().or(z.string().transform(val => parseFloat(val))),
  risk_score: z.number().min(0).max(100),
  detected_at: z.string().datetime(),
});

// El tipo se infiere del validador. No se duplica código.
export type Opportunity = z.infer<typeof OpportunitySchema>;

// En el Fetcher:
const data = await res.json();
const parsed = OpportunitySchema.array().safeParse(data);
if (!parsed.success) {
  throw new Error("El motor Rust escupió un JSON con estructura corrompida.");
}
return parsed.data;
```

## Patrón 2: Discriminated Unions para Result/Errores (Rust-Like en TS)
```ts
export type Result<T> = 
  | { ok: true; data: T }
  | { ok: false; error: string; code: string };

export async function fetchLiveNodes(): Promise<Result<Node[]>> {
  try {
     // logica
     return { ok: true, data: nodos };
  } catch(e) {
     return { ok: false, error: (e as Error).message, code: "NET_FAIL" };
  }
}
```
