# PDMP / jump process (op_05)

## Identity
- **ID**: 5 / op_05
- **Canonical Role**: Jump/event regime evidence.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
dX = mu*dt + sigma*dW + dJ - jump-diffusion regime detection
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_05_pdmp.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 8
- Matrix: 13_STRAT_OP_MATRIX column `op_05`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
