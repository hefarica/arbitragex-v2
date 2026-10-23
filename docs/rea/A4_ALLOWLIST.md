# A4 ALLOWLIST — screening de los tokens de las filas `TokenNotAllowed`

Fecha: 2026-10-23 (18:0x–18:3x UTC) · Autor: integrador-2 (t14, attempt 8ba18c30) · Repo: hefarica/arbitragex-v2
Naturaleza: **read-only**. No se tocó `risk/tokens.toml` ni ninguna config, no hubo escrituras en DB, `paper_mode` intacto, rutas protegidas intactas. La allowlist la decide el operador; acá solo hay evidencia y una recomendación citable.

## 0. Resumen y decisión

Se screeneron los **11 tokens** que hoy concentran las filas brutas-positivas muertas por `TokenNotAllowed` (59 filas, $14,848.42 de bruto no verificado, máximo $1,272.10). Resultado: **6 CLEAN, 2 RISKY, 3 BLOCKED**.

**Decisión recomendada: NO expandir la allowlist todavía.** La allowlist no es el cuello de botella: (a) los 11 tokens **no tienen precio** en `trading_config.token_prices_usd`, así que el pipeline los rechazaría en el gate siguiente (`gate_unknown_token_price`); (b) su `economics` está en `computation_status='error'` — el "bruto" es un número sin verificar y el ROI implícito a valor facial es de **~99 % mediano** (imposible para arbitraje), o sea que la cifra que motiva la expansión es un artefacto; (c) aun tomando el bruto al pie de la letra, **30 de 59 filas no llegan al piso de $50** y 6 filas ≥ $1,000 no caben en el capital de $1,000. Primero se arreglan los insumos (precio + economics), después se decide allowlist, y recién entonces se discute A2 (pisos).

## 1. Población medida y drift

Definición operativa (reproducible en §7):

```
opportunities
WHERE rejection_reason LIKE 'TokenNotAllowed:%'
  AND expected_profit_usd > 0
  AND detected_at > now() - interval '24 hours'
```

Snapshot 2026-10-23 18:0x UTC: **59 filas · $14,848.42 · máximo $1,272.10 · 11 tokens**.
La base citada por el capitán (54 filas · $13,657.61 · máximo $1,272.10 · 12 tokens) es **la misma población medida un rato antes**: es una ventana deslizante de 24 h y el productor sigue insertando filas (54 → 55 → 57 → 59 en ~70 min). El máximo coincide exacto ($1,272.10), lo que confirma la definición. **El 12.º token no está en este snapshot**: rotó fuera de la ventana; los 11 de acá cubren el 100 % de las filas y del bruto del snapshot. Drift declarado, no ocultado.

## 2. Método y límites (skill `arbx-token-safety-screen` cargada)

Se aplicaron las verificaciones de la skill con las fuentes accesibles sin llaves ni web3 con address operativa:

- **GoPlus Token Security API** (gratuita, keyless): `https://api.gopluslabs.io/api/v1/token_security/1?contract_addresses=<addr>` → honeypot, buy/sell/transfer tax, open source, proxy, owner, blacklist, pausable, mintable, holders, LP holders (lock).
- **honeypot.is v2 API** (gratuita, fork-sim hosted): `https://api.honeypot.is/v2/IsHoneypot?address=<addr>&chainID=1` → simulación de compra/venta con taxes reales, análisis de holders (cuántos pagaron tax), proxy, liquidez del par y reservas.
- **Lecturas on-chain** (`cast call` con RPC público `https://ethereum-rpc.publicnode.com`): `symbol()` de control (anti-spoof) y sondas `paused()`/`admin()`; bloque 26,257,054 (2026-10-23 18:15:11 UTC), `baseFeePerGas = 163,744,749 wei ≈ 0.164 gwei`.
- Precio ETH de referencia: `trading_config.base_token_price_usd = 2405.2052365740005`; pisos vivos leídos de `trading_config` (chain 1): `capital_usd=1000`, `min_profit_usd=50`, `min_roi_pct=0.30`, `max_slippage_pct=0.005`, `flashloan_fee_pct=0.0009`, `ops_overhead_usd_per_attempt=0.01`, `failure_risk_buffer_pct=0.0004`.

