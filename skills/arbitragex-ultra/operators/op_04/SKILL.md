# Von Neumann entropy (op_04)

## Identity
- **ID**: 4 / op_04
- **Canonical Role**: Entropy/coherence evidence.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
S(rho) = -Tr(rho ln rho) - entropy/coherence evidence
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_04_von_neumann.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 7
- Matrix: 13_STRAT_OP_MATRIX column `op_04`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
