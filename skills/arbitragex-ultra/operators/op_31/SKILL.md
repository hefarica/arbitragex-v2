# DRL Agent (op_31)

## Identity
- **ID**: 31 / op_31
- **Canonical Role**: Policy model only when trained/calibrated; otherwise unavailable.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
grad J = E[grad log pi_theta(a|s) * A_t] - policy model (trained only)
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_31_drl_agent.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 34
- Matrix: 13_STRAT_OP_MATRIX column `op_31`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
