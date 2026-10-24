# A4b PRICES — precios con procedencia para los 6 tokens CLEAN

Fecha: 2026-10-23 19:04–19:07 UTC · Autor: integrador-2 (t17, attempt f5686fc5) · Repo: hefarica/arbitragex-v2
Naturaleza: **read-only**. Cero escrituras en DB y en config; `paper_mode` intacto; rutas protegidas intactas; allowlist **no** tocada. El snippet de §5 es una **propuesta**, no se aplicó.

## 0. Corrección de premisa (importante para la decisión)

A4 concluyó que "ninguno de los 11 tokens tiene precio en `trading_config.token_prices_usd` → el pipeline los rechazaría en `gate_unknown_token_price`". Eso es **correcto sólo para la SEGUNDA fuente**. La torre de precios soberana —la PRIMERA fuente, `arbx:token_prices:1` en Redis, TTL 52 s, publicada por el `price_worker`— **YA tiene los 6 tokens CLEAN** (y también BST, KISHU, PHA, DSYNC, XAUT). Verificado en vivo:

```
$ ssh arbx 'docker exec arbitragex-v2-redis-1 redis-cli HGETALL arbx:token_prices:1 | paste - -'
ZIG	0.04531        NPC	0.0178         BOOE	0.03783
CAW	0.00000002356  BGB	1.9            YEE	0.003749
... (≈500 símbolos en el hash)
$ redis-cli TTL arbx:token_prices:1 → 52
```

Y el orden de fuentes en el hot path es explícito — `backend/searcher-rs/src/cartridge_boot.rs:2367-2399`:

```rust
let from_snapshot = price_snapshot.get(&symbol.to_ascii_uppercase()).copied()…   // (1) torre Redis
let from_config   = cfg.token_prices_usd.iter().find(|(k,_)| k.eq_ignore_ascii_case(symbol))…  // (2) trading_config
let from_bus      = price_bus.and_then(|bus| bus.view().price_with_verdict(symbol))…          // (3) PriceBus
let Some((usd, evidence_id)) = from_snapshot.or(from_config).or(from_bus) …
```

Consecuencia: **el gate `unknown_token_price` NO bloquea a estos 6 tokens hoy** — la torre los cubre y gana por precedencia. El cuello de botella sigue siendo la **allowlist** (y después el economics), no el precio. El snippet de §5 sirve como **respaldo curado** (si el `price_worker` cae o el token sale del top del hash), no como desbloqueo del gate.

## 1. Fuentes y timestamps (todas gratuitas / stack soberano)

| Fuente | Endpoint / acceso | Timestamp de la lectura |
|---|---|---|
| Torre soberana (price_worker → Redis) | `HGETALL arbx:token_prices:1` (TTL 52 s) | 2026-10-23T19:04Z |
| CoinGecko (free) | `/api/v3/simple/token_price/ethereum?contract_addresses=<addr>&vs_currencies=usd&include_last_updated_at=true` | `last_updated_at` = 1792782379 (**19:06:19Z**); YEE 1792782386 (**19:06:26Z**) |
| Bitget (público, para BGB porque CoinGecko dio 429) | `/api/v2/spot/market/tickers?symbol=BGBUSDT` | `ts` = 1792782416928 (**19:06:56.928Z**), `lastPr=1.9047` |
| DexScreener (free) | `/latest/dex/tokens/<6 addrs>` (precio + reservas del par) | fetch 19:06Z (marca openGraph 1792782300000 = 19:05:00Z) |
| Binance (chequeo de cobertura del bus) | `/api/v3/ticker/price?symbol=ZIGUSDT` y `BGBUSDT` | 19:07Z → `-1121 Invalid symbol` (no listados en spot) |
| WETH de referencia | torre: `WETH = 2396.8664318628003` | 19:04Z |

## 2. Tabla principal — precio por token, fuente, sanity

`V2 implícito` = reservas del par UniswapV2 principal (quote WETH × WETH_usd de la torre ÷ base token), calculado desde las reservas que reporta DexScreener para el par.

