# Kelly (op_16)

## Identity
- **ID**: 16 / op_16
- **Canonical Role**: Capital fraction/risk sizing after calibrated edge.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
f* = (b*p - q)/b - capital fraction/risk sizing
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_16_kelly.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 19
- Matrix: 13_STRAT_OP_MATRIX column `op_16`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
