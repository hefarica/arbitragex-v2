"""Exact reference arithmetic, not a production quote or EVM simulation.

All inputs must be supplied explicitly from a verified snapshot. No RPC, signer,
price fallback, default fee, probability threshold or capital setting is installed.
"""
from __future__ import annotations
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation, localcontext
from fractions import Fraction
from typing import Any, Iterable

U256_MAX = (1 << 256) - 1


def uint(value: Any, bits: int = 256) -> int:
    if isinstance(value, bool) or not isinstance(value, (str, int)):
        raise ValueError("integer required; floats and booleans are not raw amounts")
    if isinstance(value, str) and (not value or not value.isascii() or not value.isdigit()):
        raise ValueError("unsigned base-10 integer string required")
    number = int(value)
    if number < 0 or number >= 1 << bits:
        raise ValueError(f"value outside uint{bits}")
    return number


def decimal(value: Any) -> Decimal:
    if isinstance(value, (bool, float)) or value is None:
        raise ValueError("exact decimal string or integer required")
    try:
        d = Decimal(value)
    except (InvalidOperation, ValueError, TypeError) as exc:
        raise ValueError("invalid decimal") from exc
    if not d.is_finite():
        raise ValueError("non-finite value")
    return d


def v2_amount_out(amount_in: int | str, reserve_in: int | str,
                  reserve_out: int | str, fee_numerator: int,
                  fee_denominator: int) -> int:
    """CPMM exact-in, fee_numerator is the charged fraction (e.g. 3/1000).

    Uses floor rounding. Zero output is a real result, NOT missing data. Inputs
    and intermediate arithmetic are checked as Solidity SafeMath uint256.
    Caller supplies the fee proven for this pool; no default fee is assumed.
    """
    x, ri, ro = uint(amount_in), uint(reserve_in), uint(reserve_out)
    fee_numerator, fee_denominator = uint(fee_numerator), uint(fee_denominator)
    if ri == 0 or ro == 0 or fee_denominator == 0 or fee_numerator >= fee_denominator:
        raise ValueError("invalid reserves/fee")
    if x == 0:
        return 0
    x_fee = x * (fee_denominator - fee_numerator)
    numerator = x_fee * ro
    denominator = ri * fee_denominator + x_fee
    if max(x_fee, numerator, denominator) > U256_MAX:
        raise ValueError("uint256 intermediate overflow")
    return numerator // denominator


@dataclass(frozen=True)
class PoolSnapshot:
    pool_id: str
    token0: str
    token1: str
    reserve0: int
    reserve1: int
    fee_numerator: int
    fee_denominator: int
    chain_id: int
    block_hash: str


@dataclass(frozen=True)
class Hop:
    pool: PoolSnapshot
    token_in: str
    token_out: str


def quote_cycle(amount_in: int, hops: Iterable[Hop], max_hops: int) -> dict[str, Any]:
    """Exact hypothetical V2 ledger at one chain/block. Not executable approval.

    Reusing a physical pool updates its reserves; contradicting pool snapshots,
    open paths and mixed-block inputs are refused. Per-hop quantities are raw
    token units; no summation across different tokens and no USD invention.
    """
    hops = list(hops)
    x = uint(amount_in)
    if x == 0 or not 2 <= len(hops) <= max_hops:
        raise ValueError("positive principal and admissible hop count required")
    first = hops[0]
    if first.token_in != hops[-1].token_out:
        raise ValueError("open path: profit in unlike assets cannot be subtracted")
    seen: dict[str, PoolSnapshot] = {}
    state: dict[str, list[int]] = {}
    ledger = []
    current = x
    for i, hop in enumerate(hops):
        p = hop.pool
        if (p.chain_id, p.block_hash) != (first.pool.chain_id, first.pool.block_hash):
            raise ValueError("snapshot mismatch")
        if p.token0 == p.token1 or not p.pool_id or not p.block_hash:
            raise ValueError("invalid pool identity")
        if i and hops[i - 1].token_out != hop.token_in:
            raise ValueError("discontinuous path")
        if {hop.token_in, hop.token_out} != {p.token0, p.token1}:
            raise ValueError("token/pool mismatch")
        if p.pool_id in seen and seen[p.pool_id] != p:
            raise ValueError("contradicting initial snapshot of repeated pool")
        seen[p.pool_id] = p
        if p.pool_id not in state:
            state[p.pool_id] = [uint(p.reserve0), uint(p.reserve1)]
        reserves = state[p.pool_id]
        j = 0 if hop.token_in == p.token0 else 1
        out = v2_amount_out(current, reserves[j], reserves[1 - j],
                            p.fee_numerator, p.fee_denominator)
        ledger.append({"index": i, "pool_id": p.pool_id, "token_in": hop.token_in,
                       "token_out": hop.token_out, "amount_in_raw": str(current),
                       "amount_out_raw": str(out), "fee_numerator": p.fee_numerator,
                       "fee_denominator": p.fee_denominator})
        reserves[j] = uint(reserves[j] + current)
        reserves[1 - j] -= out
        current = out
    return {"basis": "exact_v2_reference_quote", "chain_id": first.pool.chain_id,
            "block_hash": first.pool.block_hash, "amount_in_raw": str(x),
            "amount_out_raw": str(current), "gross_profit_raw": str(current - x),
            "leg_ledger": ledger, "execution_authorized": False}


