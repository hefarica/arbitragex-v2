# Flash Loan (op_26)

## Identity
- **ID**: 26 / op_26
- **Canonical Role**: Flash-liquidity feasibility/cost.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
x_{t+1} = x_t + tau_flash - flash-liquidity feasibility/costing
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_26_flash_loan.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 29
- Matrix: 13_STRAT_OP_MATRIX column `op_26`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
