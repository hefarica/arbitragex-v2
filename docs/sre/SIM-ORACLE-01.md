# SIM-ORACLE-01 — taxonomía por capa, setup determinista y oráculo P-01

**Estado:** en curso (rama `rea-sim-fix-oracle-01`). **Modo:** paper. Sin firma, sin broadcast, sin capital.
**Origen:** orden técnica del operador (2026-10-18), a raíz de `simulations WHERE passed = 0`
sobre `simulations_total = 430.321`.

---

## 1. Por qué: `passed = 0` no es interpretable

El tally read-only mide `COUNT(*) FROM simulations WHERE passed`. Ese 0 mezcla dos cosas
que no se parecen en nada:

| Qué pasó | Qué significa | Cómo se ve hoy en el tally |
|---|---|---|
| El harness/entorno no pudo fondear, no resolvió el pool, no hubo gas | **No se midió nada.** No dice nada del mercado | idéntico a ↓ |
| La ejecución revirtió (invariante, gas) | El sim corrió y falló | idéntico a ↓ |
| La economía no da (`spread < coste`) | El mercado dijo que no hay edge | `passed = 0` |

Con esas tres leyendo igual, `fixture_failures > 0` se confunde con "no hay edge", y **G2 se
vuelve inútil**. Eso es lo que arregla esta pieza.

## 2. Taxonomía por capa — prefijos de `fail_reason`

Se prefija la razón **en el productor**, no en el lector:

```
fixture_*   harness / entorno   — fondeo, allowance, decimals, pool no resuelto, gas ausente,
                                  fork no disponible, backend no configurado
sim_*       ejecución           — revert del EVM, invariante, gas insuficiente en la tx,
                                  deadline, estado cambiado
market_*    economía            — non_positive_profit, spread_negative_round_trip,
                                  single_pool_no_spread, slippage > coste
```

Regla de mapeo (sobre lo que hoy existe): el clasificador `shared_rs::sim_taxonomy`
(`STRUCTURAL/ECONOMIC/MARKET`) ya tiene la semántica correcta — `sim_*` ↔ MARKET/EVM,
`market_*` ↔ ECONOMIC, `fixture_*` ↔ STRUCTURAL — pero **el string que se persiste no lleva
la capa**. `sim_signer_funding_slot_unresolved` cae hoy por *fail-closed* en STRUCTURAL sin
que nadie lo diga; con el prefijo sería `fixture_signer_funding_slot_unresolved` y **nadie
tendría que inferirlo**.

El tally pasa a reportar cuatro números en vez de uno:

```
passed · fixture_failures · sim_failures · market_failures
```

**Es un cambio de prefijos, no de arquitectura.** No toca la máquina de estados ni el scoring.

## 3. Setup determinista del fork — sin adivinar slots

Hoy `sim-ctl/src/signer_funding.rs` resuelve el balance del signer **sondeando slots**
(`CANDIDATE_SLOTS = [0, 2, 3, 9]`): escribe un centinela, comprueba `balanceOf`, y si el
token es un proxy o usa otro layout, **agota el loop** y devuelve
`sim_signer_funding_slot_unresolved` (≈ 89,5 % de las sims en el corte post-deploy).
El ETH nativo **no tiene slot**: si el harness resuelve uno, está resolviendo algo que no
existe.

Secuencia nueva, ejecutada **antes** de la sim y **asertada**:

1. **Balance nativo del signer** → `override directo de cuenta` (el saldo nativo vive en la
   cuenta, no en un slot de storage).
2. **ERC20 al signer** → en vez de resolver el slot del proxy, **impersonar una ballena y
   hacer un `transfer` real** al signer como pre-tx. Determinista, y no hay índice que adivinar.
3. **`approve` explícito** al router/executor por el importe de la operación.
4. **Aserciones de setup**, antes de tocar la sim:
   `signer.ETH ≥ 2 × gas` · `balanceOf(signer) ≥ size` · `allowance(signer, spender) ≥ size`.
   Si alguna falla → `fixture_*` **y la sim no se intenta** (el fallo no se imputa al mercado).

Decimales: **leer `token0()` de cada pool** y usar `decimals()` del token — el error clásico
es asumir que WETH es `token1` (en USDC/WETH **token0 es USDC**, 6 dec; token1 WETH, 18 dec).

**No se tocan los `rpc_error`** (7/27 en la corrida de hoy): son consecuencia. Primero el
setup, después se re-mide y se mira el residuo.

## 4. Oráculo `oracle_p01.py` — el número contra el que se compara el sim

`scripts/oracle/oracle_p01.py` (ya en esta rama). **No importa nada del repo**; solo la
librería estándar. Calcula, con aritmética V3 independiente, el beneficio esperado de un
round trip de dos piernas **en el mismo bloque**, y lo firma.

