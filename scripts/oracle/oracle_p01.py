#!/usr/bin/env python3
"""
oracle_p01.py — oráculo independiente para el caso P-01 del simulador (V3, 2 piernas).

NO IMPORTA NADA DEL REPO. Habla JSON-RPC directo contra un nodo. Su única
dependencia es la librería estándar de Python 3.

POR QUÉ EXISTE
--------------
`simulations WHERE passed = 0` es ininterpretable por sí solo: un fixture roto y
un mercado sin edge se ven idénticos desde el tally. Este oráculo calcula, con
aritmética V3 independiente del simulador, el beneficio ESPERADO de un round
trip de dos piernas en el mismo bloque, y devuelve el número contra el que el
`sim` se compara. Si el sim pasa pero se sale de la tolerancia, el `passed`
miente.

DECLARACIÓN DE TÉRMINOS DE FEE (lo que el sim añade, para que el oráculo lo espeje)
----------------------------------------------------------------------------------
  fee_pool_a / fee_pool_b : pips por piscina (se aplican DENTRO de cada swap V3).
  fee_executor            : comisión que el contrato de ejecución cobra sobre el
                            beneficio (por defecto 0 — declararla explícitamente).
  flash_fee               : prima del flash loan. 0 por defecto: la ruta decidida
                            es CAPITAL PROPIO (cero flash-loan). Si algún día se
                            enciende el tramo flash, hay que subirla o el oráculo
                            miente por defecto.
  gas_units               : unidades de gas del arb. `eth_estimateGas` exige un
                            `from`/`to`/`data` de un contrato desplegado; el
                            executor NO está desplegado en mainnet, así que el
                            oráculo usa este valor DECLARADO (override con
                            --gas-units) y lo multiplica por `baseFeePerGas*2`
                            tomado del propio bloque — nunca un gas "a ojo" sin
                            decirlo.

USO
---
  # caso puntual
  ./oracle_p01.py --rpc https://eth.drpc.org --block 0x18fca02 --size 40

  # barrido: encuentra el bloque de máxima dislocación en un rango y lo evalúa
  ./oracle_p01.py --rpc https://eth.drpc.org --scan 60000 --scan-step 1500

  # salida JSON canónica (la que se firma)
  ./oracle_p01.py ... --json

SALIDA
------
Un JSON con: bloque, blockHash, direcciones (factory, pools, tokens), slot0 y
liquidity crudos de ambas piscinas, `state_sha256` (firma del estado crudo),
expected_amount_out1 / _out2, expected_gas_eth, expected_profit_token0 y
expected_profit_usd (si hay precio), tolerancia, y el veredicto
`expected_profit >= 3 * gas`.
"""

import argparse
import hashlib
import json
import sys
import time
import urllib.request

Q96 = 1 << 96

# Defaults mainnet — USDC/WETH en Uniswap V3 (verificados on-chain 2026-10-18).
FACTORY = "0x1F98431c8aD98523631AE4a59f267346ea31F984"
TOKEN_A = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"  # USDC (6 dec)
TOKEN_B = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"  # WETH (18 dec)
FEE_A = 500
FEE_B = 3000
RPC_DEFAULT = "https://eth.drpc.org"

SEL_GET_POOL = "0x1698ee82"  # getPool(address,address,uint24)
SEL_TOKEN0 = "0x0dfe1681"    # token0()
SEL_SLOT0 = "0x3850c7bd"     # slot0()
SEL_LIQUIDITY = "0x1a686502"  # liquidity()
SEL_DECIMALS = "0x313ce567"  # decimals()


class RpcError(RuntimeError):
    pass


class Rpc:
    def __init__(self, url, retries=3, pause=0.6):
        self.url = url
        self.retries = retries
        self.pause = pause

    def __call__(self, method, params):
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
        last = None
        for _ in range(self.retries):
            try:
                req = urllib.request.Request(
                    self.url,
                    data=body,
                    headers={"Content-Type": "application/json", "User-Agent": "curl/8.5.0"},
                )
                out = json.load(urllib.request.urlopen(req, timeout=30))
                if "error" in out:
                    raise RpcError(out["error"])
                return out["result"]
            except Exception as exc:  # noqa: BLE001 — reintento deliberado
                last = exc
                time.sleep(self.pause)
        raise RpcError(f"{method} failed after {self.retries} tries: {last}")


def w(x):
    """Left-pad a hex string to a 32-byte word."""
    return x[2:].lower().rjust(64, "0")


def word_of(raw, index=0):
    s = raw[2:]
    return int(s[index * 64 : (index + 1) * 64], 16)


def call(rpc, to, data, block):
    return rpc("eth_call", [{"to": to, "data": data}, block])


# ── aritmética V3 exact-in (liquidez constante; válida mientras no se cruce un tick) ──


def amount_out0_for_in1(sqrt_p, liq, amount_in, fee_pips):
    """token0 -> token1 exact-in. El precio BAJA."""
    f = 1 - fee_pips / 1e6
    num = sqrt_p * liq
    den = liq + amount_in * f * sqrt_p / Q96
    sqrt_next = num / den
    return liq * (sqrt_p - sqrt_next) / Q96


