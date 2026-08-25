# Newton-Raphson (op_21)

## Identity
- **ID**: 21 / op_21
- **Canonical Role**: Root/break-even/invariant solve.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
x_{n+1} = x_n - f(x_n)/f_prime(x_n) - root/break-even solving
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_21_newton.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 24
- Matrix: 13_STRAT_OP_MATRIX column `op_21`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