- Lee `factory.getPool(tA,tB,fee)`, `token0()`, `slot0()` y `liquidity()` de las dos piscinas
  en el bloque N; guarda el estado crudo y su `sha256`.
- Pierna 1 exact-in (precio baja): `sqrtNext = L·sqrtP/(L + amountIn·(1−f)·sqrtP/Q96)`,
  `amount1 = L·(sqrtP − sqrtNext)/Q96`.
- Pierna 2 (precio sube): `sqrtNext = sqrtP + amountIn·(1−f)·Q96/L`,
  `amount0 = L·Q96·(sqrtNext − sqrtP)/(sqrtP·sqrtNext)`.
- `gas_wei = gas_units · baseFeePerGas · 2`; `expected_profit = out2 − in1 − gas − fee_executor − flash_fee`.
- **Términos de fee declarados** (para que el oráculo los espeje): `fee_pool_a`, `fee_pool_b`,
  `fee_executor`, `flash_fee`. Por defecto `flash_fee = 0` porque la ruta decidida es
  **capital propio, cero flash-loan**.
- **Tolerancia:** `|sim − expected| / expected ≤ 10 %` **o** ±2 bps del notional, el mayor.
  Si el sim pasa y se sale de tolerancia, ese `passed` miente.
- **N-01:** tamaño = **60 % del break-even** derivado del propio oráculo (el tamaño donde el
  bruto iguala al gas), nunca a ojo.
- **N-02:** piernas invertidas → esperado `market_non_positive_profit`.
- **N-03:** P-01 dos veces, con `PAPER_STACK_DEPLOY=0` y `=1` → mismo beneficio dentro de
  tolerancia **en ambos**. Si sólo pasa con `=1`, el paper stack fabrica la segunda pierna.

## 5. Hallazgo que bloquea P-01 tal como estaba planteado

Con **USDC/WETH 500 ↔ 3000** el round trip tiene que superar **35 bps** de fee (5+30), y las
dos piscinas siguen el mismo activo. Barrido de ~40 muestras sobre los últimos ~60.000
bloques: **dislocación máxima 25,4 bps** (bloque `26197506` / `0x18fca02`,
hash `0xad2b52df996f22d80d10f36f879dcb6f78e1bf5d3c6142720af131c7b23f4d96`).

**Medido con el oráculo en ese bloque, size = 40 USDC:**

```
dislocation_bps                    = 25,36
amount_in                          = 40.000.000 (unidades base USDC = 40 USDC)
expected_out1                      = 16.292.520.003.569.016 wei WETH
expected_out2                      = 39.759.205,76 unidades base USDC
expected_gas                       = 80.023,89 unidades base USDC  (250.000 gas · baseFee·2)
expected_profit                    = −320.818,12 unidades base USDC  (−0,32 USDC)
assert(expected_profit >= 3*gas)   = False
```

⇒ **No existe P-01 positivo con ese par/tier.** Opciones (a decidir por el operador):
(i) mismo par con **gap de fee mayor** (500↔10000 exige >105 bps: peor);
(ii) **par/tier de liquidez fina**, donde la dislocación real supera el fee;
(iii) un bloque del **conjunto que exporta el pipeline** (los 27 del benchmark) — si el caso
tiene que validar al sim **que corre en producción**, el bloque debe venir de ahí.

Insumos verificados (on-chain, 2026-10-18): factory UniV3 `0x1F98431c8aD98523631AE4a59f267346ea31F984`;
USDC `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`; WETH `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2`;
fee 500 → `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640`; fee 3000 → `0x8ad599c3A0ff1De082011EFDDc58f1908eB6e6D8`;
RPC de fork `https://eth.drpc.org` **sirve archivo** (bloque 20.578.304 OK).

## 6. Uso

```bash
# caso puntual
python3 scripts/oracle/oracle_p01.py --block 0x18fca02 --size 40000000
# barrido de dislocación
python3 scripts/oracle/oracle_p01.py --scan 60000 --scan-step 1500
```

## 7. Pendiente en esta rama

- [x] `scripts/oracle/oracle_p01.py` (verificado contra mainnet)
- [x] prefijos por capa en el productor: `shared-rs::sim_taxonomy::{layer_of, with_layer_prefix, layer_of_prefixed}`
      + prefijado del `fail_reason` persistido en `sim-ctl::insert_simulation` (tests en verde)
- [ ] tally desglosado (`passed`/`fixture_failures`/`sim_failures`/`market_failures`) en el canal read-only
- [ ] setup determinista del fork (nativo por override, ballena + `transfer`, `approve`, aserciones)
- [ ] casos N-01 / N-02 / N-03 cableados al gate
- [ ] bloque de P-01 (a decidir: (i) / (ii) / (iii))
