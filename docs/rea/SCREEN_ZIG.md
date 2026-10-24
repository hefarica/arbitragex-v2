# SCREEN-ZIG — checks 2/4/5/8 de `arbx-token-safety-screen` para ZIG (fork anvil, solo lectura)

Fecha: **2026-10-23** · Autor: verificador (t18) · Repo: `hefarica/arbitragex-v2` · Base: `origin/main` @ `3de7c420`.
Insumo directo: `docs/rea/A4_ALLOWLIST.md` (A4 corrió los checks 1,3,6,7,9,10 por GoPlus + honeypot.is y **declaró NO CORRIDOS** los checks 2/3/4/5 *"sin web3+signer ni fork local"*). Esta unidad cierra **2, 4, 5 y 8**.

> **Naturaleza de la evidencia:** todo se ejecutó con `cast call` = `eth_call` contra el fork de mainnet del contenedor `arbitragex-v2-anvil-1`.
> **Cero claves reales** (no se firmó nada), **cero broadcast** (ninguna transacción salió hacia mainnet ni hacia el fork: `eth_call` no persiste estado),
> **cero escrituras** en PG/config, `paper_mode` intacto. Las cuentas usadas son direcciones **públicas**
> (`ARBITRAGE_EXECUTOR`, `SIM_SIGNER_ADDRESS`, el pool, el burn address) — nunca una clave.

---

## 0. Sujetos de la prueba (fuente de cada address)

| Rol | Address | Fuente (citada) |
|---|---|---|
| **Token (ZIG)** | `0xb2617246d0c6c0087f18703d576831899ca94f01` | PG `tokens` (chain 1, `symbol=ZIG`, `decimals=18`, `is_active=t`) — Q1; coincide con A4 (`A4_ALLOWLIST.md:46`) |
| **Nuestra address operativa** | `0x0c82CB749B53cB3433319cd6Be18d746b3781B9B` | env **`ARBITRAGE_EXECUTOR`** de los 4 servicios paper (relays-client, sim-ctl, api-server, searcher-rs) — Q3. Es el contrato que *sostiene y mueve* el token on-chain |
| Executor de flash loan (2ª address operativa) | `0xb47B3294d6B978964700Cb9350f8f8b4ACc39ACE` | env **`FLASHLOAN_EXECUTOR_1`** — Q3 |
| Signer de simulación (paper) | `0x1234567890123456789012345678901234567890` | env **`SIM_SIGNER_ADDRESS`** — Q3 |
| **Holder fondeado** (para originar el transfer) | `0xb36ec83d844c0579ec2493f10b2087e96bb65460` | PG `pools` (`UniswapV2`, fee 30, `ZIG/WETH`, `is_active=t`) — Q2; saldo on-chain 482,060.158 ZIG — §2 |
| Control random | `0x000000000000000000000000000000000000dEaD` | constante pública (burn address); **sí tiene saldo** de ZIG (46,059,208.26) — §2, por eso sirve de control *fondeado* |

**Fork**: `chain-id = 1`, `block-number = 26253631`, `block.timestamp = 1792738091` (`cast chain-id/block-number/latest`). Fork fijo al arranque del contenedor (`anvil --fork-url $ANVIL_FORK_URL`, sin `--fork-block-number` ⇒ tip del proveedor en ese momento).

---

## 1. Tabla de veredicto — checks 2, 4, 5, 8