def amount_out1_for_in0(sqrt_p, liq, amount_in, fee_pips):
    """token1 -> token0 exact-in. El precio SUBE.

    amount0 = L * Q96 * (sqrtNext - sqrtP) / (sqrtP * sqrtNext)   [todo en X96]
    """
    f = 1 - fee_pips / 1e6
    sqrt_next = sqrt_p + amount_in * f * Q96 / liq
    return liq * Q96 * (sqrt_next - sqrt_p) / (sqrt_p * sqrt_next)


def read_pool(rpc, factory, ta, tb, fee, block):
    pool = rpc("eth_call", [{"to": factory, "data": SEL_GET_POOL + w(ta) + w(tb) + hex(fee)[2:].rjust(64, "0")}, block])
    pool = "0x" + pool[-40:]
    if int(pool, 16) == 0:
        raise RpcError(f"getPool returned zero for fee={fee}")
    t0 = "0x" + call(rpc, pool, SEL_TOKEN0, block)[-40:]
    slot0 = call(rpc, pool, SEL_SLOT0, block)
    liq = int(call(rpc, pool, SEL_LIQUIDITY, block), 16)
    dec = int(call(rpc, t0, SEL_DECIMALS, block), 16)
    return {
        "pool": pool,
        "fee": fee,
        "token0": t0,
        "decimals0": dec,
        "sqrt_price_x96": str(word_of(slot0, 0)),
        "tick": word_of(slot0, 1),
        "liquidity": str(liq),
        "raw_slot0": slot0,
    }


def round_trip(pa, pb, size_a, fee_executor=0.0, flash_fee=0.0):
    """Round trip: token0 -> token1 en la piscina BARATA, token1 -> token0 en la CARA.

    `size_a` va en unidades base del token de entrada (token0 del par elegido).
    Devuelve el desglose por pierna.
    """
    sq_a = int(pa["sqrt_price_x96"])
    sq_b = int(pb["sqrt_price_x96"])
    la = int(pa["liquidity"])
    lb = int(pb["liquidity"])

    # precio crudo token1/token0 de cada piscina
    price_a = (sq_a / Q96) ** 2
    price_b = (sq_b / Q96) ** 2

    # comprar en la BARATA (precio bajo => token1 barato allí), vender en la CARA
    if price_a < price_b:
        buy, sell = pa, pb
    else:
        buy, sell = pb, pa

    out1 = amount_out0_for_in1(int(buy["sqrt_price_x96"]), int(buy["liquidity"]), size_a, buy["fee"])
    out2_gross = amount_out1_for_in0(int(sell["sqrt_price_x96"]), int(sell["liquidity"]), out1, sell["fee"])
    flash = out2_gross * flash_fee
    gross = out2_gross - size_a - flash
    fee = gross * fee_executor if gross > 0 else 0.0
    return {
        "buy_pool": buy["pool"],
        "sell_pool": sell["pool"],
        "buy_fee": buy["fee"],
        "sell_fee": sell["fee"],
        "price_token1_per_token0_buy": price_a if price_a < price_b else price_b,
        "price_token1_per_token0_sell": price_a if price_a >= price_b else price_b,
        "dislocation_bps": abs(price_a - price_b) / min(price_a, price_b) * 1e4,
        "amount_in": size_a,
        "amount_out1": out1,
        "amount_out2_gross": out2_gross,
        "flash_fee_units": flash,
        "fee_executor_units": fee,
        "gross_profit_units": gross - fee,
    }


def gas_in_token0(rpc, block, gas_units, price_token1_per_token0):
    """gas_wei = gas_units * baseFeePerGas * 2 ; se expresa en unidades del token0
    vía el precio de la piscina (token1 por token0)."""
    blk = rpc("eth_getBlockByNumber", [block, False])
    base_fee = int(blk.get("baseFeePerGas") or rpc("eth_gasPrice", []), 16)
    gas_wei = gas_units * base_fee * 2
    # gas_wei está en wei de ETH = token1 crudo. A unidades de token1:
    # se convierte a unidades de token0 dividiendo por el precio (token1 por token0).
    if price_token1_per_token0 <= 0:
        return gas_wei, base_fee
    gas_token0 = gas_wei / price_token1_per_token0
    return gas_token0, base_fee


