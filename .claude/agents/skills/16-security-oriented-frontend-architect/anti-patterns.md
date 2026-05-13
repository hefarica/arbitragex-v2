# Antipatrones Prohibidos

## Antipatrón 1: Secret Leakage a través de Cliente
```tsx
// 🔴 PROHIBIDO: Esto incrusta la contraseña en texto claro en todos los navegadores
const dbPass = process.env.NEXT_PUBLIC_DB_PASSWORD;

export function connect() { ... }
```

## Antipatrón 2: Ocultamiento Visual en Lugar de Autorización
Ocultar un botón si el usuario no es Admin, pero dejar la ruta de API / Server Action desprotegida. Un atacante puede ejecutar el `fetch` directo a `/api/admin/kill` desde la consola.
