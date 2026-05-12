# Patrones Correctos (Implementation)

## Patrón 1: Server Components via Children
Permite que un Server Component viva debajo de un Client component sin convertirse en Client component.

```tsx
// 🟢 CORRECTO: El servidor renderiza el Sidebar, y el layout cliente lo recibe
// Layout.tsx (Client Component)
'use client';
import { useState } from 'react';

export default function AppLayout({ sidebar, children }) {
  const [open, setOpen] = useState(false);
  
  return (
    <div>
      <button onClick={() => setOpen(!open)}>Toggle</button>
      {open && <aside>{sidebar}</aside>}
      <main>{children}</main>
    </div>
  );
}

// Page.tsx (Server Component)
import { db } from '@/lib/db';
import { SidebarData } from '@/components/SidebarData';

export default async function Page() {
  const data = await db.fetchSystemMetrics(); // Acceso seguro
  
  return (
    <AppLayout sidebar={<SidebarData data={data} />}>
      <MainContent />
    </AppLayout>
  );
}
```
