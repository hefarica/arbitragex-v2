#!/usr/bin/env bash
# sec-custody-identity-01-probe.sh
# SEC-CUSTODY-IDENTITY-01 — reproduccion SEGURA de la medicion de superficie de credenciales.
#
# Garantias:
#   - SOLO lectura. Ningun SET/DEL/CONFIG SET/SUBSCRIBE. Nada se reinicia ni se modifica.
#   - NUNCA imprime un valor de secreto: usa EXISTS/TYPE/TTL/STRLEN/OBJECT ENCODING,
#     conteos de grep (-c) y nombres de clave. Jamas GET / lrange / dump.
#   - Si un comando falla, lo reporta como fallo (no lo convierte en cero).
#
# Uso:  ssh arbx 'bash -s' < docs/security/sec-custody-identity-01-probe.sh
#       (o copiarlo al VPS y ejecutarlo como usuario con acceso a docker)
set -u

R=$(docker ps --format '{{.Names}}' | grep -i redis | head -1)
F=$(docker ps --format '{{.Names}}' | grep -i frontend | head -1)
[ -z "$R" ] && { echo "FALLO: contenedor redis no encontrado"; exit 1; }

hr() { printf '\n=== %s ===\n' "$1"; }

hr "1. Redis: exposicion y autenticacion"
docker ps --format '{{.Names}}|{{.Ports}}' | grep -i redis
echo "appendonly: $(docker exec "$R" redis-cli CONFIG GET appendonly | tail -1)"
echo "appendfsync: $(docker exec "$R" redis-cli CONFIG GET appendfsync | tail -1)"
PW=$(docker exec "$R" redis-cli CONFIG GET requirepass | tail -1)
echo "requirepass_len: ${#PW}   # 0 = SIN contrasena"
echo "--- ACL (nombres de usuario y permisos; los hashes de password se omiten) ---"
docker exec "$R" redis-cli ACL LIST | sed 's/ [0-9a-f]\{32,\}.*//'

hr "2. Proyeccion de credenciales: inventario (nombres) y LONGITUDES, nunca valores"
docker exec "$R" redis-cli --scan --pattern 'arbx:svc_cred:*' | wc -l | sed 's/^/claves arbx:svc_cred:* = /'
for k in $(docker exec "$R" redis-cli --scan --pattern 'arbx:svc_cred:*'); do
  printf '%s strlen=%s type=%s enc=%s ttl=%s\n' "$k" \
    "$(docker exec "$R" redis-cli STRLEN "$k")" \
    "$(docker exec "$R" redis-cli TYPE "$k")" \
    "$(docker exec "$R" redis-cli OBJECT ENCODING "$k")" \
    "$(docker exec "$R" redis-cli TTL "$k")"
done

hr "3. Estado de seguridad: quien tiene productor y quien no"
for k in arbx:killswitch arbx:papermode:global arbx:circuit_breaker:state; do
  printf '%s exists=%s type=%s\n' "$k" "$(docker exec "$R" redis-cli EXISTS "$k")" "$(docker exec "$R" redis-cli TYPE "$k")"
done

hr "4. Persistencia en disco (AOF) — conteos, no contenido"
docker exec "$R" sh -c 'ls -la /data/appendonlydir 2>/dev/null'
echo "--- ocurrencias del NOMBRE de clave en los AOF (no del valor) ---"
docker exec "$R" sh -c 'grep -rao "arbx:svc_cred:[a-z_0-9]*" /data/appendonlydir 2>/dev/null | sed "s/.*://" | sort | uniq -c'
echo "--- conteo del literal secret_value (0 NO prueba ausencia: RDB binario + LZF) ---"
docker exec "$R" sh -c 'grep -rac "secret_value" /data/appendonlydir 2>/dev/null'
docker exec "$R" redis-cli INFO persistence | grep -E 'aof_enabled|aof_current_size|aof_last_bgrewrite_status'

hr "5. Host: cifrado de bloque y placeholders de volumen"
lsblk -o NAME,FSTYPE,TYPE,MOUNTPOINT 2>/dev/null | head -20

hr "6. Radio: cuantos servicios comparten la red de Redis (lectores potenciales)"
echo "contenedores en ejecucion: $(docker ps -q | wc -l)"
docker network inspect arbitragex-v2_arbx-net --format '{{range .Containers}}{{.Name}} {{end}}' 2>/dev/null | tr ' ' '\n' | sed '/^$/d'

hr "7. F31 — pruebas NEGATIVAS contra la API (sin credencial alguna)"
EIP=$(docker inspect arbitragex-v2-edge-1 --format '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' 2>/dev/null)
printf 'edge_ip=%s\n' "$EIP"
printf 'anon  /api/credentials          -> %s (esperado 401)\n' "$(curl -s -o /dev/null -w '%{http_code}' --max-time 8 "http://$EIP:8787/api/credentials")"
printf 'cookie forjada                  -> %s (esperado 401)\n' "$(curl -s -o /dev/null -w '%{http_code}' --max-time 8 -H 'Cookie: arbx_admin_session=forged-not-a-token' "http://$EIP:8787/api/credentials")"
printf 'centinela forjado               -> %s (esperado 401)\n' "$(curl -s -o /dev/null -w '%{http_code}' --max-time 8 -H 'x-arbx-admin-token: __session_active__' "http://$EIP:8787/api/credentials")"
printf 'anon  /api/credentials/summary  -> %s (200 por diseno: solo conteos)\n' "$(curl -s -o /dev/null -w '%{http_code}' --max-time 8 "http://$EIP:8787/api/credentials/summary")"

hr "8. F31 — render ANONIMO de la pagina SSR (el control que se saltea)"
if [ -n "$F" ]; then
  FIP=$(docker inspect "$F" --format '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' 2>/dev/null)
  for P in 5173 3000; do
    CODE=$(curl -s -o /tmp/_secprobe.html -w '%{http_code}' --max-time 15 "http://$FIP:$P/settings/credentials" 2>/dev/null)
    if [ -s /tmp/_secprobe.html ]; then
      printf 'frontend:%s http=%s bytes=%s initialSnapshot=%s value_suffix=%s last_validation_error=%s\n' \
        "$P" "$CODE" "$(wc -c < /tmp/_secprobe.html)" \
        "$(grep -c initialSnapshot /tmp/_secprobe.html)" \
        "$(grep -c value_suffix /tmp/_secprobe.html)" \
        "$(grep -c last_validation_error /tmp/_secprobe.html)"
    else
      printf 'frontend:%s http=%s sin cuerpo\n' "$P" "$CODE"
    fi
    rm -f /tmp/_secprobe.html
  done
else
  echo "frontend no encontrado"
fi

hr "9. SQL de LONGITUDES (nunca SELECT secret_value) — correr por separado"
cat <<'SQL'
-- Ejecutar con: docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c "<query>"
SELECT provider, scope, status,
       length(secret_value)            AS plain_len,
       octet_length(secret_ciphertext) AS ct_len,   -- NULL = sin sobre cifrado
       secret_key_version              AS kv,
       length(metadata::text)          AS meta_len,
       length(coalesce(updated_by,'')) AS by_len
FROM service_credentials ORDER BY provider, scope;
-- Nombres de clave de metadata (nunca valores):
SELECT provider, scope, array_to_string(array(SELECT jsonb_object_keys(metadata)), ',') AS metadata_keys
FROM service_credentials ORDER BY provider, scope;
SQL

hr "FIN — ningun valor de secreto fue leido ni impreso"