def evaluate(rpc, factory, ta, tb, fa, fb, block, size, gas_units, fee_executor, flash_fee):
    pa = read_pool(rpc, factory, ta, tb, fa, block)
    pb = read_pool(rpc, factory, ta, tb, fb, block)
    rt = round_trip(pa, pb, size, fee_executor, flash_fee)
    gas_t0, base_fee = gas_in_token0(rpc, block, gas_units, rt["price_token1_per_token0_sell"])
    expected = rt["gross_profit_units"] - gas_t0
    blk = rpc("eth_getBlockByNumber", [block, False])
    raw_state = json.dumps(
        {"pa": pa["raw_slot0"], "pb": pb["raw_slot0"], "la": pa["liquidity"], "lb": pb["liquidity"]},
        sort_keys=True,
    )
    return {
        "block": blk.get("number"),
        "block_hash": blk.get("hash"),
        "block_timestamp": blk.get("timestamp"),
        "factory": factory,
        "token_a": ta,
        "token_b": tb,
        "pool_a": pa,
        "pool_b": pb,
        "dislocation_bps": rt["dislocation_bps"],
        "buy_pool": rt["buy_pool"],
        "sell_pool": rt["sell_pool"],
        "amount_in_token0_units": size,
        "expected_amount_out1_token1_units": rt["amount_out1"],
        "expected_amount_out2_token0_units": rt["amount_out2_gross"],
        "base_fee_per_gas_wei": base_fee,
        "gas_units": gas_units,
        "expected_gas_token0_units": gas_t0,
        "expected_profit_token0_units": expected,
        "fee_terms": {
            "fee_pool_a_pips": fa,
            "fee_pool_b_pips": fb,
            "fee_executor": fee_executor,
            "flash_fee": flash_fee,
            "note": "el oráculo espeja estos términos; el sim debe declarar los suyos",
        },
        "tolerance": {
            "relative": 0.10,
            "absolute_bps_of_notional": 2,
            "rule": "aceptar si |sim-expected|/expected <= 10% o <= 2 bps del notional (el mayor)",
            "notional_token0_units": size,
            "absolute_units": size * 2 / 1e4,
        },
        "assert_expected_profit_ge_3x_gas": expected >= 3 * gas_t0,
        "state_sha256": hashlib.sha256(raw_state.encode()).hexdigest(),
    }


def break_even_size(rpc, pa, pb, lo, hi, iters=40):
    """Tamaño donde gross_profit == gas. Bisección sobre el beneficio bruto."""
    for _ in range(iters):
        mid = (lo + hi) / 2
        rt = round_trip(pa, pb, mid)
        if rt["gross_profit_units"] > 0:
            hi = mid
        else:
            lo = mid
    return hi


def scan(rpc, factory, ta, tb, fa, fb, head, span, step):
    best = None
    k = step
    while k <= span:
        blk = hex(head - k)
        try:
            pa = read_pool(rpc, factory, ta, tb, fa, blk)
            pb = read_pool(rpc, factory, ta, tb, fb, blk)
            price_a = (int(pa["sqrt_price_x96"]) / Q96) ** 2
            price_b = (int(pb["sqrt_price_x96"]) / Q96) ** 2
            dis = abs(price_a - price_b) / min(price_a, price_b) * 1e4
            if best is None or dis > best["dislocation_bps"]:
                best = {"block": blk, "dislocation_bps": dis}
        except RpcError:
            pass
        k += step
    return best


def main(argv=None):
    ap = argparse.ArgumentParser(description="Oráculo P-01 (V3, 2 piernas) — independiente del repo")
    ap.add_argument("--rpc", default=RPC_DEFAULT)
    ap.add_argument("--factory", default=FACTORY)
    ap.add_argument("--token-a", default=TOKEN_A)
    ap.add_argument("--token-b", default=TOKEN_B)
    ap.add_argument("--fee-a", type=int, default=FEE_A)
    ap.add_argument("--fee-b", type=int, default=FEE_B)
    ap.add_argument("--block", default="latest")
    ap.add_argument("--size", type=float, default=0.0, help="unidades base del token0; 0 => 60%% del break-even")
    ap.add_argument("--gas-units", type=int, default=250000)
    ap.add_argument("--fee-executor", type=float, default=0.0)
    ap.add_argument("--flash-fee", type=float, default=0.0)
    ap.add_argument("--scan", type=int, default=0, help="bloques hacia atrás a barrer")
    ap.add_argument("--scan-step", type=int, default=1500)
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args(argv)

    rpc = Rpc(a.rpc)
    if a.scan:
        head = int(rpc("eth_blockNumber", []), 16)
        best = scan(rpc, a.factory, a.token_a, a.token_b, a.fee_a, a.fee_b, head, a.scan, a.scan_step)
        print(json.dumps({"head": head, "scan_span": a.scan, "scan_step": a.scan_step, "best": best}, indent=2))
        return 0

    block = a.block
    if block == "latest":
        block = hex(int(rpc("eth_blockNumber", []), 16))
    pa = read_pool(rpc, a.factory, a.token_a, a.token_b, a.fee_a, block)
    pb = read_pool(rpc, a.factory, a.token_a, a.token_b, a.fee_b, block)

    size = a.size
    if size <= 0:
        size = break_even_size(rpc, pa, pb, 1.0, 10 ** 15) * 0.6
    out = evaluate(rpc, a.factory, a.token_a, a.token_b, a.fee_a, a.fee_b, block, size, a.gas_units, a.fee_executor, a.flash_fee)
    out["size_rule"] = "declarado" if a.size > 0 else "60% del break-even derivado del oráculo"
    print(json.dumps(out, indent=2) if not a.json else json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
