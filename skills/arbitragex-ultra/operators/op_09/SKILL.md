# Lévy (op_09)

## Identity
- **ID**: 9 / op_09
- **Canonical Role**: Heavy-tail/jump-risk evidence.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
psi(u) = i*mu*u - sigma^2*u^2/2 - heavy-tail jump-risk evidence
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_09_levy.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 12
- Matrix: 13_STRAT_OP_MATRIX column `op_09`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
