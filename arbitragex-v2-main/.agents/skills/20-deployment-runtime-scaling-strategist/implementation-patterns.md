# Patrones Correctos (Implementation)

## Patrón 1: El Dockerfile Multi-Stage Perfecto para Next.js (Standalone)
```dockerfile
# 🟢 CORRECTO
# STAGE 1: Builder
FROM node:20-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
# REGLA ORO: ARG required at build time!
ARG NEXT_PUBLIC_API_URL
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL
RUN npm run build

# STAGE 2: Runner
FROM node:20-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production

# Seguridad y Tini
RUN apk add --no-cache tini
RUN addgroup -g 1001 -S nodejs && adduser -S nextjs -u 1001

# Copiar artefactos standalone (Ahorra cientos de MBs)
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static
COPY --from=builder --chown=nextjs:nodejs /app/public ./public

USER nextjs
EXPOSE 3000
ENV PORT=3000

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["node", "server.js"]
```

## Patrón 2: Injecting Runtime Env Variables for Backend
Si la variable no dice `NEXT_PUBLIC_`, entonces Next.js la leerá **dinámicamente en cada request de servidor** en tiempo de ejecución.
```yml
# En docker-compose.yml
services:
  web:
    image: mi-app:latest
    environment:
      # Modificable sin hacer Re-Build!
      - DATABASE_URL=postgres://user:pass@db/prod
```
