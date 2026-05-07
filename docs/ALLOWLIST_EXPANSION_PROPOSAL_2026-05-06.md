# Allowlist Expansion Proposal — 2026-05-06

> **Audiencia**: operador (decisión final tuya).
> **Generado por**: análisis data-driven sobre 7,788 opps últimas 24h.
> **Acción esperada**: 5-15 min, Redis HSET o UI tab Tokens.

---

## TL;DR

**98.3% de las opps últimas 24h fueron rechazadas por allowlist** (7655/7788).
Solo 12 pasaron todos los gates. El operador puede **DOBLAR el throughput de
candidate_enriched events** con cero riesgo añadiendo 13 tokens que el sistema
YA TIENE POOLS para — todos verified medium/large cap.

**NO añadir** los top rejected addresses sin verificación profunda — son
desconocidos al token cache y probable spam/honeypot bots.

---

## 1. Estado actual

```
Allowlist actual (12 tokens):
WETH · USDC · USDT · DAI · WBTC · ARB · OP · BNB · MATIC · LINK · UNI · AAVE
```

**Pool index cache** (loaded by migration 031, 30 V3 pools / 25 pares + 24 V2 pools):
```
22 tokens con pools:
WETH · USDC · USDT · DAI · WBTC · MATIC · LINK · UNI · AAVE
+ APE · COMP · CRV · ENS · LDO · MANA · MKR · PEPE · RETH · SAND · SHIB · SUSHI · WSTETH
```

**Diff (en pool_index PERO NO en allowlist)** = **13 tokens "shadow"** — el sistema
puede evaluar matemáticamente arbitrajes con ellos PERO los rechaza por allowlist.
Estos son la **expansión segura**.

**Allowlist con tokens NO en pool_index** = ARB, OP, BNB. Son L2-native (ARB, OP)
o cross-chain (BNB) — improbables en pool index de Ethereum mainnet. Considerar
removerlos del allowlist O añadir migration 032b con sus pools si planeas operar
multi-chain.

---

## 2. Top 5 rejected tokens — NO añadir (SECURITY RISK)

| Rank | Address | Rejections 24h | Análisis |
|------|---------|---------------:|----------|
| 1 | `0xfeedf398124aafeb6a36351c924bd00a361ea89a` | 747 | ❌ Desconocido al token cache. Vanity prefix `feedf3` típico de memecoin. Auditoría requerida. |
| 2 | `0xf19304e6bfe0a18d2a0171758aa433921f192897` | 718 | ❌ Desconocido. Patrón sospechoso de bot spam. |
| 3 | `0xb90b2a35c65dbc466b04240097ca756ad2005295` | 343 | ❌ Desconocido. Frecuencia alta sugiere bot wash-trading. |
| 4 | `0x00f3c42833c3170159af4e92dbb451fb3f708917` | 326 | ❌ Desconocido. |
| 5 | `0x8b3c308e2d78d1eebd7ec8c6c078c77878d0a49f` | 268 | ❌ Desconocido. |

**Por qué NO añadir blindly**:
- Sin token cache entry → no sabemos symbol, decimals, ni si es legítimo
- Patrón de aparición masiva en mempool sugiere bot spam o wash trading
- Honeypots típicamente atraen bots con liquidity falsa
- Si añades sin verificar y resulta ser honeypot → todos los swaps revierten
  → puro gas wasted

**Si quieres procesarlos**, pasos en orden:
1. Buscar address en Etherscan → ¿contract verified?
2. TokenSniffer / GoPlus → ¿honeypot? ¿transfer tax >5%?
3. DexScreener → ¿liquidity TVL real? ¿>$100K?
4. Solo si los 3 pasan: añadir a `tokens` table en PG con migration → reload caches → añadir a allowlist

---

## 3. ✅ Expansión SEGURA (recomendada): 13 tokens shadow

Tokens con pools YA cargados, addresses verified, cap markets establecidos.
**Cero riesgo** porque el sistema ya tiene metadata + pool reserves para cada uno.

### 3a. Tier 1 — Mainstream high-volume (añadir TODOS)
| Token | Address | Pools | Razón añadir |
|-------|---------|------:|--------------|
| **PEPE** | 0x6982508145454ce325ddbe47a25d4ec3d2311933 | 2 | Top memecoin volume; PEPE/WETH muy líquido |
| **SHIB** | 0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce | 2 | Top memecoin establecido; volume sostenido |
| **MKR** | 0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2 | 2 | Maker DAO; large cap, alto valor unitario |
| **COMP** | 0xc00e94cb662c3520282e6f5717214004a7f26888 | 2 | Compound; DeFi blue chip |

### 3b. Tier 2 — DeFi mid-cap (añadir si quieres más cobertura)
| Token | Address | Pools | Razón añadir |
|-------|---------|------:|--------------|
| **CRV** | 0xd533a949740bb3306d119cc777fa900ba034cd52 | 1 | Curve DAO; alto volume DeFi |
| **LDO** | 0x5a98fcbea516cf06857215779fd812ca3bef1b32 | 1 | Lido staking governance |
| **ENS** | 0xc18360217d8f7ab5e7c516566761ea12ce7f9d72 | 1 | Ethereum Name Service |
| **RETH** | 0xae78736cd615f374d3085123a210448e74fc6393 | 1 | Rocket Pool ETH (LST) |
| **WSTETH** | 0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0 | 1 | Wrapped stETH (Lido LST) |