def sqrt_x96_price(sqrt_price_x96: int | str, decimals0: int, decimals1: int) -> Fraction:
    """token1 per token0 in human units. A spot ratio, NEVER a finite-size quote."""
    q = uint(sqrt_price_x96, 160)
    if q == 0:
        raise ValueError("zero square-root price")
    d0, d1 = uint(decimals0, 8), uint(decimals1, 8)
    return Fraction(q * q, 1 << 192) * Fraction(10 ** d0, 10 ** d1)


def fee_rate(value: int, unit: str) -> Fraction:
    divisors = {"bps": 10_000, "pips": 1_000_000}
    if unit not in divisors:
        raise ValueError("explicit bps or pips required")
    value = uint(value)
    if value >= divisors[unit]:
        raise ValueError("fee must be less than 100%")
    return Fraction(value, divisors[unit])


def economic_ledger(principal_usd: str, final_out_usd: str,
                    external_costs: dict[str, str], quote_includes: Iterable[str]) -> dict[str, str]:
    """Signed net = out - principal - EXTERNAL costs; never subtract principal twice.

    quote_includes explicitly names costs already embedded in the quoted output
    (typically lp_fees and price impact), which cannot be subtracted again.
    Model/estimated cost provenance is validated by the enclosing receipt.
    """
    principal, out = decimal(principal_usd), decimal(final_out_usd)
    if principal <= 0 or out < 0:
        raise ValueError("invalid principal/output")
    included = set(quote_includes)
    if included & set(external_costs):
        raise ValueError("double-counted quoted costs")
    costs = {k: decimal(v) for k, v in external_costs.items()}
    if any(v < 0 for v in costs.values()):
        raise ValueError("negative external cost")
    with localcontext() as ctx:
        ctx.prec = 80
        gross = out - principal
        total = sum(costs.values(), Decimal(0))
        net = gross - total
        return {"principal_usd": str(principal), "gross_profit_usd": str(gross),
                "external_cost_usd": str(total), "net_profit_usd": str(net),
                "roi_bps": str(net / principal * Decimal(10000)),
                "scope": "supplied_cost_components_only_not_cost_completeness_proof"}


def homogeneous_prices(samples: list[dict[str, Any]], axis: str) -> list[Decimal]:
    """Refuse unrelated token ratios disguised as one temporal price series."""
    if not samples or axis not in {"time", "venue"}:
        raise ValueError("non-empty typed axis required")
    required = {"base_asset", "quote_asset", "price", "timestamp_ms", "block_hash", "venue"}
    for s in samples:
        if not required <= set(s):
            raise ValueError("missing price identity")
    pair = (samples[0]["base_asset"], samples[0]["quote_asset"])
    if any((s["base_asset"], s["quote_asset"]) != pair for s in samples):
        raise ValueError("heterogeneous asset units")
    if axis == "venue" and len({s["block_hash"] for s in samples}) != 1:
        raise ValueError("cross-venue snapshot mismatch")
    if axis == "time":
        if len({s["venue"] for s in samples}) != 1:
            raise ValueError("temporal series changes venue")
        if any(b["timestamp_ms"] <= a["timestamp_ms"] for a, b in zip(samples, samples[1:])):
            raise ValueError("temporal samples not strictly ordered")
    values = [decimal(s["price"]) for s in samples]
    if any(v <= 0 for v in values):
        raise ValueError("non-positive price")
    return values
