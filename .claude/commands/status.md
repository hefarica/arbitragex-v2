Reporta el estado actual del sistema ArbitrageX en tiempo real. NO modifiques nada.

1. **VPS**: `ssh arbx "docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'"` — ¿todos los contenedores corriendo?
2. **Searcher**: `ssh arbx "docker logs searcher-rs --tail 10 --since 5m"` — ¿generando oportunidades?
3. **Redis**: `ssh arbx "docker exec redis redis-cli XLEN opportunities_stream"` — ¿pipeline fluyendo?
4. **Postgres**: `ssh arbx "docker exec postgres psql -U arbx -c \"SELECT count(*) as total, max(detected_at) as ultima FROM opportunities\""` — ¿datos frescos?
5. **API Health**: `curl -s https://edge-arbx.ape-tv.net/api/health` — ¿200 OK?
6. **Frontend**: `curl -s -o /dev/null -w '%{http_code}' https://edge-arbx.ape-tv.net` — ¿200?
7. **Último profit**: `ssh arbx "docker exec postgres psql -U arbx -c \"SELECT expected_profit_usd, detected_at FROM opportunities ORDER BY detected_at DESC LIMIT 3\""` — ¿valores reales (no null)?

Presenta el reporte con 🟢/🔴 por componente. Aplica R8: si no puedes conectar, reporta "NO ACCESIBLE" no inventes.
