#!/usr/bin/env python3
"""
Controles del selector de cadenas. Sin red: métricas inyectadas.

Control POSITIVO: una cadena con más volumen y menos gas sube en el prior.
Control NEGATIVO: una cadena sin gas medido queda EXCLUIDA con causa (no se inventa un cero).
Control del bandit: las recompensas realizadas mueven el posterior hacia lo observado.
Ejecutar: python3 scripts/arbi/test_chain_selector.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import chain_selector as cs  # noqa: E402

FAILED = []


def check(name, cond, detail=""):
    if cond:
        print(f"  PASS  {name}")
    else:
        print(f"  FAIL  {name}  {detail}")
        FAILED.append(name)


def mk(name, vol, gas_wei, native=2000.0, gas_units=250_000, comp=0.0):
    c = cs.ChainState(name=name, llama_name=name, chain_id=1, rpc="", native="x",
                      gas_units=gas_units, competition=comp)
    c.volume_24h_usd = vol
    c.gas_price_wei = gas_wei
    c.native_usd = native
    c.gas_usd = cs.compute_gas_usd(gas_wei, gas_units, native)
    return c


def test_robust_z():
    print("robust_z")
    check("MAD=0 devuelve ceros, no NaN", cs.robust_z([5.0, 5.0, 5.0]) == [0.0, 0.0, 0.0])
    z = cs.robust_z([1.0, 2.0, 3.0])
    check("monótono", z[0] < z[1] < z[2])


def test_gas_usd():
    print("compute_gas_usd")
    # 20 gwei * 250000 * 2000 USD / 1e18 = 0.01 USD
    g = cs.compute_gas_usd(20_000_000_000, 250_000, 2000.0)
    # 20 gwei * 250k gas = 0.005 ETH; * 2000 USD = 10 USD
    check("20 gwei/250k/2000 = 10 USD", abs(g - 10.0) < 1e-9, f"g={g}")
    check("sin precio -> None", cs.compute_gas_usd(20_000_000_000, 250_000, None) is None)


def test_priors():
    print("compute_priors")
    # misma gas, más volumen -> mejor prior
    a = mk("big", 1_000_000_000, 1_000_000)
    b = mk("small", 1_000_000, 1_000_000)
    # mismo volumen, más gas -> peor prior
    c = mk("cheap", 100_000_000, 1_000_000)
    d = mk("pricey", 100_000_000, 1_000_000_000)
    cs.compute_priors([a, b, c, d])
    check("más volumen => mayor prior", a.prior > b.prior, f"{a.prior} vs {b.prior}")
    check("más gas => menor prior", c.prior > d.prior, f"{c.prior} vs {d.prior}")


def test_exclusion():
    print("exclusión honesta")
    no_vol = mk("novol", None, 1_000_000)
    no_gas = mk("nogas", 100_000_000, None)
    no_gas.gas_usd = None
    cs.compute_priors([no_vol, no_gas])
    check("sin volumen => prior None", no_vol.prior is None)
    check("sin volumen => causa nombrada", bool(no_vol.excluded_reason))
    check("sin gas => prior None", no_gas.prior is None)
    check("sin gas => causa nombrada", bool(no_gas.excluded_reason))


def test_softmax():
    print("softmax")
    w = cs.softmax([0.0, 0.0, 0.0])
    check("uniforme suma 1", abs(sum(w) - 1.0) < 1e-9)
    w2 = cs.softmax([2.0, 0.0, -2.0])
    check("monótono", w2[0] > w2[1] > w2[2])


def test_bandit():
    print("bandit (Thompson)")
    a = mk("A", 100_000_000, 1_000_000)
    b = mk("B", 100_000_000, 1_000_000)
    cs.compute_priors([a, b])
    check("priors iguales de partida", abs(a.prior - b.prior) < 1e-12)
    # B rinde mucho mejor en la realidad
    cs.update_posterior([a, b], {"A": [-0.5, -0.4, -0.6], "B": [3.0, 3.5, 2.8]})
    check("B posterior > A posterior", b.posterior_mean > a.posterior_mean,
          f"A={a.posterior_mean} B={b.posterior_mean}")
    top = cs.thompson_topk([a, b], 1, rng_seed=1)
    check("Thompson elige B", top[0].name == "B", f"top={[c.name for c in top]}")


def main():
    for t in (test_robust_z, test_gas_usd, test_priors, test_exclusion, test_softmax, test_bandit):
        t()
    print()
    if FAILED:
        print(f"FALLOS: {len(FAILED)} -> {FAILED}")
        return 1
    print("TODO EN VERDE")
    return 0


if __name__ == "__main__":
    sys.exit(main())
