# Nash (op_24)

## Identity
- **ID**: 24 / op_24
- **Canonical Role**: Strategic solver/auction interaction.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
u_i(sigma_i*, sigma_-i*) >= u_i(sigma_i, sigma_-i*) - strategic equilibrium
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_24_nash.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 27
- Matrix: 13_STRAT_OP_MATRIX column `op_24`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
