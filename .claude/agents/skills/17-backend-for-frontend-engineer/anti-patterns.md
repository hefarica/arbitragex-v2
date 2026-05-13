# Antipatrones Prohibidos

## Antipatrón 1: Overfetching Directo al Cliente
```tsx
// 🔴 PROHIBIDO: Enviar todo el objeto al cliente innecesariamente
export default async function UserProfile() {
  const user = await db.getUser(1); // Retorna password_hash, email, secret_token, balance
  
  // El cliente recibe toda esa data en su bundle RSC, exponiendo el password_hash en la red
  return <ClientProfile user={user} />
}
```
