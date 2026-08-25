# KL divergence (op_14)

## Identity
- **ID**: 14 / op_14
- **Canonical Role**: Distribution/logical divergence.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
D_KL(P||Q) = Sum P*ln(P/Q) - distribution divergence
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_14_kl_divergence.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 17
- Matrix: 13_STRAT_OP_MATRIX column `op_14`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
