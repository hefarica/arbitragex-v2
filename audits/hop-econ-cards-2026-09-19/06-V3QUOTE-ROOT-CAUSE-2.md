# Root-Cause v2: v3_quote_unavailable (78% del funnel) — 2026-09-19/20

Autorizado por el operador ("¿y esto cómo se soluciona de raíz?"). Continúa
`audits/real-cycles-audit-20260916/ROOT-CAUSE-v3-quote.md` (B1-B3).

## Evidencia viva (2026-09-20 03:0xZ, searcher iniciado 02:27Z)

- PG: `v3_quote_unavailable` 69.930 rechazos / 30 min (vs 7.818 spot_product_le_one).
- Métrica `arbx_v3_quote_total` rate 30m: `cache_neg_hit` 36.6/s · `rpc` 1.94/s ·
  `rpc_error` 1.94/s · `cache_hit` **0.0** · `rpc_ok` **0.0** → 100% de RPC falla.
- `arbx_rpc_provider_state{service=searcher-rs}`: **publicnode/drpc/flashbots/mevblocker
  = Open(2)**; blockpi Healthy(0). relays-client y recon usan LOS MISMOS proveedores
  sanos (state=0) → el daño es específico del volumen/uso del searcher.
- Sonda HTTP desde el host VPS (`eth_blockNumber`): drpc 200 · flashbots 200 ·
  mevblocker 200 · 0xrpc 200 · blockpi 200 · **alchemy 429 (quota agotada)** ·
  llama 403 · **1rpc 410 Gone (proveedor muerto en la lista)**.
- `RPC_HTTP_1` declara 9 proveedores pero la métrica del searcher sólo registra 5
  (alchemy/llama/0xrpc/1rpc ausentes — dropeados en boot o no exportados).

## Cadena causal (de raíz)

1. El quoter emite **1 eth_call por pool** (`vec![1]` en v3_quote_provider.rs:257)
   — B1 (batching) SIGUE ABIERTO desde la RCA del 09-16. La caché TTL (B2) amortigua
   duplicados pero el volumen único por tick + el resto de usos del pool saturan a los
   públicos gratuitos.
2. Los públicos 429/403 → breakers abren con floor 120s + backoff exponencial
   (ARBX-R-0003) → `AllUnhealthy` → quote falla → **neg-cache 2s** → reintento →
   bucle. 36.6/s de rechazos instantáneos por neg-cache = 78% del funnel.
3. flashbots `/fast` y mevblocker son **relays MEV, no RPC generales** (el eth_call
   del quoter los 403-a permanente → reabren eternamente).
4. **alchemy (única opción privada escalable) está 429-eando incluso un
   eth_blockNumber suelto** — la key está sin quota (free tier agotada; precedente
   2026-09-04 "Alchemy PAYG 80%"). El operador la añadió el 09-17 pero así no carga.
5. 1rpc=410 Gone contamina la lista.

## Fix de raíz (propuesto)

- **F1 (código, mayor palanca): B1 batching** — recolectar todas las quotes V3 del
  tick y despacharlas en UN multicall3 (el kernel `v3_quote_exact_in_multicall` YA
  soporta batches; el caller pasa vec de 1). ~40x menos eth_call.
- **F2 (config/env): sanear `RPC_HTTP_1`** — quitar 1rpc (410) y flashbots/mevblocker
  del pool del quoter (relays); mantener drpc/publicnode/0xrpc/blockpi/llama.
- **F3 (operador, decisión de costo): Alchemy quota** — activar PAYG o resolver el
  429 de la key; es el único proveedor con headroom para el quoter.
- **F4 (observabilidad): exportar `arbx_rpc_provider_state` para TODOS los
  proveedores del pool** (hoy 4/9 invisibles) + alerta si >50% Open.

Sin F3, F1+F2 probablemente bastan para que los públicos respiren; F3 da margen.
