#!/usr/bin/env python3
"""
ARBI — catálogo canónico de RPC + sonda de latencia.

Dos cosas, y las dos con artefacto:

  1) `extract`  — lee el Excel unificado (`ArbitrageX_Unified_Config.xlsm`, hoja
     "RPC Providers") y emite un catálogo JSON **versionable** con los RPC por
     cadena. Los secretos NO se copian: cualquier URL que lleve una API key
     embebida se sanea a `${VAR}` (ver `sanitize`). Un catálogo con credenciales
     dentro no es canónico, es una fuga esperando a pasar.

  2) `probe`    — mide LATENCIA REAL de cada RPC (`eth_chainId`, N muestras) y
     emite, por cadena, el orden de conexión: el más rápido primero, con su
     p50/p95. Eso es "engancharse siempre al mejor con menor latencia": no por
     fama del proveedor, por número medido.

R8 / RULE 00: un RPC que no responde se marca `ok=false` con su causa; nunca se
le asigna una latencia inventada.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import statistics
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, asdict
from typing import Optional

UA = "arbi-rpc-catalog/1.0 (+https://github.com/hefarica/arbitragex-v2)"

# Cadenas con las que trabaja el pipeline (las mismas del selector).
WANTED_CHAINS = {
    "Ethereum Mainnet", "Base", "Arbitrum One", "Optimism", "Polygon Mainnet",
    "BSC Mainnet", "Avalanche", "Linea", "Scroll", "Blast", "Gnosis",
}

# Proveedor -> variable de entorno para la clave (el catálogo guarda `${VAR}`, nunca el valor).
PROVIDER_KEY_ENV = {
    "Alchemy": "ALCHEMY_API_KEY",
    "Infura": "INFURA_API_KEY",
    "QuickNode": "QUICKNODE_API_KEY",
}

_KEYLIKE = re.compile(r"(/v2/|/v3/|apikey=|/rpc/)[A-Za-z0-9_\-]{16,}", re.IGNORECASE)


def sanitize(url: str, provider: str) -> str:
    """Sustituye una API key embebida por `${VAR}`. No toca rutas públicas."""
    env = PROVIDER_KEY_ENV.get(provider)
    if "alchemy.com/v2/" in url:
        return re.sub(r"(/v2/)[^/?#]+", r"\1${ALCHEMY_API_KEY}", url)
    if env and _KEYLIKE.search(url):
        return _KEYLIKE.sub(lambda m: m.group(1) + "${" + env + "}", url)
    return url


def has_embedded_secret(url: str) -> bool:
    return "alchemy.com/v2/" in url or bool(_KEYLIKE.search(url))


# ─────────────────────────────── extracción ────────────────────────────────────

@dataclass
class RpcEntry:
    chain: str
    protocol: str
    provider: str
    url: str


def extract(xlsm_path: str, sheet: str = "RPC Providers") -> list[RpcEntry]:
    import openpyxl
    wb = openpyxl.load_workbook(xlsm_path, data_only=True, read_only=True)
    if sheet not in wb.sheetnames:
        raise SystemExit(f"hoja '{sheet}' no está en el Excel; hay: {wb.sheetnames}")
    ws = wb[sheet]
    out: list[RpcEntry] = []
    for row in ws.iter_rows(values_only=True):
        c = [("" if v is None else str(v).strip()) for v in row]
        if len(c) >= 4 and c[0] and c[1] in ("HTTP", "WSS") and c[2] and c[3]:
            out.append(RpcEntry(chain=c[0], protocol=c[1], provider=c[2], url=c[3]))
    return out


def build_catalog(entries: list[RpcEntry]) -> dict:
    chains: dict[str, list[dict]] = {}
    secrets_found = 0
    for e in entries:
        if e.chain not in WANTED_CHAINS:
            continue
        if has_embedded_secret(e.url):
            secrets_found += 1
        chains.setdefault(e.chain, []).append({
            "protocol": e.protocol,
            "provider": e.provider,
            "url": sanitize(e.url, e.provider),
            "url_has_env_var": has_embedded_secret(e.url),
        })
    return {
        "_comment": ("Catálogo canónico de RPC por cadena. Extraído del Excel unificado; "
                     "las URLs con clave van saneadas a `${VAR}` y la clave vive en el `.env` del host, "
                     "NUNCA en este fichero."),
        "secrets_sanitized": secrets_found,
        "chains": {k: chains[k] for k in sorted(chains)},
    }


# ─────────────────────────────── sonda de latencia ─────────────────────────────

def _rpc_timed(url: str, method: str = "eth_chainId", timeout: float = 8.0) -> tuple[bool, float, str]:
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": []}).encode()
    req = urllib.request.Request(url, data=body,
                                 headers={"content-type": "application/json",
                                          "accept": "application/json", "user-agent": UA})
    t0 = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            d = json.loads(r.read().decode("utf-8"))
        dt = (time.perf_counter() - t0) * 1000.0
        if d.get("result") is not None:
            return True, dt, ""
        return False, dt, str(d.get("error"))[:120]
    except Exception as exc:  # noqa: BLE001
        return False, (time.perf_counter() - t0) * 1000.0, f"{type(exc).__name__}: {exc}"[:120]


def probe_url(url: str, samples: int = 5, timeout: float = 8.0) -> dict:
    lat: list[float] = []
    last_err = ""
    for _ in range(samples):
        ok, dt, err = _rpc_timed(url, timeout=timeout)
        if ok:
            lat.append(dt)
        else:
            last_err = err
    if not lat:
        return {"ok": False, "n_ok": 0, "p50_ms": None, "p95_ms": None, "error": last_err or "sin respuesta"}
    lat_sorted = sorted(lat)
    p50 = statistics.median(lat_sorted)
    p95 = lat_sorted[min(len(lat_sorted) - 1, int(round(0.95 * (len(lat_sorted) - 1))))]
    return {"ok": True, "n_ok": len(lat), "p50_ms": round(p50, 1), "p95_ms": round(p95, 1), "error": ""}


def probe_catalog(catalog: dict, chains: Optional[list[str]], samples: int, timeout: float) -> dict:
    result: dict[str, list[dict]] = {}
    for chain, entries in catalog.get("chains", {}).items():
        if chains and chain not in chains:
            continue
        scored = []
        for e in entries:
            if e["protocol"] != "HTTP":
                continue  # la sonda mide HTTP; WSS se ordena aparte, sin fingir
            if e.get("url_has_env_var"):
                scored.append({**e, "ok": None, "p50_ms": None, "p95_ms": None,
                               "error": "requiere ${VAR}: no se sondea sin clave"})
                continue
            m = probe_url(e["url"], samples, timeout)
            scored.append({**e, **m})
        # orden: los medidos por p50 ascendente; al final los no medidos
        scored.sort(key=lambda s: (s["p50_ms"] is None, s["p50_ms"] if s["p50_ms"] is not None else 1e9))
        for i, s in enumerate(scored):
            s["rank"] = i + 1
            s["use"] = ("primary" if i == 0 and s["p50_ms"] is not None
                        else ("failover" if s["p50_ms"] is not None else "unprobed"))
        result[chain] = scored
    return {"_comment": "Orden de conexión por LATENCIA MEDIDA (p50). primary = el más rápido medido.",
            "samples": samples, "chains": result}


def main() -> int:
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser(description="ARBI RPC catalog + latency probe")
    sub = ap.add_subparsers(dest="cmd", required=True)

    pe = sub.add_parser("extract", help="Excel -> catálogo canónico saneado")
    pe.add_argument("--xlsm", required=True)
    pe.add_argument("--out", default=os.path.join(here, "rpc_catalog.json"))

    pp = sub.add_parser("probe", help="catálogo -> latencias medidas y orden de conexión")
    pp.add_argument("--catalog", default=os.path.join(here, "rpc_catalog.json"))
    pp.add_argument("--chain", action="append", default=None)
    pp.add_argument("--samples", type=int, default=5)
    pp.add_argument("--timeout", type=float, default=8.0)
    pp.add_argument("--out", default="rpc_latency.json")

    args = ap.parse_args()

    if args.cmd == "extract":
        entries = extract(args.xlsm)
        cat = build_catalog(entries)
        with open(args.out, "w", encoding="utf-8") as fh:
            json.dump(cat, fh, indent=2, ensure_ascii=False)
        total = sum(len(v) for v in cat["chains"].values())
        print(f"cadenas={len(cat['chains'])} rpcs={total} secretos_saneados={cat['secrets_sanitized']} -> {args.out}")
        return 0

    cat = json.load(open(args.catalog, encoding="utf-8"))
    res = probe_catalog(cat, args.chain, args.samples, args.timeout)
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(res, fh, indent=2, ensure_ascii=False)
    for chain, entries in res["chains"].items():
        print(f"\n== {chain}")
        for e in entries:
            if e["p50_ms"] is None:
                print(f"   - {e['provider']:<11} {e['use']:<8} ({e['error']})")
            else:
                print(f"   {e['rank']:>2}. {e['provider']:<11} {e['use']:<8} p50={e['p50_ms']:>7.1f}ms p95={e['p95_ms']:>7.1f}ms")
    return 0


if __name__ == "__main__":
    sys.exit(main())
