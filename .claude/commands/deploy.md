Ejecuta el protocolo de deploy completo al VPS (RULE 01 + RULE 03 + R6 + R7).

Secuencia obligatoria:

1. **Pre-flight**: Verifica que el working tree está limpio (`git status`). Si hay cambios sin commit, PARA y reporta.
2. **Push**: `git add -A && git commit -m "<describe changes>" && git push origin main`
3. **SSH al VPS**: `ssh arbx` y ejecuta:
   ```
   cd /opt/arbitragex-v2
   git pull origin main
   docker compose build --no-cache --env-file .env
   docker compose up -d
   ```
4. **Verificación E2E (R7)**:
   - `docker logs searcher-rs --tail 20` → ¿genera oportunidades?
   - `docker exec redis redis-cli XLEN opportunities_stream` → ¿>0?
   - `docker exec postgres psql -U arbx -c "SELECT count(*) FROM opportunities"` → ¿crece?
   - `curl -s https://edge-arbx.ape-tv.net/api/health` → ¿200 OK?
   - `curl -s https://edge-arbx.ape-tv.net/api/opportunities?limit=3` → ¿datos reales?
5. **Post-flight**: Si CUALQUIER verificación falla, ejecuta `docker compose logs --tail 50` y diagnostica. Loop autónomo de corrección (OMEGA PROTOCOL).

NUNCA declares "deployado" sin mostrar evidencia de los 5 checks.
