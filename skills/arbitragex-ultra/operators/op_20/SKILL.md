# Gradient Descent (op_20)

## Identity
- **ID**: 20 / op_20
- **Canonical Role**: Continuous multi-variable optimization.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
theta_{t+1} = theta_t - eta*grad(J) - continuous optimization
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_20_gradient_descent.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 23
- Matrix: 13_STRAT_OP_MATRIX column `op_20`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
