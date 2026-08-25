# GNN Encoder (op_30)

## Identity
- **ID**: 30 / op_30
- **Canonical Role**: Graph/path pruning representation.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
h_v^(k+1) = sigma(W * Sum h_u/||h_u||) - graph pruning representation
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_30_gnn_encoder.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 33
- Matrix: 13_STRAT_OP_MATRIX column `op_30`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
