# Markov Chain (op_06)

## Identity
- **ID**: 6 / op_06
- **Canonical Role**: Cross-domain/state transition regime.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
P(X_{n+1}=j|X_n=i) = P_{ij} - state transition probabilities
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_06_markov_chain.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 9
- Matrix: 13_STRAT_OP_MATRIX column `op_06`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