| Token | Torre (Redis) | 2.ª fuente (market) | V2 implícito | Δ torre vs 2.ª fuente | Δ implícito vs torre | Liquidez V2 |
|---|---|---|---|---|---|---|
| **ZIG** `0xb2617246…9ca94f01` | 0.04531 | 0.04513582414082261 (CoinGecko, ethereum/ZIG) | 0.04508631843 | +0.39 % | −0.49 % | $44,766 |
| **NPC** `0x8ed97a63…dc05408f6` | 0.0178 | 0.017717797024581722 (CoinGecko) | 0.01775385293 | +0.46 % | −0.26 % | $4,487,355 |
| **BOOE** `0x289ff002…2f9540a6` | 0.03783 | 0.037610719891737615 (CoinGecko) | 0.03765288481 | +0.58 % | −0.47 % | $493,522 |
| **CAW** `0xf3b9569f…33ebe452` | 0.00000002356 | 2.34955085010686e-08 (CoinGecko) | 2.35001553e-08 | +0.27 % | −0.25 % | $1,747,087 |
| **BGB** `0x54d22527…8ff90581` | 1.9 | 1.9047 (Bitget BGBUSDT) | 1.902841504 | −0.25 % | +0.15 % | $455,723 |
| **YEE** `0x9ac9468e…92c77fd` | 0.003749 | 0.0037436574179419376 (CoinGecko) | 0.003735874378 | +0.14 % | −0.35 % | $460,898 |

**Sanity**: desvío máximo torre vs segunda fuente **0.58 %**; desvío máximo precio implícito del par vs torre **0.49 %**. Nada fuera de la banda de ruido de mercado/redondeo de reservas → los precios son consistentes entre tres fuentes independientes. **Ningún token quedó NO COMPUTABLE.**

Notas de procedencia por token:
- **ZIG**: CoinGecko id `zignaly` (rebrand a "ZIG Finance"), `market_cap_rank` 384; torre = DexScreener al decimal mostrado.
- **NPC**: CoinGecko `non-playable-coin`; par V2 con $4.49 M de liquidez (el más profundo de los 6).
- **BOOE**: CoinGecko `book-of-ethereum`.
- **CAW**: CoinGecko `a-hunters-dream`; el par V2 (0x48d2…) mueve $1.75 M.
- **BGB**: Bitget es la plaza natural del token (BitgetToken); CoinGecko devolvió **HTTP 429** (rate limit gratuito) — no se reintentó en bucle ni se inventó el número: se usó la API pública de Bitget.
- **YEE**: el símbolo on-chain es "Yee"; la torre publica la clave **`YEE`** (mayúsculas) — coincide con el lookup `symbol.to_ascii_uppercase()` del searcher.

## 3. Por qué el bus en proceso (Binance WS) no cubre a estos 6

`GET https://api.binance.com/api/v3/ticker/price?symbol=ZIGUSDT` → `{"code":-1121,"msg":"Invalid symbol."}`; ídem `BGBUSDT`. Ninguno de los 6 tiene par spot USDT en Binance, así que la tercera fuente (PriceBus = Binance WS ⊕ anchors Chainlink, `cartridge_boot.rs:2281-2288`) **no puede valuarlos**. La valuación efectiva depende de la **torre** (fuente 1) y, como respaldo, de `trading_config.token_prices_usd` (fuente 2). Esto refuerza que el snippet de §5 tiene valor real de redundancia.

## 4. Qué habilita y qué NO habilita este trabajo

- **Habilita**: que el economics pueda valuar los 6 tokens (todos con precio > 0, tres fuentes concordantes), y que un reinicio/caída del `price_worker` no deje a estos tokens sin precio si se carga el respaldo curado.
- **NO habilita**: ni la allowlist (sigue siendo decisión del operador tras economics verificado) ni el `status='executed'` (sigue bloqueado por `paper_mode` + flujo de ejecución, ver `docs/rea/FASE1_GATE.md` §3.3). Tampoco sustituye la verificación de economics: los "brutos" de A4 siguen siendo pre-economics (`computation_status='error'`), y el ROI facial ~99 % sigue siendo un artefacto hasta que el pipeline cotice las piernas.