**Límites honestos (R8)**:
- Checks 2/3/4/5 (transfer/fork-sim con *nuestra* address, blacklist contra *nuestra* address, pause/freeze en storage) **no se corrieron con la address operativa** (no hay web3 con signer ni fork-sim local en este host). Se sustituyen por la simulación de terceros (honeypot.is) y los flags de GoPlus, que miden lo mismo sobre una address de prueba **pero no prueban la nuestra**.
- `admin()` del proxy de XAUt **no fue legible** con las herramientas disponibles (EIP-1967 storage no expuesto por `cast call`); por eso XAUt queda RISKY (skill #7 exige admin timelock/renounced **verificado**).
- `transfer()` retorna bool (check #8) no fue medido token por token: ninguno de los 11 está marcado como non-standard por las fuentes; el flag operativo `requiresSafeERC20` queda **no computado**.

## 3. Tabla principal: token | verdict | evidencia | net estimado | recomendación

Net estimado = **a valor facial** (bruto tal como lo publicó el productor), con costos modelados (gas ≈ $1.00 = 250k unidades a ~1.16 gwei + tip, flash 0.09 % del tamaño, buffer 0.04 %, ops $0.01). **El net real por fila es NO COMPUTADO** (ver §4): el bruto no está verificado.

| Token | Verdict | Evidencia literal (hora UTC) | Net estimado (facial, filas que pasan pisos) | Recomendación |
|---|---|---|---|---|
| **ZIG** `0xb2617246…9ca94f01` | **CLEAN** | GoPlus: `is_honeypot:0, buy_tax:0, sell_tax:0, transfer_tax:0, is_open_source:1, is_proxy:0, is_blacklisted:0, holder_count:16721, lp_holder_count:6` · honeypot.is: `"risk":"low","riskLevel":1,"flags":[],isHoneypot:false,buyTax:0,sellTax:0,transferTax:0`, holderAnalysis `5358/5358 success`, par `Uniswap V2: ZIG-WETH` `liquidity:44903.75` | $6,216.79 (14 filas) | **Allowlist candidato #1** *con tope de tamaño*: GoPlus muestra **99.2 % del LP en una sola EOA sin lock** (`percent 0.99198, is_locked 0`) → riesgo de retiro de liquidez (no es check de honeypot, es riesgo operativo). |
| **XAUt** `0x68749665…a622782f38` | **RISKY** | GoPlus: `is_honeypot:0, taxes 0, is_open_source:1, is_proxy:1, holder_count:63681` · honeypot.is: `"risk":"low"`, `isProxy:true, hasProxyCalls:true`, `liquidity:4096094.91` · `cast call admin()(address)` → sin retorno legible (proxy admin **no verificado**) | $1,112.49 (1 fila) | **Solo con verificación del proxy admin** (timelock o renounced). Si no, excluir (skill #7: admin EOA viva = veto). Tether Gold, liquidez profunda. |
| **NPC** `0x8ed97a63…dc05408f6` | **CLEAN** | GoPlus: `is_honeypot:0, taxes 0, is_open_source:1, is_proxy:0, owner_address:0x000…000, holder_count:20634`, LP `TeamFinance is_locked:1` 99.99 % · honeypot.is: `"risk":"low"`, holderAnalysis `4551/4551`, `liquidity:4493000.37` | $0 (1 fila, pero **no pasa el piso de capital**: $1,633.93 > $1,000) | **Allowlist candidato #2**; la fila concreta muere por capital, no por token. |
| **DSync** `0xf94e7d07…d3f91cc` | **BLOCKED** | GoPlus: `is_blacklisted:1, slippage_modifiable:1, external_call:1, owner_address:0x9ce77c… (0.1038 %)` · honeypot.is `holderAnalysis`: `averageTax:2.7466, highestTax:5.79`, `taxDistribution: [{tax:0,count:4501},{tax:4,count:5326},{tax:5,count:164}]` → **5,490 wallets pagaron 4–5.79 %** | $0 (no recomendado) | **No allowlist**. Blacklist + tax condicional por wallet (exactamente el "tax que solo se activa para algunos" de la skill). |
| **BST** `0x509a38b7…ec7c7f4a` | **BLOCKED** | GoPlus: `transfer_pausable:1, hidden_owner:1, is_mintable:1, owner_address:0xd09671…, holder_count:4449` · honeypot.is: `"risk":"low"`, taxes 0, `liquidity:42113.03` | $208.32 (2 filas, a valor facial) | **No allowlist** (skill #5 pause + #6 mint). Si el operador quiere, override firmado con fecha de re-review. |
| **KISHU** `0xa2b4c0af…d625817d` | **BLOCKED** | honeypot.is: `buyTax:1.99, sellTax:1.88, transferTax:1.99`, `taxDistribution: [{tax:0,count:646},{tax:1,count:5849}]` · GoPlus: `buy_tax:0.02, sell_tax:0.0177, is_anti_whale:1` | $167.33 (2 filas) | **No allowlist**: tax-on-transfer viola `amount_received == amount_sent` (skill #3); cada hop pierde ~2 %. |
| **PHA** `0x6c5ba916…c9d52f4e` | **RISKY** | GoPlus: `transfer_pausable:1, hidden_owner:1, is_mintable:0, owner_address:0xb7687a… (0.0065 %)` · honeypot.is: `"risk":"low"`, taxes 0, `liquidity:370953.80`, holderAnalysis `1058/1058`; listado en Binance (`is_in_cex:["Binance"]`) | $0 (2 filas; ambas < $50) | **Override firmado o esperar**: pausable + hidden owner son fallos duros de la skill, aunque el resto esté limpio. |
| **BOOE** `0x289ff002…2f9540a6` | **CLEAN** | GoPlus: `is_honeypot:0, taxes 0, is_proxy:0, owner_address:0x000…000, is_mintable:0`, LP `TeamFinance is_locked:1` 99.96 % · honeypot.is: `"risk":"low"`, `2318/2318`, `liquidity:493605.56` | $0 (3 filas; todas < $50) | **Allowlist candidato #3**, sin urgencia: sus filas mueren por el piso de $50. |
| **CAW** `0xf3b9569f…33ebe452` | **CLEAN** | GoPlus: `is_honeypot:0, taxes 0, is_proxy:0`, LP **99.9957 % en `0x…dead` (locked)** · honeypot.is: `"risk":"low"`, `2280/2280`, `liquidity:1753682.50` | $0 (2 filas; < $50) | **Allowlist candidato #4** (LP quemado). |
| **BGB** `0x54d22527…8ff90581` | **CLEAN** | GoPlus: `is_honeypot:0, taxes 0, is_proxy:0`, holder top `0x…dead 54.6 %` · honeypot.is: `"risk":"low"`, `2287/2287`, `liquidity:455989.13` | $0 (2 filas; < $50) | **Allowlist candidato #5** (Bitget token). |
| **Yee** `0x9ac9468e…92c77fd` | **CLEAN** | GoPlus: `is_honeypot:0, taxes 0, is_proxy:0, owner_address:0x000…000, is_anti_whale:1`, LP 99.9999 % en dead · honeypot.is: `"risk":"low"`, `3759/3759`, `liquidity:460900.50` | $0 (1 fila; < $50) | **Allowlist candidato #6**, con nota del flag anti-whale (limita tamaño, no bloquea). |

**Anti-spoof**: en los 11 casos el par que reportan las fuentes es `<token>/WETH` (o `XAUt/USDT`), y `token_addresses` del `route_metadata` de las filas coincide con la address screeneada; `symbol()` on-chain de control devolvió el símbolo esperado (p.ej. ZIG `"ZIG"`). No se detectó spoof de pool.

## 4. Neto REAL con los pisos actuales (A2) — y por qué no existe todavía

Pisos vivos (`trading_config`, chain 1): `min_profit_usd=50`, `min_roi_pct=0.30`, `capital_usd=1000`.

Para aplicar el piso de ROI y el de capital hace falta el tamaño en USD. Se derivó con el **precio implícito del par V2** (reservas WETH × precio ETH / reservas token, medidas hoy por honeypot.is). Resultado sobre las 59 filas:

| Token | Filas | Bruto facial | ≥ $50 | amount_in ≤ $1000 | Pasan AMBOS (facial) | ROI facial mediano | Net facial (pasantes) |
|---|---|---|---|---|---|---|---|
| ZIG | 33 | $12,236.13 | 19 | 28 | 14 | 1.0382 (104 %) | $6,216.79 |
| XAUt | 1 | $1,116.70 | 1 | 1 | 1 | 1.2072 (121 %) | $1,112.49 |
| NPC | 1 | $610.33 | 1 | 0 | 0 | 0.3735 | $0 |
| DSync | 2 | $396.90 | 2 | 2 | 2 | 0.8366 | $390.27 |
| BST | 10 | $258.17 | 2 | 10 | 2 | 0.6489 | $208.32 |
| KISHU | 2 | $173.61 | 2 | 2 | 2 | 0.8603 | $167.33 |
| PHA | 2 | $44.79 | 0 | 2 | 0 | 0.0568 | $0 |
| BOOE | 3 | $8.43 | 0 | 3 | 0 | 0.0167 | $0 |
| CAW | 2 | $3.27 | 0 | 2 | 0 | 0.2932 | $0 |
| BGB | 2 | $0.06 | 0 | 2 | 0 | 0.1288 | $0 |
| Yee | 1 | $0.02 | 0 | 1 | 0 | 0.0064 | $0 |
| **Total** | **59** | **$14,848.42** | **27** | **53** | **21** | **0.9878** | **$8,095.20** |

Lecturas que importan:

1. **El bruto no es beneficio**: `economics.computation_status='error'` en el 100 % de las filas; `net_profit_usd`, `total_cost_usd`, `gas_usd`, `slippage_usd` y `legs` están en `null`. El ROI implícito mediano es **98.8 %** y el máximo 121 %: un arbitraje no rinde eso. El "bruto" es una estimación pre-economics, no un resultado. **Net real por fila: NO COMPUTADO (R8)** — el $8,095.20 es un techo facial, no una expectativa.
2. **El piso de capital muerde**: 6 filas ≥ $1,000 de bruto implican tamaños (o ROIs) incompatibles con `capital_usd=$1000` — incluida la fila top de ZIG ($1,272.10 de bruto sobre 28,289 ZIG ≈ $1,293 de tamaño).
3. **El piso de profit muerde**: 30/59 filas < $50 mueren ahí aunque el token esté allowlisted (PHA, BOOE, CAW, BGB, Yee: 0 filas pasan).
4. **El piso de ROI no es el binding**: con estos tamaños, cualquier bruto ≥ $50 sobre ≤ $1,000 da ROI ≥ 5 %, muy por encima del 0.30 %.
5. **Falta el insumo, no el permiso**: ninguno de los 11 tokens tiene entrada en `trading_config.token_prices_usd` (las claves son APE, CRV, DAI, ENS, LDO, MKR, UNI, AAVE, COMP, LINK, MANA, PEPE, RETH, SAND, SHIB, USDC, USDT, WBTC, WETH, MATIC, SUSHI). Sin precio, el pipeline rechaza con `gate_unknown_token_price` antes de que la allowlist importe.

**Respuesta a A2**: expandir la allowlist **no basta** y hoy no es la decisión correcta. El orden que sí mueve la aguja: (1) precio por token en `trading_config`; (2) que `economics` compute (leg quotes reales) para que el bruto deje de ser un artefacto; (3) re-medir; (4) recién ahí allowlist y pisos con números verificados.

## 5. Recomendación ordenada (para el operador; la allowlist la decide él)

1. **P0 — No allowlist todavía.** Sin precio y sin economics verificado, cualquier expansión solo cambia el motivo del rechazo (`TokenNotAllowed` → `gate_unknown_token_price` → economics error).
2. **P1 — Insumos primero**: agregar precio para los 6 CLEAN (ZIG, NPC, BOOE, CAW, BGB, Yee) y re-medir 24 h. Criterio de éxito: `economics.computation_status='ok'` en ≥ 1 fila de cada token y ROI verificado plausible (< 5 %).
3. **P2 — Allowlist de CLEAN con topes**: ZIG primero (es el 82 % del bruto y 56 % de las filas), con tope de tamaño por LP unlocked (99.2 % en una EOA) y cap de capital $1,000. NPC/BOOE/CAW/BGB/Yee después: sus filas actuales no pasan el piso de $50, así que el beneficio esperado es bajo.
4. **P3 — RISKY solo con override firmado** (skill, §Override): XAUt exige verificar el proxy admin; PHA exige aceptar pausable+hidden owner. Fecha de re-review ≤ 30 días.
5. **P4 — BLOCKED nunca por defecto**: DSync (blacklist + tax 4–5.79 %), BST (pausable+mintable+hidden owner), KISHU (tax 2 %).

## 6. Evidencia por token — comandos y salidas literales (resumen)

```
# GoPlus (una llamada por token; ejemplo ZIG)
curl https://api.gopluslabs.io/api/v1/token_security/1?contract_addresses=0xb2617246d0c6c0087f18703d576831899ca94f01
{"0xb261…":{"is_honeypot":"0","buy_tax":"0","sell_tax":"0","transfer_tax":"0","is_open_source":"1",
 "is_proxy":"0","is_blacklisted":"0","is_mintable":"0","holder_count":"16721","lp_holder_count":"6",
 "lp_holders":[{"address":"0x98a808a1…","percent":"0.991981188885060133","is_locked":0}, …]}}

# honeypot.is (una llamada por token; ejemplo ZIG)
curl "https://api.honeypot.is/v2/IsHoneypot?address=0xb2617246d0c6c0087f18703d576831899ca94f01&chainID=1"
{"summary":{"risk":"low","riskLevel":1,"flags":[]},"honeypotResult":{"isHoneypot":false},
 "simulationResult":{"buyTax":0,"sellTax":0,"transferTax":0},
 "holderAnalysis":{"holders":"5358","successful":"5358","failed":"0","siphoned":"0"},
 "contractCode":{"openSource":true,"isProxy":false},
 "pair":{"pair":{"name":"Uniswap V2: ZIG-WETH","type":"UniswapV2"},"liquidity":44903.75}}

# sonda on-chain (control anti-spoof / pause / proxy admin)
cast call 0xb2617246…9ca94f01 "symbol()(string)" --rpc-url https://ethereum-rpc.publicnode.com  → "ZIG"
cast call 0x509a38b7…ec7c7f4a "paused()(bool)" --rpc-url https://ethereum-rpc.publicnode.com  → sin retorno (no expone paused())
cast call 0x68749665…a622782f38 "admin()(address)" --rpc-url https://ethereum-rpc.publicnode.com → sin retorno legible (proxy EIP-1967 no consultable con estas tools)
```

## 7. Reproducibilidad (SQL literal)

```sql
-- población
SELECT count(*), round(sum(expected_profit_usd),2), round(max(expected_profit_usd),2),
       count(DISTINCT split_part(rejection_reason,':',2))
  FROM opportunities
 WHERE rejection_reason LIKE 'TokenNotAllowed:%' AND expected_profit_usd > 0
   AND detected_at > now() - interval '24 hours';
-- por token
SELECT split_part(rejection_reason,':',2) AS token, count(*),
       round(sum(expected_profit_usd),2) AS bruto, round(max(expected_profit_usd),2) AS maximo
  FROM opportunities
 WHERE rejection_reason LIKE 'TokenNotAllowed:%' AND expected_profit_usd > 0
   AND detected_at > now() - interval '24 hours'
 GROUP BY 2 ORDER BY bruto DESC;
-- insumo que falta: precios configurados
SELECT jsonb_object_keys(token_prices_usd) FROM trading_config WHERE chain_id = 1;
-- pisos vivos
SELECT capital_usd, min_profit_usd, min_roi_pct, max_slippage_pct, flashloan_fee_pct,
       ops_overhead_usd_per_attempt, failure_risk_buffer_pct, base_token_price_usd
  FROM trading_config WHERE chain_id = 1;
```

Acceso: `ssh` (WSL) → `docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -F '|'`. `sql_query` del harness **no ejecuta** (devuelve `ok:true` con data vacía incluso para SQL inválido) — se usó psql real.

## 8. Huecos declarados (no computado ≠ cero)

1. **Net real por fila: NO COMPUTADO.** `economics` en error; sin leg quotes, gas, slippage ni fees por fila. El net facial de §4 es un techo con costos modelados (gas ≈ $1), no una medición.
2. **Checks con address operativa (2/3/4/5) no corridos**: sin web3+signer ni fork local. Se sustituyeron por fork-sim de terceros (honeypot.is) y flags GoPlus, que **no prueban específicamente nuestra address**.
3. **Proxy admin de XAUt no legible** → RISKY por definición de la skill (#7 exige verificación), no por evidencia de admin malicioso.
4. **`requiresSafeERC20` (check #8) no medido** token por token.
5. **Lista negra interna del operador no disponible**: el check #10 se cubrió solo con GoPlus/honeypot.is.
6. **Drift poblacional**: ventana deslizante; 54→59 filas y 12→11 tokens en ~70 min. Los números son del snapshot 18:0x–18:3x UTC.
7. **Precios de token derivados del par V2**: son spot del bloque de la consulta, no TWAP; no se usaron para operar, solo para aplicar los pisos.
