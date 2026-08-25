# Monte Carlo (op_22)

## Identity
- **ID**: 22 / op_22
- **Canonical Role**: Scenario/risk/latency Monte Carlo.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
mu_hat = (1/N)*Sum f(X_i) - scenario/risk/latency estimation
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_22_monte_carlo.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 25
- Matrix: 13_STRAT_OP_MATRIX column `op_22`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
