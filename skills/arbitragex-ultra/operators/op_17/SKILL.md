# Pontryagin (op_17)

## Identity
- **ID**: 17 / op_17
- **Canonical Role**: Constrained dynamic-control sizing.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
H = L + lambda^T*f - constrained dynamic-control sizing
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_17_pontryagin.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 20
- Matrix: 13_STRAT_OP_MATRIX column `op_17`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
