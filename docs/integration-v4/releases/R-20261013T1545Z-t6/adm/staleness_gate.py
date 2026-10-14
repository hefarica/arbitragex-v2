#!/usr/bin/env python3
"""ADM-3 (t13, familia I §12) — Gate de staleness de oraculo (cuarentena de precios viejos).

ADM-3 | I staleness-oraculo
Admision T: mu = 2/26 = 0.0769 >= delta_x5 = 0.02 (cobertura de clase previsible en la
muestra congelada; T = max(p95 muestral, 2405ms) medido en x_ref/substrato) -> ADMITIDO
lambda = P (gate puro de control-plane; no ejecuta nada; paper-safe)
C(theta): no toca SSOT.

Eje: x5 (clases previsibles de riesgo: precio con pipeline_latency alto => seleccion
adversa esperada). Umbral desde env ARBX_STALENESS_MS (R1: nada hardcodeado); si env
ausente => None y el gate declara NO COMPUTADO (R2), jamas aplica un default oculto.
"""
from __future__ import annotations
import os


def staleness_threshold_ms(env: dict | None = None) -> float | None:
    env = dict(os.environ if env is None else env)
    raw = env.get("ARBX_STALENESS_MS")
    if raw is None or str(raw).strip() == "":
        return None
    try:
        v = float(raw)
    except ValueError:
        return None
    return v if v > 0 else None


def gate(item: dict, threshold_ms: float | None) -> dict:
    """Cuarentena si pipeline_latency_ms > threshold_ms. Fail-honest si falta dato."""
    lat = item.get("pipeline_latency_ms")
    if lat is None:
        return {"quarantine": None, "reason": "pipeline_latency_ms ausente: NO COMPUTADO"}
    if threshold_ms is None:
        return {"quarantine": None, "reason": "ARBX_STALENESS_MS sin definir: NO COMPUTADO"}
    q = float(lat) > float(threshold_ms)
    return {"quarantine": q,
            "reason": f"latencia {lat}ms {'>' if q else '<='} umbral {threshold_ms}ms"}


def apply(items: list, threshold_ms: float | None) -> dict:
    results = [gate(i, threshold_ms) for i in items]
    q = sum(1 for r in results if r["quarantine"] is True)
    nc = sum(1 for r in results if r["quarantine"] is None)
    return {"n": len(items), "quarantined": q, "not_computed": nc,
            "passing": len(items) - q - nc, "threshold_ms": threshold_ms}
