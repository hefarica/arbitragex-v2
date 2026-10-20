#!/usr/bin/env python3
"""
ARBI — selector dinámico de cadenas por ineficiencia medida.

Objetivo (pedido del dueño 2026-10-19): elegir las MEJORES cadenas para arbitraje
de forma DINÁMICA, por análisis, y actualizar la lista cada hora con un algoritmo
adaptativo que MIDA —no que suponga— dónde está la ineficiencia.

ALGORITMO (dos fases, declaradas, sin fingir madurez):

  FASE 1 — PRIOR MEDIDO (hoy, con `recon total=0`):
    No hay recompensa realizada todavía, así que no se puede "aprender". Se puntúa
    cada cadena con proxies MEDIDOS y se etiqueta la salida `phase="prior-only"`.
    Ninguna fila de esta fase se presenta como rendimiento.

  FASE 2 — ADAPTATIVO (bandit de Thompson):
    Cuando existan operaciones realizadas (PnL neto por cadena), el posterior de
    cada cadena se actualiza en línea y la selección pasa a Thompson sampling:
    se explora donde la incertidumbre es alta y se explota donde el neto es bueno.
    La salida se etiqueta `phase="adaptive"`.

Score del prior (por cadena c):
    prior_c = z(log10 V_c) − γ·z(log10 G_c) − δ·competition_c
  donde
    V_c  = volumen DEX 24 h de la cadena (proxy de densidad de desequilibrios)
    G_c  = coste en USD de UN intento = gas_price_wei · gas_units · native_usd / 1e18
    z()  = z-score robusto (mediana / MAD) sobre las cadenas candidatas
    competition_c = fracción de oportunidades ya capturadas por otros (0..1);
                    hoy es un campo DECLARADO en el config, no medido — se dice.
  Un z-score robusto se usa porque los volúmenes cruzan 6 órdenes de magnitud y la
  media no es un estadístico honesto ahí.

R8 / RULE 00: si una fuente falta, el valor es `null` y la cadena se marca
`excluded_reason`; NUNCA se inventa un cero.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import statistics
import sys
import urllib.error
import urllib.request
from dataclasses import dataclass, field, asdict
from typing import Optional

DEXS_URL = "https://api.llama.fi/overview/dexs"
PRICES_URL = "https://coins.llama.fi/prices/current/"
GAS_UNITS_DEFAULT = 250_000  # igual que gas_estimate_units del trading-config

GAMMA_DEFAULT = 1.0   # peso del coste de gas
DELTA_DEFAULT = 1.0   # peso de la competencia declarada
TOPK_DEFAULT = 10
SEED_DEFAULT = 20261019


# ─────────────────────────────── estadística robusta ────────────────────────────

def median_abs_dev(values: list[float]) -> float:
    med = statistics.median(values)
    return statistics.median([abs(v - med) for v in values])


def robust_z(values: list[float]) -> list[float]:
    """z-score robusto: (x − mediana) / (1.4826·MAD).

    Degeneración declarada: si MAD=0 (más de la mitad de los valores idénticos) se
    cae a la desviación típica; si también es 0, se devuelve 0 (no NaN). Sin este
    respaldo, un factor con casi todos los valores iguales desaparecía del score.
    """
    if not values:
        return []
    med = statistics.median(values)
    mad = median_abs_dev(values)
    scale = 1.4826 * mad
    if scale <= 0:
        scale = statistics.pstdev(values)
    if scale <= 0:
        return [0.0 for _ in values]
    return [(v - med) / scale for v in values]


def log10_pos(x: Optional[float]) -> Optional[float]:
    if x is None or x <= 0:
        return None
    return math.log10(x)


# ─────────────────────────────── recolección (medida) ──────────────────────────

UA = "arbi-chain-selector/1.0 (+https://github.com/hefarica/arbitragex-v2)"


def _http_json(url: str, timeout: int = 25):
    req = urllib.request.Request(url, headers={"accept": "application/json", "user-agent": UA})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def _rpc_call(rpc: str, method: str, params: list, timeout: int = 12) -> Optional[str]:
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    # User-Agent explícito: sin él, endpoints tras Cloudflare (p.ej. mainnet.base.org) y
    # algunos publicnode devuelven 403 al UA por defecto de urllib, y el gas salía `null`
    # —una cadena excluida por el transporte, no por el mercado.
    req = urllib.request.Request(rpc, data=body,
                                 headers={"content-type": "application/json",
                                          "accept": "application/json",
                                          "user-agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            d = json.loads(r.read().decode("utf-8"))
        return d.get("result")
    except Exception:
        return None


def fetch_dex_volume_by_chain(timeout: int = 25) -> dict[str, float]:
    """Volumen DEX 24 h por cadena (DefiLlama, sin API key)."""
    d = _http_json(DEXS_URL + "?excludeTotalDataChart=true&excludeTotalDataChartBreakdown=true&dataType=dailyVolume", timeout)
    prot = d.get("protocols") or []
    agg: dict[str, float] = {}
    for p in prot:
        chains = p.get("chains") or []
        v = p.get("total24h") or 0
        if not chains:
            continue
        share = v / len(chains)
        for c in chains:
            agg[c] = agg.get(c, 0.0) + share
    return agg


def fetch_native_prices(coingecko_ids: list[str], timeout: int = 20) -> dict[str, Optional[float]]:
    ids = [i for i in dict.fromkeys(coingecko_ids) if i]
    if not ids:
        return {}
    try:
        d = _http_json(PRICES_URL + ",".join(ids), timeout)
    except Exception:
        return {i: None for i in ids}
    coins = d.get("coins") or {}
    return {i: (coins.get(i, {}) or {}).get("price") for i in ids}


def fetch_gas_price_wei(rpc: str) -> Optional[int]:
    res = _rpc_call(rpc, "eth_gasPrice", [])
    if not res:
        return None
    try:
        return int(res, 16)
    except (TypeError, ValueError):
        return None


# ─────────────────────────────── modelo y score ────────────────────────────────

@dataclass
class ChainState:
    name: str
    llama_name: str
    chain_id: int
    rpc: str
    native: str
    gas_units: int = GAS_UNITS_DEFAULT
    competition: float = 0.0
    # medidos
    volume_24h_usd: Optional[float] = None
    gas_price_wei: Optional[int] = None
    native_usd: Optional[float] = None
    gas_usd: Optional[float] = None
    prior: Optional[float] = None
    posterior_mean: Optional[float] = None
    posterior_sd: Optional[float] = None
    weight: Optional[float] = None
    reward_n: int = 0
    excluded_reason: Optional[str] = None


def compute_gas_usd(gas_price_wei: Optional[int], gas_units: int, native_usd: Optional[float]) -> Optional[float]:
    if gas_price_wei is None or native_usd is None:
        return None
    return gas_price_wei * gas_units * native_usd / 1e18


def compute_priors(chains: list[ChainState], gamma: float = GAMMA_DEFAULT, delta: float = DELTA_DEFAULT) -> None:
    """Calcula `prior` in-place. Las cadenas sin volumen o sin gas quedan excluidas con causa."""
    usable: list[ChainState] = []
    for c in chains:
        if c.volume_24h_usd is None:
            c.excluded_reason = "sin volumen DEX medido"
            continue
        if c.gas_usd is None:
            c.excluded_reason = "sin coste de gas medido (RPC o precio nativo no disponible)"
            continue
        usable.append(c)
    if not usable:
        return
    z_vol = robust_z([log10_pos(c.volume_24h_usd) or 0.0 for c in usable])
    z_gas = robust_z([log10_pos(c.gas_usd) or 0.0 for c in usable])
    for c, zv, zg in zip(usable, z_vol, z_gas):
        c.prior = zv - gamma * zg - delta * c.competition


def softmax(xs: list[float], temperature: float = 1.0) -> list[float]:
    if not xs:
        return []
    t = max(temperature, 1e-9)
    m = max(xs)
    exps = [math.exp((x - m) / t) for x in xs]
    s = sum(exps)
    return [e / s for e in exps]


def thompson_topk(chains: list[ChainState], k: int, rng_seed: int) -> list[ChainState]:
    """Thompson sampling gaussiano: samplea θ_c ~ N(μ_c, σ_c²) y ordena por θ."""
    import random
    rng = random.Random(rng_seed)
    scored = []
    for c in chains:
        if c.prior is None:
            continue
        mu = c.posterior_mean if c.posterior_mean is not None else c.prior
        sd = c.posterior_sd if c.posterior_sd is not None else 1.0
        theta = rng.gauss(mu, sd)
        scored.append((theta, c))
    scored.sort(key=lambda t: t[0], reverse=True)
    return [c for _, c in scored[:k]]


def update_posterior(chains: list[ChainState], rewards: dict[str, list[float]], prior_sd: float = 1.0) -> None:
    """Actualiza μ, σ por cadena con recompensas realizadas (neto USD por operación)."""
    for c in chains:
        rs = rewards.get(c.name) or []
        c.reward_n = len(rs)
        if not rs:
            continue
        n = len(rs)
        mean_r = statistics.fmean(rs)
        base = c.prior if c.prior is not None else 0.0
        # posterior conjugado simple: mezcla prior + observaciones (varianza conocida)
        w = n / (n + prior_sd * prior_sd)
        c.posterior_mean = (1 - w) * base + w * mean_r
        if n > 1:
            c.posterior_sd = max(statistics.pstdev(rs), 1e-6) / math.sqrt(n)
        else:
            c.posterior_sd = prior_sd / math.sqrt(n + 1)


# ─────────────────────────────── orquestación ──────────────────────────────────

def load_config(path: str) -> list[ChainState]:
    with open(path, "r", encoding="utf-8") as fh:
        raw = json.load(fh)
    return [ChainState(**c) for c in raw["chains"]]


def collect(chains: list[ChainState], offline_metrics: Optional[dict] = None) -> None:
    """Rellena volumen, gas y precio nativo. `offline_metrics` permite test sin red."""
    if offline_metrics is not None:
        for c in chains:
            m = offline_metrics.get(c.name, {})
            c.volume_24h_usd = m.get("volume_24h_usd")
            c.gas_price_wei = m.get("gas_price_wei")
            c.native_usd = m.get("native_usd")
            c.gas_usd = compute_gas_usd(c.gas_price_wei, c.gas_units, c.native_usd)
        return
    vol = fetch_dex_volume_by_chain()
    prices = fetch_native_prices([c.native for c in chains])
    for c in chains:
        c.volume_24h_usd = vol.get(c.llama_name)
        c.native_usd = prices.get(c.native)
        c.gas_price_wei = fetch_gas_price_wei(c.rpc)
        c.gas_usd = compute_gas_usd(c.gas_price_wei, c.gas_units, c.native_usd)


def run(config_path: str, rewards_path: Optional[str], k: int, gamma: float, delta: float,
        seed: int, out_path: Optional[str], offline_metrics: Optional[dict] = None) -> dict:
    chains = load_config(config_path)
    collect(chains, offline_metrics)

    rewards: dict[str, list[float]] = {}
    if rewards_path and os.path.exists(rewards_path):
        with open(rewards_path, "r", encoding="utf-8") as fh:
            rewards = json.load(fh).get("rewards", {})
    phase = "adaptive" if any(rewards.get(c.name) for c in chains) else "prior-only"

    compute_priors(chains, gamma, delta)
    update_posterior(chains, rewards)

    usable = [c for c in chains if c.prior is not None]
    top = thompson_topk(usable, k, seed)
    weights = softmax([c.posterior_mean if c.posterior_mean is not None else c.prior for c in usable])
    wmap = {c.name: w for c, w in zip(usable, weights)}
    for c in chains:
        c.weight = wmap.get(c.name)

    ranking = sorted(usable, key=lambda c: (c.posterior_mean if c.posterior_mean is not None else c.prior), reverse=True)
    result = {
        "phase": phase,
        "note": ("prior-only: no hay recompensas realizadas (recon total=0); la lista es un PRIOR MEDIDO, "
                 "no rendimiento" if phase == "prior-only" else
                 "adaptive: la selección usa recompensas realizadas"),
        "params": {"gamma": gamma, "delta": delta, "k": k, "seed": seed, "gas_units": GAS_UNITS_DEFAULT},
        "top_k": [c.name for c in top],
        "chains": [asdict(c) for c in sorted(chains, key=lambda c: (c.prior is None, -(c.prior or 0)))],
    }
    if out_path:
        with open(out_path, "w", encoding="utf-8") as fh:
            json.dump(result, fh, indent=2, ensure_ascii=False)
    return result


def main() -> int:
    ap = argparse.ArgumentParser(description="ARBI chain selector")
    here = os.path.dirname(os.path.abspath(__file__))
    ap.add_argument("--config", default=os.path.join(here, "chains.json"))
    ap.add_argument("--rewards", default=os.path.join(here, "rewards.json"))
    ap.add_argument("--k", type=int, default=TOPK_DEFAULT)
    ap.add_argument("--gamma", type=float, default=GAMMA_DEFAULT)
    ap.add_argument("--delta", type=float, default=DELTA_DEFAULT)
    ap.add_argument("--seed", type=int, default=SEED_DEFAULT)
    ap.add_argument("--out", default="chain-ranking.json")
    args = ap.parse_args()

    res = run(args.config, args.rewards, args.k, args.gamma, args.delta, args.seed, args.out)
    print(f"phase={res['phase']} k={args.k}")
    print(f"{'chain':<12}{'vol24h_usd':>16}{'gas_usd':>12}{'prior':>9}{'post_mean':>11}{'n':>4}")
    for c in res["chains"]:
        if c["prior"] is None:
            print(f"{c['name']:<12}{'--':>16}{'--':>12}{'--':>9}{'--':>11}{c['reward_n']:>4}  ({c['excluded_reason']})")
            continue
        vol = c["volume_24h_usd"] or 0
        print(f"{c['name']:<12}{vol:>16,.0f}{c['gas_usd']:>12.4f}{c['prior']:>9.3f}"
              f"{(c['posterior_mean'] if c['posterior_mean'] is not None else c['prior']):>11.3f}{c['reward_n']:>4}")
    print("TOP-" + str(args.k) + ": " + ", ".join(res["top_k"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
