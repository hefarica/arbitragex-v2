# JIT Liquidity (op_28)

## Identity
- **ID**: 28 / op_28
- **Canonical Role**: JIT/liquidity-state evidence.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
L_jit = L_target * exp(-k*t) - JIT/liquidity-state evidence
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_28_jit_liquidity.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 31
- Matrix: 13_STRAT_OP_MATRIX column `op_28`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
