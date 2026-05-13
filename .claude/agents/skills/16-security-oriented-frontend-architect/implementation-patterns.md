# Patrones Correctos (Implementation)

## Patrón 1: Secure Server Actions (Zero Trust)
No asumas que quien llama al Server Action es el botón de tu propia UI.

```tsx
// 🟢 CORRECTO
'use server';

import { cookies, headers } from 'next/headers';
import { db } from '@/lib/db';
import { z } from 'zod';

const InputSchema = z.object({ newLimit: z.number().max(100) });

export async function updateGasLimit(data: FormData) {
  // 1. Autorización (Re-verificar)
  const authCookie = cookies().get('session_token');
  if (!isValidSession(authCookie)) throw new Error("Unauthorized");
  
  // 2. Validación de Input (Zod)
  const parsed = InputSchema.safeParse({ newLimit: data.get('limit') });
  if (!parsed.success) throw new Error("Invalid Input");

  // 3. Ejecución Segura
  await db.updateGas(parsed.data.newLimit);
}
```

## Patrón 2: Strict HTTP Headers in Next.js
En `next.config.js`:
```js
module.exports = {
  async headers() {
    return [
      {
        source: '/(.*)',
        headers: [
          { key: 'X-Frame-Options', value: 'DENY' }, // Clickjacking protection
          { key: 'X-Content-Type-Options', value: 'nosniff' },
          { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
        ],
      },
    ];
  },
};
```
