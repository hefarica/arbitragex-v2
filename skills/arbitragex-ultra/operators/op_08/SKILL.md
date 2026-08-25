# Kalman (op_08)

## Identity
- **ID**: 8 / op_08
- **Canonical Role**: Filtered fair-value / innovation signal.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
x_hat_{k|k} = x_hat_{k|k-1} + K_k(z_k - H*x_hat_{k|k-1}) - filtered fair-value
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_08_kalman.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 11
- Matrix: 13_STRAT_OP_MATRIX column `op_08`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
