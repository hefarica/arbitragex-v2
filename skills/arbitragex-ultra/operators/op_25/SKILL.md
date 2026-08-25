# Bundle Recon (op_25)

## Identity
- **ID**: 25 / op_25
- **Canonical Role**: Bundle/state reconstruction.
- **Enabled**: YES | **Engine**: YES
- **Calibration**: UNCALIBRATED | **Weight**: 1.0%

## Mathematical Definition
```
B = {Sum lambda_i*x_i | lambda_i >= 0} - bundle reconstruction
```

## Pipeline Phase
Determined by canonical role: DISCOVER, SIZE, RISK, RANK, or VALIDATE.

## Implementation
- File: `backend/math-engine/src/operators/op_25_bundle_recon.rs`
- Tests: `backend/math-engine/src/operators/real_ops_tests.rs`

## Excel Traceability
- Source: ULTRA 12_OPERATOR_CONTROL row 28
- Matrix: 13_STRAT_OP_MATRIX column `op_25`

## Calibration
Currently `UNCALIBRATED` - the IV motor needs Y-labels before
this operator's output becomes signal rather than telemetry.
