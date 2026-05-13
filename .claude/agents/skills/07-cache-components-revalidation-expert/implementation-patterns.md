# Patrones Correctos (Implementation)

## Patrón 1: Tag-based Revalidation
```tsx
// En el Server Component
export async function getSystemSettings() {
  const res = await fetch('https://api/settings', {
    next: { tags: ['system-settings'] } // Etiqueta de caché
  });
  return res.json();
}

// En el Server Action (Mutación)
'use server';
import { revalidateTag } from 'next/cache';

export async function updateSystemSettings(formData: FormData) {
  await db.updateSettings(formData);
  // Purgar inmediatamente la caché de todos los componentes que usen esta etiqueta
  revalidateTag('system-settings');
  return { success: true };
}
```