| # | Check (skill) | Resultado | Comando exacto (abreviado: `C = docker exec arbitragex-v2-anvil-1 cast`, `--rpc-url http://127.0.0.1:8545`) | Salida literal |
|---|---|---|---|---|
| **2** | `transfer`/`transferFrom` no revierten con nuestra address | **PASS** | `C call ZIG "transfer(address,uint256)(bool)" 0x0c82… 1000000000000000000 --from 0xb36e…` → **destino = nuestra address, origen fondeado** | `true` (exit 0) |
| 2a | origen = nuestra address (monto 0 ⇒ aísla el gate de balance) | **PASS** | `C call ZIG "transfer(address,uint256)(bool)" 0xdEaD 0 --from 0x0c82…` | `true` (exit 0) |
| 2b | nuestra address como origen **y** destino | **PASS** | `C call ZIG "transfer(address,uint256)(bool)" 0x0c82… 0 --from 0x0c82…` | `true` (exit 0) |
| 2c | signer de sim (paper) como origen | **PASS** | `C call ZIG "transfer(address,uint256)(bool)" 0xdEaD 0 --from 0x1234…7890` | `true` (exit 0) |
| 2d | `transferFrom` con **recipient = nuestra address** (monto 0 ⇒ aísla allowance) | **PASS** | `C call ZIG "transferFrom(address,address,uint256)(bool)" 0xb36e… 0x0c82… 0 --from 0xdEaD` | `true` (exit 0) |
| 2e | `transferFrom` con **from = nuestra address** | **PASS** | `C call ZIG "transferFrom(address,address,uint256)(bool)" 0x0c82… 0xdEaD 0 --from 0xdEaD` | `true` (exit 0) |
| 2f | `transferFrom` con monto real (sin allowance) — el revert es el gate **estándar**, idéntico para destino nuestro y random | **PASS (gate estándar)** | `C call ZIG "transferFrom(...)" 0xb36e… 0x0c82… 1e18 --from 0xb36e…` **vs** `… 0xdEaD 1e18 …` | ambas: `Error: … execution reverted: ERC20: transfer amount exceeds allowance` (mismo revert, exit 1) |
| **4** | Sin blacklist contra nuestra address | **PASS** | diferencial de destino desde el mismo origen fondeado: `transfer(0xdEaD,1e18)`, `transfer(0x0c82…,1e18)`, `transfer(0x1234…,1e18)` — los tres `--from 0xb36e…` | `true` / `true` / `true` (exit 0 los tres) |
| 4b | ¿existe el hook de blacklist? | **AUSENTE** | `C call ZIG "isBlacklisted(address)(bool)" 0x0c82…` y `"getBlackListStatus(address)(bool)"`; selectores `cast sig` buscados en `cast code` | revert con `data: "0x"` (función inexistente); `isBlacklisted`/`getBlackListStatus`/`blacklist`/`addBlackList` → **hits=0** en el runtime (2,449 bytes) |
| **5** | Sin pause/freeze activado | **PASS** | (i) `C call ZIG "paused()(bool)"`, `"isPaused()(bool)"`, `"frozen()(bool)"`, `"isFrozen(address)(bool)"`; (ii) selectores en bytecode; (iii) **transfer real ejecutado** | (i) los 4 revierten con `data: "0x"` (no existen); (ii) `paused()`/`isPaused()`/`frozen()`/`isFrozen` → **hits=0**; (iii) `transfer(0x0c82…,1e18) --from 0xb36e…` → `true` ⇒ **el token no está pausado ahora** |
| **8** | ERC-20 estándar (devuelve `bool` en `transfer`) | **PASS** | `C call ZIG "transfer(address,uint256)(bool)" …` y `"approve(address,uint256)(bool)" …` | `true` en ambas ⇒ ABI devuelve `bool`; **`requiresSafeERC20 = false`** |

**Extra (corrobora A4, no es check de esta unidad):** ni `owner()` (`0x8da5cb5b`), ni `implementation()` (`0x5c60da1b`), ni `admin()` (`0xf851a440`), ni `upgradeTo(address)` (`0x3659cfe6`) aparecen en el runtime (**hits=0**) y las llamadas revierten con `data:"0x"` ⇒ sin superficie de owner/proxy en el bytecode, consistente con `is_proxy:0` / `is_open_source:1` de GoPlus (A4).

---

## 2. Contexto medido (saldos y gates)

