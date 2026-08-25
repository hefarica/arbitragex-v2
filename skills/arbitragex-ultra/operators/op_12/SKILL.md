# MLE (op_12)

## Identity
- **ID**: 12 / op_12
- **Canonical Role**: Parameter estimation.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
theta_hat = argmax L(theta|x) - parameter estimation
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_12_mle.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 15
- Matrix: 13_STRAT_OP_MATRIX column `op_12`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