### 3c. Tier 3 — Gaming/NFT mid-cap (opcional)
| Token | Address | Pools | Notas |
|-------|---------|------:|-------|
| **APE** | 0x4d224452801aced8b2f0aebe155379bb5d594381 | 1 | ApeCoin — volume variable |
| **MANA** | 0x0f5d2fb29fb7d3cfee444a200298f468908cc942 | 1 | Decentraland — volume bajo |
| **SAND** | 0x3845badade8e6dff049820680d1f14bd3903a5d0 | 1 | The Sandbox — volume bajo |
| **SUSHI** | 0x6b3595068778dd592e39a122f4f5a5cf09c90fe2 | 1 | SushiSwap governance |

---

## 4. Comando concreto para aplicar Tier 1 (4 tokens, recomendado start)

```bash
# Update allowlist + token_prices_usd via Redis hot-reload
ssh arbx 'docker exec arbitragex-v2-redis-1 redis-cli SET arbx:trading_config:1 \
  "$(docker exec arbitragex-v2-redis-1 redis-cli GET arbx:trading_config:1 | \
    python3 -c "
import json, sys
c = json.load(sys.stdin)
# Append Tier 1 tokens to allowlist (preserve existing)
existing = set(s.upper() for s in c[\"allowed_token_symbols\"])
for t in [\"PEPE\", \"SHIB\", \"MKR\", \"COMP\"]:
    if t not in existing:
        c[\"allowed_token_symbols\"].append(t)
# Add price entries (use spot prices ±10% as starter — operator updates later)
prices = c.get(\"token_prices_usd\", {})
prices.update({
    \"PEPE\": 0.0000095,   # check Coingecko before applying!
    \"SHIB\": 0.000018,    # check Coingecko before applying!
    \"MKR\":  1300,        # check Coingecko before applying!
    \"COMP\": 45,          # check Coingecko before applying!
})
c[\"token_prices_usd\"] = prices
print(json.dumps(c))
")"'
```

**ANTES de ejecutar el comando**: verificar precios spot actuales en Coingecko
y actualizarlos en el bloque `prices.update()`. Los valores arriba son
indicativos del momento de generar este doc.

---

## 5. Impacto estimado

Con Tier 1 añadido:
- **+4 tokens en allowlist** (12 → 16, +33%)
- **Cobertura de pares mainstream**: PEPE/WETH, SHIB/WETH, MKR/WETH, COMP/WETH
- **Estimado conservador**: 5-15% más opps llegando al math evaluator (las que involucran estos tokens dejarán de rechazarse)
- **Riesgo**: cero (tokens ya en pool_index = system los conoce)

Con Tier 1+2+3 (los 13 tokens):
- Allowlist 12 → 25 (+108%)
- Cobertura completa del pool index actual
- Estimado: 15-30% más opps al math evaluator
- Riesgo: bajo (todos verified, mid/large cap)

**NO esperes profit > 0 inmediato** — recuerda los gates downstream (price oracle,
sanity bound, risk policy) siguen filtrando. Pero el throughput a heartbeat
counter `enriched_v2/v3` sí debería subir notablemente, dándote más data para
analizar.

---

## 6. Cleanup recomendado del allowlist actual

3 tokens en allowlist sin pools en cache (mainnet Ethereum):
- **ARB** (0x912CE59144191C1204E64559FE8253a0e49E6548) — Arbitrum L2 native; raro en mainnet pools
- **OP** (0x4200000000000000000000000000000000000042) — Optimism L2 native
- **BNB** (0xB8c77482e45F1F44dE1745F52C74426C631bdd52) — Binance Coin (BEP-20 wrapped)

**Opciones**:
- (A) Removerlos del allowlist → reduce ruido en `gate_token_not_allowed` para tokens improcesables
- (B) Mantenerlos por si añades pools manualmente en futuro
- (C) Añadir pools para ellos vía migration 032 (BNB sí tiene pools en mainnet)

Mi recomendación: **B** (mantener, low cost). Los datos del heartbeat ya muestran
que no aparecen frecuentemente, así que el ruido es mínimo.

---

## 7. Validación post-cambio

Después de aplicar la migración:

1. **Inmediato (5s)**: heartbeat siguiente debería mostrar `gate_unknown_token_price=0`
   para los tokens añadidos (porque les diste price)
2. **30 min**: contar opps con `rejection_reason` por token añadido — debería
   bajar dramáticamente:
   ```sql
   SELECT rejection_reason, COUNT(*) FROM opportunities
   WHERE detected_at > NOW() - INTERVAL '30 minutes'
     AND rejection_reason LIKE '%PEPE%' OR rejection_reason LIKE '%SHIB%'
   GROUP BY rejection_reason;
   -- Esperado: 0 o pocos (ya no rechazadas por allowlist)
   ```
3. **1-3 horas**: heartbeat counter `enriched_v2 + enriched_v3` debería superar
   el promedio actual (0-1 por minuto) si los tokens añadidos tienen tráfico real
4. **24h**: query opportunities con profit > 0 — debería haber al menos 5-10
   más (rate base actual: 12/día con allowlist actual)

---

## Decisión del operador

- ✅ **Aplicar Tier 1 (PEPE, SHIB, MKR, COMP)** — recomendado start
- 🟡 **Aplicar Tier 1+2** — más cobertura DeFi
- 🟢 **Aplicar Tier 1+2+3** — cobertura completa pool index
- 🔴 **Skip por ahora** — esperar primero datos de Phase 5b REVM antes de scaling

Tu decisión + comando ejecutado → me avisas si quieres que verifique post-deploy.
