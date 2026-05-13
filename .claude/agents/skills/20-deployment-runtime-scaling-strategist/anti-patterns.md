# Antipatrones Prohibidos

## Antipatrón 1: Confusión Build/Runtime de Variables Next.js
El error más común del ecosistema (El caso `localhost:8787`).

```yml
# 🔴 PROHIBIDO (NO ALTERA EL FRONTEND DE CLIENTE)
services:
  frontend:
    build: . # Compila SIN la variable. Next.js le asigna "undefined" o el fallback del código a todos los JS estáticos.
    environment:
      - NEXT_PUBLIC_API_URL=https://api.produccion.com # El Server Node lo leerá, pero el Navegador jamás lo verá.
```

## Antipatrón 2: npm start como PID 1
Correr `CMD ["npm", "start"]` lanza shells intermedios que no reenvían señales de apagado a Node. Al actualizar el contenedor, Docker esperará 10s y le enviará un `SIGKILL` forzoso, cortando conexiones abruptamente.
