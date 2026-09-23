"""Bound, ID-addressed evidence: 32 operators != 29 projection dimensions.

This evaluator accepts only a new explicit contract. Legacy strategy-wide cache
snapshots cannot be promoted to opportunity evidence by inventing a context id.
The result is a calibrated model output, not a promise of realized profit.
"""
from __future__ import annotations
from decimal import Decimal, localcontext
from typing import Any
from .integrity import HASH, operator_coverage
from .math_reference import decimal


def evaluate_evidence(snapshot: dict[str, Any], expected_context: str,
                      required_ids: list[int], registered_ids: list[int],
                      disabled_ids: list[int], prior_log_odds: str | None,
                      calibration: dict[str, Any] | None) -> dict[str, Any]:
    absent = {"posterior_log_odds":None,"posterior_probability":None,
              "calibration_applied":False,"execution_authorized":False}
    if snapshot.get('schema') != 'arbx.bound-evidence.v1':
        return {**absent,'state':'legacy_or_unknown_schema','reason':'Legacy object/array has no event-bound provenance'}
    if not HASH.fullmatch(expected_context) or snapshot.get('context_id') != expected_context:
        return {**absent,'state':'context_mismatch','reason':'Evidence does not belong to this input snapshot'}
    outputs=snapshot.get('operators')
    if not isinstance(outputs,list):
        return {**absent,'state':'invalid','reason':'operators must be an ID-addressed array'}
    if any(not isinstance(o,dict) or not isinstance(o.get('operator_id'),int) or isinstance(o.get('operator_id'),bool) for o in outputs):
        return {**absent,'state':'invalid','reason':'invalid operator entry'}
    coverage=operator_coverage(required_ids,registered_ids,disabled_ids,outputs,expected_context)
    # Strict audit completeness only. Does not change any live admission gate.
    if not required_ids or coverage['computed'] != coverage['required'] or coverage['undeclared_results']:
        return {**absent,'state':'incomplete','coverage':coverage,'reason':'No imputation for disabled/missing/invalid operators'}
    if prior_log_odds is None or calibration is None:
        return {**absent,'state':'uncalibrated','coverage':coverage,'reason':'No model result without supplied calibration and prior'}
    if not calibration.get('version') or calibration.get('evidence_schema') != 'arbx.bound-evidence.v1':
        return {**absent,'state':'invalid_calibration','reason':'Calibration version/schema missing'}
    weights=calibration.get('weights_by_operator')
    if not isinstance(weights,dict) or any(str(i) not in weights for i in required_ids):
        return {**absent,'state':'incomplete_calibration','reason':'Missing weights are not zero weights'}
    try:
        with localcontext() as ctx:
            ctx.prec=64
            lo=decimal(prior_log_odds)
            contributions={str(o['operator_id']):decimal(weights[str(o['operator_id'])])*decimal(o['value']) for o in coverage['operators']}
            lo+=sum(contributions.values(),Decimal(0))
            # Stable logistic avoids exp(large positive values).
            z=(-abs(lo)).exp()
            probability=Decimal(1)/(Decimal(1)+z) if lo>=0 else z/(Decimal(1)+z)
            applied=any(decimal(weights[str(i)]) != 0 for i in required_ids)
    except (ValueError, ArithmeticError) as exc:
        return {**absent,'state':'invalid_numeric_input','reason':type(exc).__name__}
    return {'state':'computed_model','context_id':expected_context,'coverage':coverage,
            'posterior_log_odds':str(lo),'posterior_probability':str(probability),
            'calibration_applied':applied,'calibration_version':calibration['version'],
            'contributions':{k:str(v) for k,v in contributions.items()},
            'semantics':'Versioned log-linear model; probabilistic calibration must be validated externally',
            'execution_authorized':False}
