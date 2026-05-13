# Antipatrones Prohibidos

## Antipatrón 1: Mutation sin Revalidación
Un CRUD que no purga la caché obliga al usuario a usar `Ctrl+F5` o dudar de si la acción tuvo éxito.

```tsx
// 🔴 PROHIBIDO
'use server';
export async function updateProfile(data) {
  await db.updateUser(data);
  // Falta revalidatePath('/profile')
  // La UI seguirá mostrando el perfil viejo de manera infinita.
}
```

## Antipatrón 2: Cachear lo Incacheable
Poner ISR (ej. `revalidate: 60`) en endpoints financieros o endpoints que emiten nonces/tokens de seguridad (Anti-CSRF).
