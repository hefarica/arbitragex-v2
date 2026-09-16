# XEN-FLOOD-EXCL-2026-09-15 — Exclusión de pools XEN del universo observado

> **ID de anomalía (§37 P-∅):** XEN-FLOOD-EXCL-2026-09-15 · **Tipo:** data-only (cero código) ·
> **Estado:** WORK ORDER — requiere GO del operador (escritura en PG de producción)

## 1. Qué pasa (evidencia verificada 2026-09-15)

- Muestra últimas 10,000 opportunities (VPS, 2026-09-15 ~00:16Z): **5,808 (58%)** rechazadas con
  `TokenNotAllowed:0x06450dee7fd2fb8e39061434babcfc05599a6fb8` (ventana 1h paralela: 16,800/27,736).
- El token **es XEN** (verificado on-chain read-only: `symbol()` → hex `58454e` = "XEN", RPC publicnode).
  Es el mismo token spam de la taxonomía del 2026-09-06 (XEN+AGLD = 78% del flood entonces).
- El allowlist del operador **correctamente NO permite XEN** → el gate `TokenIdentityIndex` rechaza
  cada oportunidad que lo toca. El problema NO es el gate: es que **5 pools XEN siguen activos en el
  universo observado**, generando detecciones que nacen muertas (~16k rechazos/hora, escrituras PG
  y ruido de dashboard, sin ningún posible throughput).

Pools XEN activos (chain 1, `pools.is_active=true`, TVL sin reportar):

| pool | fee_tier |
|---|---|
| 0xc0d776e2223c9a2ad13433dab7ec08cb9c5e76ae | 30 |
| 0x1add06b17398afca1ad9993ef17061317e463888 | 30 |
| 0x26f35b980f3b791ac3f7c09ff152815c0dcb5bf3 | 5 |
| 0x7995430a85156b2d40d5bb701608788cf84019e3 | 30 |
| 0x2a9d2ba41aba912316d16742f259412b681898db | 100 |

## 2. Por qué es data y no código

- La detección parte del universo de pools (`pools.is_active`); desactivar el pool elimina la
  detección en la RAÍZ para TODOS los modos (§34.1 mode-invariant: paper/shadow/live por igual).
- El gate `TokenNotAllowed` del evaluator **NO se toca** (queda como backstop de defensa en
  profundidad). No se hardcodea ninguna address en código (RULE 00 / arbx-no-hardcode): es un cambio
  de datos operativos, reversible con un UPDATE inverso.
- Los paths de grafo (`route_discovery/graph_builder`) son shadow off-by-default y route-discovery es
  Nivel 1 congelado (§37): NO se tocaron.

## 3. Procedimiento (backup-first, estilo G1)

```bash
# 1) Backup (guardar checksum)
ssh arbx "docker exec arbitragex-v2-postgres-1 pg_dump -U postgres arbitragex | gzip > /opt/backups/pre_xen_excl_$(date +%Y%m%d_%H%M).sql.gz"

# 2) Aplicar (SOLO las 5 pools verificadas arriba — jamás un UPDATE masivo sin WHERE por address)
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c \"UPDATE pools SET is_active=false WHERE address IN ('0xc0d776e2223c9a2ad13433dab7ec08cb9c5e76ae','0x1add06b17398afca1ad9993ef17061317e463888','0x26f35b980f3b791ac3f7c09ff152815c0dcb5bf3','0x7995430a85156b2d40d5bb701608788cf84019e3','0x2a9d2ba41aba912316d16742f259412b681898db') AND chain_id=1;\""

# 3) Verificar (10-30 min después): el bucket TokenNotAllowed:0x0645... debe caer a ~0
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c \"SELECT rejection_reason, count(*) FROM opportunities WHERE detected_at > now() - interval '30 minutes' GROUP BY 1 ORDER BY 2 DESC LIMIT 8;\""
```

Reversibilidad: `UPDATE pools SET is_active=true WHERE ...` mismas 5 addresses.
No requiere flush de Redis: el universo se reconstruye desde PG en el ciclo de enumeración.

## 4. Qué NO hace este cambio

- No re-etiqueta nada (no toca opportunities históricas — la lección R-0001 aplica).
- No modifica el allowlist (si el operador algún día QUIERE tradear XEN, la vía es
  `allowed_token_symbols` + reactivar pools — decisión suya, no de este doc).
- No toca código; la branch `fix/567-canonical-plan-consumer` es independiente.

## 5. Criterio de éxito

`TokenNotAllowed:0x0645...` < 1% del total de rechazos en ventana de 1h post-cambio, con el
total de opportunities/hora cayendo ~58% (capacidad recuperada para detección real).
