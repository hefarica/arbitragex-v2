#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Captura RESERVAS REALES de pools Uniswap-V2 mainnet via RPC publico keyless.

PRECIO-CANONICO-CEX WO-PC8: el harness de comparacion de estrategias corre
offline, asi que las reservas de la ruta de 3 legs se capturan UNA VEZ aqui
(lectura real, documentada) y se almacenan como snapshot JSON en el repo.

Fuente RPC: https://ethereum-rpc.publicnode.com (publico, sin key, $0).
Metodo: eth_call getReserves()/token0()/token1() sobre pares descubiertos via
UniswapV2Factory.getPair() (0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f).
Se registra bloque + baseFeePerGas real del bloque de captura (para el gas
estimado del harness). RULE 00: nada se fabrica; fallo = error explícito.
"""
import json
import urllib.request
import time

RPC = "https://ethereum-rpc.publicnode.com"
FACTORY = "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"
WETH = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
USDC = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
USDT = "0xdac17f958d2ee523a2206206994597c13d831ec7"
DAI = "0x6b175474e89094c44da98b954eedeac495271d0f"

SEL_GETPAIR = "0xe6a94a77"   # getPair(address,address)
SEL_RESERVES = "0x0902f1ac"  # getReserves()
SEL_TOKEN0 = "0x0dfe1681"    # token0()
SEL_TOKEN1 = "0xd21220a7"    # token1()

CANDIDATE_CYCLES = [
    ("WETH", "USDC", "DAI"),
    ("WETH", "USDC", "USDT"),
    ("WETH", "USDT", "DAI"),
]

ADDR = {"WETH": WETH, "USDC": USDC, "USDT": USDT, "DAI": DAI}
DECIMALS = {"WETH": 18, "USDC": 6, "USDT": 6, "DAI": 18}


def rpc(method, params, tries=3):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method,
                       "params": params}).encode()
    last = None
    for _ in range(tries):
        req = urllib.request.Request(RPC, data=body, headers={
            "Content-Type": "application/json",
            "User-Agent": "arbx-reserves-audit/1.0",
        })
        try:
            raw = json.load(urllib.request.urlopen(req, timeout=20))
            if "result" in raw:
                return raw["result"]
            last = raw
        except Exception as e:  # noqa: BLE001
            last = str(e)
        time.sleep(1.5)
    raise SystemExit("RPC %s fallo tras reintentos: %s" % (method, last))


def pad_addr(a):
    return "0x" + "0" * 24 + a[2:].lower()


def call(to, data):
    return rpc("eth_call", [{"to": to, "data": data}, "latest"])


def get_pair(a, b):
    data = SEL_GETPAIR + pad_addr(a)[2:] + pad_addr(b)[2:]
    res = call(FACTORY, data)
    if int(res, 16) == 0:
        return None
    return "0x" + res[26:]


def word(res, i):
    h = res[2:]
    return int(h[i * 64:(i + 1) * 64], 16)


def main():
    block_hex = rpc("eth_blockNumber", [])
    block = int(block_hex, 16)
    blk = rpc("eth_getBlockByNumber", [block_hex, False])
    base_fee_wei_per_gas = int(blk["baseFeePerGas"], 16)
    out = {
        "source": RPC,
        "captured_at_unix": int(time.time()),
        "block_number": block,
        "base_fee_wei_per_gas": base_fee_wei_per_gas,
        "base_fee_gwei": base_fee_wei_per_gas / 1e9,
        "factory": FACTORY,
        "cycles_tried": [],
        "selected_cycle": None,
        "pools": {},
    }
    for cyc in CANDIDATE_CYCLES:
        a, b, c = cyc
        out["cycles_tried"].append(list(cyc))
        edges = [(a, b), (b, c), (a, c)]
        pairs = {}
        ok = True
        for (t0, t1) in edges:
            p = get_pair(ADDR[t0], ADDR[t1])
            if p is None:
                ok = False
                break
            pairs[(t0, t1)] = p
        if not ok:
            continue
        for (t0, t1), p in pairs.items():
            res = call(p, SEL_RESERVES)
            tok0 = "0x" + call(p, SEL_TOKEN0)[26:]
            tok1 = "0x" + call(p, SEL_TOKEN1)[26:]
            r0, r1, ts = word(res, 0), word(res, 1), word(res, 2)
            # orienta por token0 real del contrato
            sym0 = t0 if tok0.lower() == ADDR[t0].lower() else t1
            sym1 = t1 if sym0 == t0 else t0
            key = "%s/%s" % (t0, t1)
            out["pools"][key] = {
                "pair_address": p,
                "token0_addr": tok0,
                "token1_addr": tok1,
                "token0_symbol": sym0,
                "token1_symbol": sym1,
                "reserve_token0": str(r0),
                "reserve_token1": str(r1),
                "reserves_ts_unix": ts,
                "decimals0": DECIMALS[sym0],
                "decimals1": DECIMALS[sym1],
            }
        out["selected_cycle"] = list(cyc)
        break
    if out["selected_cycle"] is None:
        raise SystemExit("ningun ciclo candidato tuvo 3 pares V2 con liquidez")
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
