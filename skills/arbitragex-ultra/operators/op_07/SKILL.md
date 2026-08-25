# HMM (op_07)

## Identity
- **ID**: 7 / op_07
- **Canonical Role**: Hidden regime / finality-latency state.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
P(X_t|O_{1:t}) = alpha_t(i) - hidden regime inference
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_07_hmm.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 10
- Matrix: 13_STRAT_OP_MATRIX column `op_07`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
