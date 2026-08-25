# Bayes (op_11)

## Identity
- **ID**: 11 / op_11
- **Canonical Role**: Posterior probability/evidence.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
P(theta|D) proportional to L(D|theta)*P(theta) - posterior probability
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_11_bayes.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 14
- Matrix: 13_STRAT_OP_MATRIX column `op_11`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
