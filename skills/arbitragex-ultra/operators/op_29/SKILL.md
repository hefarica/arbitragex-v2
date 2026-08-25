# Shapley (op_29)

## Identity
- **ID**: 29 / op_29
- **Canonical Role**: Surplus attribution / cooperative allocation.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
phi_i = Sum [v(S+{i}) - v(S)] * |S|!*(|N|-|S|-1)!/|N|! - surplus attribution
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_29_shapley.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 32
- Matrix: 13_STRAT_OP_MATRIX column `op_29`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
