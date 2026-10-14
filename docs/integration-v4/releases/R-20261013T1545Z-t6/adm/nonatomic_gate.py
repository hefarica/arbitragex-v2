#!/usr/bin/env python3
"""ADM-4 (t13, familia I §12) — Gate de ventanas no atomicas (cuarentena de clase).

ADM-4 | I ventanas-no-atomicas
Admision T: mu = 30000/264000 = 0.113636 >= delta_x5 = 0.02 (fraccion de la clase
NONATOMIC_BRIDGE_REQUIRED en TESTS_ATOMICOS del A8, sha eb5b803c...) -> ADMITIDO
lambda = P (gate de clasificacion; no ejecuta; paper-safe)
C(theta): no toca SSOT.

Eje: x5 (clases previsibles: NONATOMIC = riesgo de ventana/reorg entre txs).
Taxonomia A8 hoja F13: DETERMINISTIC_EXECUTABLE | DERIVATIVE_DATA_REQUIRED |
NONATOMIC_BRIDGE_REQUIRED | SIM_REQUIRED | OBSERVE_ONLY.
Regla: cuarentena si exec_class == NONATOMIC_BRIDGE_REQUIRED (o bridge declarado).
"""
from __future__ import annotations

NONATOMIC = "NONATOMIC_BRIDGE_REQUIRED"


def gate(item: dict) -> dict:
    ec = item.get("exec_class")
    bridge = item.get("bridge")
    if ec == NONATOMIC or (bridge not in (None, "", False)):
        return {"quarantine": True,
                "reason": f"clase {ec or '-'} con bridge={bridge!r}: ventana no atomica"}
    if ec is None:
        return {"quarantine": None, "reason": "exec_class ausente: NO COMPUTADO"}
    return {"quarantine": False, "reason": f"clase {ec}: dentro de politica"}


def apply(items: list) -> dict:
    results = [gate(i) for i in items]
    q = sum(1 for r in results if r["quarantine"] is True)
    nc = sum(1 for r in results if r["quarantine"] is None)
    return {"n": len(items), "quarantined": q, "not_computed": nc,
            "passing": len(items) - q - nc}