## 5. Snippet de config PROPUESTO (no aplicado)

Destino formal: `trading_config.token_prices_usd` (jsonb, chain 1) — validado por el zod de `backend/api-server/src/routes/trading-config.ts:150` como `Record<string, number positivo>` (clave 1–16 chars). Valores = torre al momento del snapshot (redondeados a la precisión que publica la torre); **no** son un promedio de fuentes.

```json
{
  "ZIG": 0.04531,
  "NPC": 0.0178,
  "BOOE": 0.03783,
  "CAW": 0.00000002356,
  "BGB": 1.9,
  "YEE": 0.003749
}
```

Si el operador prefiere el valor de la segunda fuente en vez del de la torre, éste es el mapa alternativo (mismo formato, también sin promediar):

```json
{
  "ZIG": 0.04513582,
  "NPC": 0.01771780,
  "BOOE": 0.03761072,
  "CAW": 0.00000002349551,
  "BGB": 1.9047,
  "YEE": 0.00374366
}
```

Advertencia de doctrina: un precio curado **caduca**. Si se carga, debe reemplazarse por la torre viva o refrescarse con una política explícita (p.ej. re-medición diaria); un valor estático de 2026-10-23 no debe quedar como verdad permanente. El `token_prices_usd` actual mezcla ambas cosas (21 símbolos curados) — cargar 6 más no empeora, pero tampoco resuelve la frescura.

## 6. Reproducibilidad (comandos literales)

```bash
# torre soberana (VPS, read-only)
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli HGETALL arbx:token_prices:1 | paste - - | sort"
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli TTL arbx:token_prices:1"

# CoinGecko free (1 address por llamada en el plan gratuito)
curl "https://api.coingecko.com/api/v3/simple/token_price/ethereum?contract_addresses=0xb2617246d0c6c0087f18703d576831899ca94f01&vs_currencies=usd&include_last_updated_at=true&precision=full"

# Bitget público (BGB)
curl "https://api.bitget.com/api/v2/spot/market/tickers?symbol=BGBUSDT"

# DexScreener (precio + reservas del par)
curl "https://api.dexscreener.com/latest/dex/tokens/0xb2617246d0c6c0087f18703d576831899ca94f01,0x8ed97a637a790be1feff5e888d43629dc05408f6,0x289ff00235d2b98b0145ff5d4435d3e92f9540a6,0xf3b9569f82b18aef890de263b84189bd33ebe452,0x54d2252757e1672eead234d27b1270728ff90581,0x9ac9468e7e3e1d194080827226b45d0b892c77fd"

# cobertura del bus
curl "https://api.binance.com/api/v3/ticker/price?symbol=ZIGUSDT"   # -1121
```

## 7. Huecos declarados (R8)

1. **CoinGecko 429 en BGB**: se usó Bitget (fuente gratuita y plaza natural del token). No se promedió ni se inventó.
2. **Reservas de DexScreener redondeadas**: el precio implícito tiene error de redondeo (por eso las desviaciones de ~0.3–0.5 %, no de 0.00 %). No se usó para operar, sólo para sanity.
3. **Anchors Chainlink por token no verificados uno a uno**: no se localizó feed USD público para estos 6; el `price_with_verdict` del bus puede devolver `stale_anchor`/`NoSource`, que es precisamente por qué la torre (fuente 1) manda. Declarado, no asumido.
4. **Torre leída una sola vez** (TTL 52 s): los valores son un snapshot de 19:04Z, no un TWAP. El precio vivo cambia.
5. **La segunda fuente (CoinGecko/Bitget) es spot de exchange/CG**, no el mismo mercado del par: se usa como control, no como verdad operativa.
6. **No se aplicó nada**: cero escrituras en `trading_config`, cero configs, cero allowlist.
