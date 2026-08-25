# Simplex (op_19)

## Identity
- **ID**: 19 / op_19
- **Canonical Role**: LP/simplex allocation and batch/split routing.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
min c^T*x s.t. Ax <= b - LP allocation and batch optimization
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_19_simplex.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 22
- Matrix: 13_STRAT_OP_MATRIX column `op_19`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
