# Checklist Operativo: Deployment

- [ ] Docker Build: ¿Las variables `NEXT_PUBLIC_` están pasadas como `ARG` en el Dockerfile y como `args:` en el `docker-compose.yml` en el bloque build?
- [ ] Docker Image: ¿El Dockerfile es Multi-Stage (Builder vs Runner) evitando que el compilador TypeScript se suba a la fase productiva?
- [ ] Seguridad Node: ¿El proceso de Next se ejecuta como un usuario sin privilegios (ej. `USER nextjs`) en lugar de `root`?
- [ ] Standalone: ¿El archivo `next.config.js` incluye `output: "standalone"`?
