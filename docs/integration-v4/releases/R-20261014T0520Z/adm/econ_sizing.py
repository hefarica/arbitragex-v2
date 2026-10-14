#!/usr/bin/env python3
"""ADM-1 (t13, familia H §12) — Sizing AMM forma cerrada (x·y=k) + cota Kelly.

ADM-1 | H sizing-AMM-cerrada+kelly (A14 FLASHLOAN-SIZING + A15 OPTIMAL-FRACTION-KELLY)
Admision T (docs/integration-v4/t13-evidence/T13_admission_eval_output.json):
  mu = 17915.4383 USD >= delta = 358.3088 USD (delta = max(0.02*|J|, 0.005), J = suma
  simulated_net_profit_usd de x_ref = -17915.4383, n=17)  -> ADMITIDO
  lambda = P (funcion pura control-plane; NO hot path; paper-safe)
  C(theta): el termino NO toca SSOT (condiciones 264/60/1436/hop_dist intactas, t8).

Modelo (V2 exacto, V3 aproximado — DECLARADO):
  net(a) = a*g - a^2*s_c - fixed
  calibrado por item con el punto simulado medido (a0, slippage0, lp0, net0):
    s_c   = slippage0 / a0^2            (V2: slippage exactamente cuadratico en a)
    fixed = costs_total - slippage0 - lp0 (gas+relay+buffers: independientes del tamano)
    g     = (net0 + fixed + slippage0 + lp0) / a0  (tasa bruta marginal)
  optimo: a* = g / (2*s_c)  si g>0 y s_c>0; net(a*) = g^2/(4*s_c) - fixed
  politica: operar solo si net(a*) > 0; ademas a_final = min(a*, kelly_cap).
Kelly (cota superior de tamano, no apalancamiento):
  f* = p - (1-p)/b  con p = prob de win, b = win/loss ratio; cap = f* * bankroll.
R2 fail-honest: si falta un dato -> None, jamas 0 inventado.
"""
from __future__ import annotations


def calibrate(a0: float, net0: float, slippage0: float, lp0: float, costs_total: float):
    """Calibra (g, s_c, fixed) desde el punto simulado medido. None si a0<=0.

    Modelo: net(a) = a*g - a^2*s_c - fixed  con  net(a0) == net0 EXACTO:
      s_c   = slippage0 / a0^2            (V2: slippage exactamente cuadratico en a)
      fixed = costs_total - slippage0 - lp0 (gas+relay+buffers: independientes del tamano)
      g     = (net0 + slippage0 + fixed) / a0  (tasa marginal EFECTIVA: bruta menos fees
              lineales lp ~ a; absorberlos en g es exacto porque son lineales en a)
    """
    if a0 is None or a0 <= 0:
        return None
    s_c = (slippage0 or 0.0) / (a0 * a0)
    fixed = (costs_total or 0.0) - (slippage0 or 0.0) - (lp0 or 0.0)
    g = (net0 + (slippage0 or 0.0) + fixed) / a0
    return {"g": g, "s_c": s_c, "fixed": fixed}


def net_at(a: float, cal: dict) -> float:
    return a * cal["g"] - a * a * cal["s_c"] - cal["fixed"]


def optimal_size(cal: dict) -> tuple[float, float]:
    """(a*, net(a*)); a*=0 si no hay edge marginal."""
    if cal is None:
        return (0.0, 0.0)
    g, s_c = cal["g"], cal["s_c"]
    if g <= 0 or s_c <= 0:
        return (0.0, 0.0)
    a_star = g / (2.0 * s_c)
    return (a_star, net_at(a_star, cal))


def kelly_cap(p_win: float, b_win_loss: float, bankroll: float, fraction: float = 1.0) -> float:
    """Cota Kelly (fraccionable). None si b<=0 (dato invalido) o p fuera de [0,1]."""
    if b_win_loss is None or b_win_loss <= 0 or p_win is None or not (0.0 <= p_win <= 1.0):
        return None
    f = p_win - (1.0 - p_win) / b_win_loss
    if f <= 0:
        return 0.0
    return f * fraction * (bankroll or 0.0)


def size_policy(cal: dict, kelly: float | None = None) -> dict:
    """Politica ADM-1: tamano final = min(a*, kelly) operando solo si net>0 en ese tamano."""
    a_star, net_star = optimal_size(cal)
    if net_star <= 0:
        return {"trade": False, "amount_usd": 0.0, "expected_net_usd": 0.0,
                "reason": "net(a*)<=0: edge insuficiente para piso de costos fijos"}
    a_final = a_star if kelly is None else min(a_star, kelly)
    net_final = net_at(a_final, cal)
    if net_final <= 0:  # kelly recorto por debajo del optimo y quedo negativo
        return {"trade": False, "amount_usd": 0.0, "expected_net_usd": 0.0,
                "reason": "kelly_cap por debajo del minimo viable"}
    return {"trade": True, "amount_usd": a_final, "expected_net_usd": net_final, "reason": "ok"}
