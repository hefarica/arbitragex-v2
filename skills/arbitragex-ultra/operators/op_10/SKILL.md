# Welford (op_10)

## Identity
- **ID**: 10 / op_10
- **Canonical Role**: Online volatility/mean/variance.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
M_k = M_{k-1} + (x_k - M_{k-1})/k - online volatility/mean
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_10_welford.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 13
- Matrix: 13_STRAT_OP_MATRIX column `op_10`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
