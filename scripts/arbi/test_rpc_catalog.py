#!/usr/bin/env python3
"""
Controles del catálogo de RPC. El control que importa es el NEGATIVO de secreto:
ninguna clave embebida puede sobrevivir al saneado. Sin red.

Ejecutar: python3 scripts/arbi/test_rpc_catalog.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rpc_catalog as rc  # noqa: E402

FAILED = []


def check(name, cond, detail=""):
    if cond:
        print(f"  PASS  {name}")
    else:
        print(f"  FAIL  {name}  {detail}")
        FAILED.append(name)


def test_sanitize():
    print("sanitize")
    a = rc.sanitize("https://eth-mainnet.g.alchemy.com/v2/alch_7Yw8qxnY1fY_iKzJt-jYp", "Alchemy")
    check("Alchemy: la clave NO sobrevive", "alch_7Yw8qxnY1fY_iKzJt-jYp" not in a, a)
    check("Alchemy: queda como ${VAR}", "${ALCHEMY_API_KEY}" in a, a)
    pub = "https://ethereum-rpc.publicnode.com/"
    check("URL pública intacta", rc.sanitize(pub, "PublicNode") == pub)
    check("detecta secreto embebido", rc.has_embedded_secret(
        "https://eth-mainnet.g.alchemy.com/v2/abc123456789012345678") is True)
    check("no marca una pública", rc.has_embedded_secret(pub) is False)


def test_build_catalog():
    print("build_catalog")
    entries = [
        rc.RpcEntry("Ethereum Mainnet", "HTTP", "Alchemy", "https://eth-mainnet.g.alchemy.com/v2/SUPERSECRETKEY123456"),
        rc.RpcEntry("Ethereum Mainnet", "HTTP", "PublicNode", "https://ethereum-rpc.publicnode.com/"),
        rc.RpcEntry("Ethereum Sepolia", "HTTP", "PublicNode", "https://ethereum-sepolia-rpc.publicnode.com/"),
    ]
    cat = rc.build_catalog(entries)
    blob = str(cat)
    check("testnet fuera del catálogo de producción", "Ethereum Sepolia" not in cat["chains"])
    check("secreto contado", cat["secrets_sanitized"] == 1)
    check("secreto NO está en el JSON", "SUPERSECRETKEY123456" not in blob)
    check("entrada pública presente", any(e["provider"] == "PublicNode" for e in cat["chains"]["Ethereum Mainnet"]))


def test_emit_env():
    print("emit_env (G-RPC-1)")
    entries = [
        rc.RpcEntry("Ethereum Mainnet", "HTTP", "Alchemy", "https://eth-mainnet.g.alchemy.com/v2/SUPERSECRETKEY123456"),
        rc.RpcEntry("Ethereum Mainnet", "HTTP", "PublicNode", "https://ethereum-rpc.publicnode.com/"),
        rc.RpcEntry("Ethereum Mainnet", "HTTP", "DRPC", "https://eth.drpc.org/"),
        rc.RpcEntry("Base", "HTTP", "DRPC", "https://base.drpc.org/"),
    ]
    cat = rc.build_catalog(entries)
    text, warns = rc.emit_env(cat)
    check("clave G-RPC-1 de Ethereum (chain 1)", "RPC_HTTP_1=" in text)
    check("clave G-RPC-1 de Base (8453)", "RPC_HTTP_8453=" in text)
    check("formato name=url", "publicnode=https://ethereum-rpc.publicnode.com/" in text)
    check("el secreto NO está en el .env emitido", "SUPERSECRETKEY123456" not in text)
    check("Alchemy queda con ${VAR}", "${ALCHEMY_API_KEY}" in text)
    # Base tiene UN proveedor -> aviso de failover imposible
    check("avisa cuando hay <2 proveedores", any("Base HTTP" in w for w in warns), str(warns))
    check("Ethereum (3 proveedores) sin aviso", not any("Ethereum Mainnet HTTP" in w for w in warns))


def test_percentile():
    print("percentile")
    v = [10.0, 20.0, 30.0, 40.0, 100.0]
    check("p50", rc.percentile(v, 0.50) == 30.0, rc.percentile(v, 0.50))
    check("p99 tope = max", rc.percentile(v, 0.99) == 100.0)
    check("vacío -> nan", rc.percentile([], 0.5) != rc.percentile([], 0.5))


def test_controls():
    print("run_controls (sonda que DISTINGUE)")
    orig = rc._rpc_timed

    def fake(url, method="eth_chainId", timeout=8.0):
        if "good" in url:
            return True, 12.0, ""
        return False, 5.0, "HTTPError: 525"
    rc._rpc_timed = fake
    try:
        ok = rc.run_controls("http://good", "http://dead", samples=3, timeout=1)
        check("distingue bueno de muerto", ok["distinguishes"] is True)
        check("veredicto positivo", "distingue" in ok["verdict"])

        def broken(url, method="eth_chainId", timeout=8.0):
            return True, 10.0, ""  # miente: todo "bueno"
        rc._rpc_timed = broken
        bad = rc.run_controls("http://good", "http://dead", samples=3, timeout=1)
        check("sonda rota se detecta (no distingue)", bad["distinguishes"] is False)
        check("veredicto nombra el fallo", "CONTROL FALLIDO" in bad["verdict"])
    finally:
        rc._rpc_timed = orig


def test_compare_runs():
    print("compare_runs (criterio de muerte)")
    a = {"chains": {"X": [{"provider": "p", "p50_ms": 10}, {"provider": "q", "p50_ms": 20}]}}
    b = {"chains": {"X": [{"provider": "p", "p50_ms": 11}, {"provider": "q", "p50_ms": 19}]}}
    r = rc.compare_runs(a, b)
    check("mismo orden => estable", r["unstable"] == [])
    c = {"chains": {"X": [{"provider": "q", "p50_ms": 9}, {"provider": "p", "p50_ms": 22}]}}
    r2 = rc.compare_runs(a, c)
    check("orden invertido => inestable", r2["unstable"] == ["X"])
    check("veredicto nombra ruido", "INESTABLE" in r2["verdict"])


def main():
    for t in (test_sanitize, test_build_catalog, test_emit_env,
              test_percentile, test_controls, test_compare_runs):
        t()
    print()
    if FAILED:
        print(f"FALLOS: {len(FAILED)} -> {FAILED}")
        return 1
    print("TODO EN VERDE")
    return 0


if __name__ == "__main__":
    sys.exit(main())