```
symbol()                      -> "ZIG"
name()                        -> "ZigCoin"
decimals()                    -> 18
totalSupply()                 -> 2000000000000000000000000000   (2e27)
balanceOf(pool UniV2)         -> 482060158529480338201961        (482,060.158 ZIG)
balanceOf(0x0c82… executor)   -> 0
balanceOf(0x1234… signer)     -> 0
balanceOf(0xdEaD control)     -> 46059208256256444544046448      (46,059,208.26 ZIG)
transfer(0x0c82…, 1e18) --from pool   -> true
transfer(0xdEaD, 1e18) --from pool    -> true
transfer(…, 1e18) --from 0x0c82…      -> revert "ERC20: transfer amount exceeds balance"   (gate de SALDO, no de blacklist)
transfer(…, 1e18) --from 0xdEaD       -> true                                             (el control fondeado sí puede originar)
allowance(pool, 0xdEaD)       -> 0
```

Los dos reverts observados en la batería son **los dos gates estándar de OpenZeppelin** (`…exceeds balance`, `…exceeds allowance`), con el **mismo texto** para direcciones nuestras y random: no hay un gate que discrimine nuestra address.

---

## 3. Veredicto GLOBAL: **PASS** — ZIG puede allowlistearse (con las salvedades ya declaradas por A4)

- **Checks 2, 4, 5, 8 de esta unidad: PASS.** No hay blacklist ni pause/freeze que discrimine a nuestras addresses operativas (`ARBITRAGE_EXECUTOR`, `FLASHLOAN_EXECUTOR_1`, `SIM_SIGNER_ADDRESS`), y el token es un ERC-20 estándar que devuelve `bool` (**no** requiere `SafeERC20`).
- **Completitud de las 10:** checks 1,3,6,7,9,10 vienen de A4 (GoPlus + honeypot.is: `is_honeypot:0`, `buy/sell/transfer_tax:0`, `is_open_source:1`, `is_proxy:0`, `is_blacklisted:0`); 2,4,5,8 quedan cerrados acá ⇒ **10/10 sin fallos**.
- **Salvedades que NO son fallos del screen (heredadas y re-declaradas):**
  1. **Riesgo operativo de liquidez** (A4): GoPlus reporta **99.2 % del LP en una sola EOA sin lock** (`is_locked:0`) ⇒ no es honeypot, es riesgo de retiro de liquidez ⇒ **tope de tamaño** al operar.
  2. **Piso de capital** (A4/A2): la fila top de ZIG ($1,272.10 de bruto sobre 28,289 ZIG ≈ $1,293 de tamaño) no cabe en `capital_usd=$1000`; el allowlist no cambia ese desajuste.
  3. **Re-screen obligatorio** si cambia el bytecode (acá no hay proxy: `implementation()`/`admin()` ausentes ⇒ el trigger clásico de upgrade no aplica; sí aplica re-screen por fecha/periodo).

---

## 4. Límites de la evidencia (fail-honest)

1. **`transfer` originado por nuestra address con saldo real: NO COMPUTADO** — nuestras addresses tienen **0 ZIG** en el fork (`balanceOf=0`), así que no se puede ejecutar un transfer con monto real desde ellas. Se sustituye por la prueba de **monto 0** (2a/2b/2c), que atraviesa exactamente los gates que importan (blacklist/pause del sender) y esquiva el de saldo, y por el **control diferencial** (2/4: mismo revert de saldo para random y para nosotros). No se inventa un saldo ni se usa `anvil_setBalance/setStorageAt` (eso mutaría el fork y violaría "solo lectura").
2. **`transferFrom` con allowance real: NO COMPUTADO** — no existe en el fork ninguna address con allowance sobre el pool (se midió `allowance(pool, 0xdEaD)=0`). Se prueba con monto 0 (2d/2e) y se documenta el revert estándar de allowance (2f).
3. **Pause por variable privada sin getter**: los selectores `paused/isPaused/frozen/isFrozen` no existen en el bytecode, y **un transfer real se ejecuta**, lo que prueba que nada está pausado *en este bloque*. Un flag privado sin getter no es observable por ABI; la prueba comportamental lo cubre para el estado actual, no para el futuro (por eso el re-screen periódico sigue siendo obligatorio).
4. **El fork es un bloque fijo (26253631)**: los resultados valen para ese estado, no para el tip. `block.timestamp` del fork = 1792738091.
5. **`cast` estuvo disponible** (host: `C:\Users\HFRC\.foundry\bin\cast.exe`; contenedor: `cast 1.7.1`, commit `4072e48`) — no hubo bloqueo de toolchain.

