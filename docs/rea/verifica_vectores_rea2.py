#!/usr/bin/env python3
"""REA-2 OPTIMAL-SIZE-01 — verificacion de vectores (FUSILE binding rule 1/3).

Determinista, sin red, sin dependencias (stdlib pura). Reproduce:
  V-A: getAmountIn/getAmountOut enteros del upstream
       (jacksonConrad/better-simple-arbitrage@f8b96f3, src/UniswappyV2EthPair.ts:293-304,
        tests en test/UniswappyV2EthPair.test.ts).
  V-B: escalera entera del ancestro (flashbots/simple-arbitrage@03cf1058,
       src/Arbitrage.ts getBestCrossedMarket) que produce los valores del test heredado
       del fork (test/Arbitrage.test.ts: volume 208333333333333333, profit 84409320961485006).
  V-C: forma cerrada del fork (src/Arbitrage.ts:79-94) recalculada en Decimal 60d +
       ejecucion entera del tamano optimo (dominancia sobre la escalera).

Salida: una linea "OK <item>" por verificacion exitosa; exit != 0 si alguna falla.
"""

from decimal import Decimal, getcontext, ROUND_FLOOR

getcontext().prec = 60
E = 10**18


def get_amount_in(r_in: int, r_out: int, amt_out: int) -> int:
    # UniswappyV2EthPair.ts:293-297 — fee sobre output-side, floor + 1 (ceil)
    num = r_in * amt_out * 1000
    den = (r_out - amt_out) * 997
    return num // den + 1


def get_amount_out(r_in: int, r_out: int, amt_in: int) -> int:
    # UniswappyV2EthPair.ts:299-304 — fee sobre input ANTES de dividir, floor
    aif = amt_in * 997
    return (aif * r_out) // (r_in * 1000 + aif)


def main() -> int:
    failures = []

    def check(name: str, cond: bool) -> None:
        print(("OK " if cond else "FAIL ") + name)
        if not cond:
            failures.append(name)

    # ---- V-A: pool [TOKEN=1e18, WETH=2e18], amountOut=1e17 (test upstream) ----
    a1 = get_amount_in(E, 2 * E, E // 10)
    a2 = get_amount_in(2 * E, E, E // 10)
    a3 = get_amount_out(E, 2 * E, a1)
    a4 = get_amount_out(2 * E, E, a2)
    check("VA1 getAmountIn(TOK,WETH,1e17)==52789948793749671", a1 == 52789948793749671)
    check("VA2 getAmountIn(WETH,TOK,1e17)==222890894906943052", a2 == 222890894906943052)
    check("VA3 roundtrip==1e17+1", a3 == E // 10 + 1)
    check("VA4 roundtrip==1e17", a4 == E // 10)

    # ---- V-B: escalera del ancestro, buy=m1[T=1,W=1], sell=m0[T=1,W=2] ----
    buy, sell = dict(t=1 * E, w=1 * E), dict(t=1 * E, w=2 * E)

    def profit_at(size: int) -> int:
        t_out = get_amount_out(buy["w"], buy["t"], size)
        proceeds = get_amount_out(sell["t"], sell["w"], t_out)
        return proceeds - size

    ladder = [E // 100, E // 10, E // 6, E // 4, E // 2, E, 2 * E, 5 * E, 10 * E]
    best = None
    for size in ladder:
        p = profit_at(size)
        if best is not None and p < best[1]:
            try_size = (size + best[0]) // 2  # meet halfway (ancestro)
            try_p = profit_at(try_size)
            if try_p > best[1]:
                best = (try_size, try_p)
            break
        best = (size, p)
    check("VB1 ladder volume==208333333333333333", best[0] == 208333333333333333)
    check("VB2 ladder profit==84409320961485006", best[1] == 84409320961485006)
    check("VB3 5/24e18 floor==208333333333333333", (5 * E) // 24 == 208333333333333333)

    # ---- V-C: forma cerrada (fork) recalculada, Decimal 60d ----
    D = Decimal
    g = D(997) / D(1000)
    t1, w1, t2, w2 = D(1), D(1), D(1), D(2)
    kB, kS = t1 * w1, t2 * w2
    d_alpha = ((kS.sqrt() * t1) - g**-1 * (kB.sqrt() * t2)) / (kB.sqrt() + kS.sqrt())
    d_beta = g**-1 * (kB / (t1 - d_alpha) - w1)
    d_beta_p = w2 - kS / (t2 + g * d_alpha)
    # cuadratica en dx: x* = (g*sqrt(t1*t2*w1*w2) - t2*w1)/(g*(t2+g*t1)) — misma raiz
    x_star = (g * (t1 * t2 * w1 * w2).sqrt() - t2 * w1) / (g * (t2 + g * t1))
    check("VC1 cerrada(deltaAlpha)==cuadratica(dx)", abs(d_beta - x_star) < D("1e-40"))
    dx_wei = int((x_star * E).to_integral_value(rounding=ROUND_FLOOR))
    check("VC2 dx*_wei==205911134347446829", dx_wei == 205911134347446829)
    check("VC3 profit_entero(dx*)>=ladder", profit_at(dx_wei) >= best[1])
    print("INFO profit_entero(dx*) =", profit_at(dx_wei), "(ladder:", best[1], ")")

    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
