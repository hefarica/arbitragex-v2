# nginx — entry point canónico de producción (NGINX-CONF-DRIFT-01)

`arbitragex.conf` es la **fuente de verdad byte-exacta** del routing `:80` del
VPS (195.201.235.70, detrás de Cloudflare para `arbx.ape-tv.net`).

- Instalación / reconciliación: `bash scripts/vps/install-nginx-canonical.sh`
  (backup + `nginx -t` + rollback automático + reload + smoke).
- Gate anti-drift: `scripts/vps/verify-deploy.sh` sección **L0** falla si el
  archivo vivo (`/etc/nginx/sites-enabled/arbitragex`) difiere de este archivo.

## Decisiones FIJAS — no "restaurar"

1. **SIN `limit_req` en nginx.** Tras Cloudflare, `$binary_remote_addr` es la IP
   de egreso de CF, no el cliente → un solo bucket para todo el tráfico humano
   (recrearía la tormenta EDGE-429-BROWSER-01, resuelta en #531). La autoridad
   de rate-limit es el **edge worker** (Redis KV, floor 600/min por IP real).
2. **`/socket.io/` → `127.0.0.1:8080` directo** (RULE 02): WebSocket a
   api-server, NUNCA via edge (fix POLLING 2026-08-20).
3. **`server_name` por IP**: único server block = *default server* para
   cualquier `Host` (Cloudflare pasa el Host original del dominio).

## Histórico

`sites-enabled/arbitragex` fue un **archivo regular divergido** de
`sites-available` entre el 2026-05-17 y el 2026-08-23: el *available* viejo
tenía `limit_req` zones + `/socket.io/` → `8787` (ambos incorrectos — el
segundo rompería WS si se activaba). Reconciliado y versionado el 2026-09-05:
el contenido VIVO (correcto según RULE 02) pasó a ser el canónico del repo;
`install-nginx-canonical.sh` sobreescribe el *available* divergido y convierte
el *enabled* en symlink.
