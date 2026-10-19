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


def main():
    for t in (test_sanitize, test_build_catalog):
        t()
    print()
    if FAILED:
        print(f"FALLOS: {len(FAILED)} -> {FAILED}")
        return 1
    print("TODO EN VERDE")
    return 0


if __name__ == "__main__":
    sys.exit(main())