---

## 5. Reproducibilidad (comandos literales)

```bash
# transporte: WSL -> ssh -> docker exec (el ssh nativo de Windows falla en este host)
ssh -i ~/.ssh/arbx_hetzner root@195.201.235.70 \
  'docker exec arbitragex-v2-anvil-1 cast call 0xb2617246d0c6c0087f18703d576831899ca94f01 \
     "transfer(address,uint256)(bool)" 0x0C82CB749B53Cb3433319cd6Be18d746b3781B9B 1000000000000000000 \
     --from 0xb36ec83d844c0579ec2493f10b2087e96bb65460 --rpc-url http://127.0.0.1:8545'
# -> true

# selectores (presencia/ausencia en bytecode)
CODE=$(ssh … 'docker exec arbitragex-v2-anvil-1 cast code 0xb261… --rpc-url http://127.0.0.1:8545')
for SIG in "paused()" "isBlacklisted(address)" "owner()" "implementation()" "admin()" "upgradeTo(address)"; do
  SEL=$(ssh … "docker exec arbitragex-v2-anvil-1 cast sig '$SIG'")           # p.ej. 0x5c975abb
  echo "$SIG -> hits=$(printf %s "$CODE" | grep -o "${SEL#0x}" | wc -l)"    # 0 en todos
done
```

**Queries de procedencia (Q1–Q3)** — PG del VPS, solo lectura:

```sql
-- Q1 (address del token)
SELECT address, symbol, decimals, chain_id, is_active FROM tokens
WHERE lower(symbol) IN ('zig') OR address ILIKE '0xb2617246%';
-- -> 0xb2617246d0c6c0087f18703d576831899ca94f01 | ZIG | 18 | 1 | t

-- Q2 (pool fondeado = holder para originar el transfer)
SELECT p.address, d.name, p.fee_tier, t0.symbol, t1.symbol, p.is_active
FROM pools p JOIN factories f ON f.id=p.factory_id JOIN dexes d ON d.id=f.dex_id
JOIN tokens t0 ON t0.id=p.token0_id JOIN tokens t1 ON t1.id=p.token1_id
WHERE t0.address ILIKE '0xb2617246%' OR t1.address ILIKE '0xb2617246%';
-- -> 0xb36ec83d844c0579ec2493f10b2087e96bb65460 | UniswapV2 | 30 | ZIG | WETH | t
--    (además UniV3 0x051f301b… fee 10000 y UniV3 0x4b2cde9e… fee 100)

-- Q3 (nuestra address operativa: env paper, por servicio)
-- docker exec <svc> printenv ARBITRAGE_EXECUTOR FLASHLOAN_EXECUTOR_1 SIM_SIGNER_ADDRESS
-- -> 0x0c82CB749B53cB3433319cd6Be18d746b3781B9B
--    0xb47B3294d6B978964700Cb9350f8f8b4ACc39ACE
--    0x1234567890123456789012345678901234567890     (idéntico en los 4 servicios paper)
```

`wallets` y `contract_registry` de PG **no aportaron** address (0 filas): la fuente válida fue el **env de los servicios paper**, que es exactamente "la config que usa el executor en paper".
