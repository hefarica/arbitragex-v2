//! Approximate CPMM batch quotes for pre-simulation ranking.
//!
//! These floating-point values are NOT executable quotes or net profit. A
//! consumer must evaluate shortlisted candidates with the existing integer
//! quote and simulation paths before sizing, gating, or submitting a trade.
//! The exact `amount_buckets` sweep deliberately does not call this module.
//!
//! Contract:
//! - equal-length input columns, including empty columns;
//! - finite nonnegative amounts, finite positive oriented reserves, and a
//!   finite fee fraction in `[0, 1)` (e.g. `0.003`, not `30` basis points);
//! - all-or-error output: invalid/unrepresentable lanes are never zero-filled;
//! - runtime AVX detection on x86_64, with a portable scalar implementation
//!   and scalar remainder for any batch length;
//! - no allocation, unsafe load, or arithmetic before input validation.
//!
//! The mathematical quote is `reserve_out * effective / (reserve_in +
//! effective)`, where `effective = amount * (1 - fee)`. We evaluate the
//! fraction using `min(effective, reserve_in) / max(effective, reserve_in)`.
//! Both the ratio and the resulting share are at most one, avoiding overflow
//! in the original numerator and denominator. Positive inputs whose output
//! rounds to zero are reported explicitly, not mistaken for zero input.

use std::fmt;

/// Input column responsible for a rejected lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteInput {
    Amount,
    ReserveIn,
    ReserveOut,
    FeeFraction,
}

/// Batch failure. A failed batch has no usable partial output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteBatchError {
    LengthMismatch {
        amounts: usize,
        reserves_in: usize,
        reserves_out: usize,
        fees: usize,
    },
    InvalidValue {
        index: usize,
        field: QuoteInput,
    },
    /// The positive quote cannot be represented, or arithmetic produced an
    /// invalid value. Callers should use the exact quote path for this batch.
    UnrepresentableOutput {
        index: usize,
    },
}

impl fmt::Display for QuoteBatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch {
                amounts,
                reserves_in,
                reserves_out,
                fees,
            } => write!(
                formatter,
                "CPMM batch lengths differ: amounts={}, reserves_in={}, reserves_out={}, fees={}",
                amounts, reserves_in, reserves_out, fees
            ),
            Self::InvalidValue { index, field } => write!(
                formatter,
                "CPMM batch invalid {:?} at index {}",
                field, index
            ),
            Self::UnrepresentableOutput { index } => write!(
                formatter,
                "CPMM batch output is not representable at index {}",
                index
            ),
        }
    }
}

impl std::error::Error for QuoteBatchError {}

/// Quote independent oriented CPMM pools or amount probes approximately.
///
/// The CPU feature check includes OS support for AVX state. No global AVX
/// compiler flag or AVX2 support is required. Invalid inputs are rejected
/// before dispatch; the error semantics match the scalar entry point.
pub fn batch_quote_cpmm_approx(
    amounts: &[f64],
    reserves_in: &[f64],
    reserves_out: &[f64],
    fees: &[f64],
) -> Result<Vec<f64>, QuoteBatchError> {
    validate_inputs(amounts, reserves_in, reserves_out, fees)?;
    let mut outputs = vec![0.0; amounts.len()];

    #[cfg(target_arch = "x86_64")]
    if amounts.len() >= 4 && std::is_x86_feature_detected!("avx") {
        // SAFETY: runtime detection established AVX and OS support. Input
        // validation established identical lengths and numeric domains;
        // outputs was allocated to precisely that length.
        unsafe {
            fill_avx_validated(amounts, reserves_in, reserves_out, fees, &mut outputs);
        }
        validate_outputs(amounts, reserves_out, &mut outputs)?;
        return Ok(outputs);
    }

    fill_scalar_validated(amounts, reserves_in, reserves_out, fees, &mut outputs);
    validate_outputs(amounts, reserves_out, &mut outputs)?;
    Ok(outputs)
}

/// Portable scalar entry point with the same validation and semantics.
/// Useful for independent verification and callers that intentionally avoid
/// SIMD; this is also the automatic fallback on unsupported hardware.
pub fn batch_quote_cpmm_approx_scalar(
    amounts: &[f64],
    reserves_in: &[f64],
    reserves_out: &[f64],
    fees: &[f64],
) -> Result<Vec<f64>, QuoteBatchError> {
    validate_inputs(amounts, reserves_in, reserves_out, fees)?;
    let mut outputs = vec![0.0; amounts.len()];
    fill_scalar_validated(amounts, reserves_in, reserves_out, fees, &mut outputs);
    validate_outputs(amounts, reserves_out, &mut outputs)?;
    Ok(outputs)
}

fn validate_inputs(
    amounts: &[f64],
    reserves_in: &[f64],
    reserves_out: &[f64],
    fees: &[f64],
) -> Result<(), QuoteBatchError> {
    let n = amounts.len();
    if reserves_in.len() != n || reserves_out.len() != n || fees.len() != n {
        return Err(QuoteBatchError::LengthMismatch {
            amounts: n,
            reserves_in: reserves_in.len(),
            reserves_out: reserves_out.len(),
            fees: fees.len(),
        });
    }
    for index in 0..n {
        for (value, field, valid_domain) in [
            (amounts[index], QuoteInput::Amount, amounts[index] >= 0.0),
            (
                reserves_in[index],
                QuoteInput::ReserveIn,
                reserves_in[index] > 0.0,
            ),
            (
                reserves_out[index],
                QuoteInput::ReserveOut,
                reserves_out[index] > 0.0,
            ),
            (
                fees[index],
                QuoteInput::FeeFraction,
                (0.0..1.0).contains(&fees[index]),
            ),
        ] {
            if !value.is_finite() || !valid_domain {
                return Err(QuoteBatchError::InvalidValue { index, field });
            }
        }
    }
    Ok(())
}

fn fill_scalar_validated(
    amounts: &[f64],
    reserves_in: &[f64],
    reserves_out: &[f64],
    fees: &[f64],
    outputs: &mut [f64],
) {
    for index in 0..amounts.len() {
        let effective = amounts[index] * (1.0 - fees[index]);
        let reserve_in = reserves_in[index];
        let ratio = effective.min(reserve_in) / effective.max(reserve_in);
        let numerator = if effective <= reserve_in { ratio } else { 1.0 };
        outputs[index] = reserves_out[index] * (numerator / (1.0 + ratio));
    }
}

/// The caller must establish AVX support, equal lengths for all slices, and
/// the input domains checked by `validate_inputs` before entering this fn.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx")]
unsafe fn fill_avx_validated(
    amounts: &[f64],
    reserves_in: &[f64],
    reserves_out: &[f64],
    fees: &[f64],
    outputs: &mut [f64],
) {
    use std::arch::x86_64::{
        _mm256_add_pd, _mm256_blendv_pd, _mm256_cmp_pd, _mm256_div_pd, _mm256_loadu_pd,
        _mm256_max_pd, _mm256_min_pd, _mm256_mul_pd, _mm256_set1_pd, _mm256_storeu_pd,
        _mm256_sub_pd, _CMP_LE_OQ,
    };

    let one = _mm256_set1_pd(1.0);
    // Multiplication cannot overflow: truncating the quotient removes at
    // most three lanes. Never load four values from an incomplete block.
    let vector_len = amounts.len() / 4 * 4;
    for index in (0..vector_len).step_by(4) {
        // SAFETY: index + 4 <= vector_len <= every slice length. Unaligned
        // intrinsics permit Vec/slice alignment without additional claims.
        unsafe {
            let amount = _mm256_loadu_pd(amounts.as_ptr().add(index));
            let reserve_in = _mm256_loadu_pd(reserves_in.as_ptr().add(index));
            let reserve_out = _mm256_loadu_pd(reserves_out.as_ptr().add(index));
            let fee = _mm256_loadu_pd(fees.as_ptr().add(index));
            let effective = _mm256_mul_pd(amount, _mm256_sub_pd(one, fee));
            let ratio = _mm256_div_pd(
                _mm256_min_pd(effective, reserve_in),
                _mm256_max_pd(effective, reserve_in),
            );
            let smaller_amount = _mm256_cmp_pd::<_CMP_LE_OQ>(effective, reserve_in);
            let numerator = _mm256_blendv_pd(one, ratio, smaller_amount);
            let share = _mm256_div_pd(numerator, _mm256_add_pd(one, ratio));
            _mm256_storeu_pd(
                outputs.as_mut_ptr().add(index),
                _mm256_mul_pd(reserve_out, share),
            );
        }
    }
    fill_scalar_validated(
        &amounts[vector_len..],
        &reserves_in[vector_len..],
        &reserves_out[vector_len..],
        &fees[vector_len..],
        &mut outputs[vector_len..],
    );
}

fn validate_outputs(
    amounts: &[f64],
    reserves_out: &[f64],
    outputs: &mut [f64],
) -> Result<(), QuoteBatchError> {
    for (index, output) in outputs.iter_mut().enumerate() {
        if !output.is_finite()
            || *output < 0.0
            || *output > reserves_out[index]
            || (amounts[index] > 0.0 && *output == 0.0)
        {
            return Err(QuoteBatchError::UnrepresentableOutput { index });
        }
        if amounts[index] == 0.0 {
            // Canonicalize -0.0 input/output consistently on both backends.
            *output = 0.0;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    type BatchFn = fn(&[f64], &[f64], &[f64], &[f64]) -> Result<Vec<f64>, QuoteBatchError>;

    const BACKENDS: [BatchFn; 2] = [batch_quote_cpmm_approx, batch_quote_cpmm_approx_scalar];

    fn assert_close(actual: f64, expected: f64) {
        let tolerance = expected.abs().max(1.0) * 2e-13;
        assert!(
            (actual - expected).abs() <= tolerance,
            "actual {}, expected {}, tolerance {}",
            actual,
            expected,
            tolerance
        );
    }

    #[test]
    fn every_tail_length_zero_through_nine_is_processed() {
        for n in 0..=9 {
            let amounts: Vec<_> = (0..n).map(|i| i as f64 * 171.0).collect();
            let reserves_in: Vec<_> = (0..n).map(|i| 111.0 + i as f64).collect();
            let reserves_out: Vec<_> = (0..n).map(|i| 901.0 + i as f64).collect();
            let fees: Vec<_> = (0..n).map(|i| i as f64 / 100.0).collect();
            let reference =
                batch_quote_cpmm_approx_scalar(&amounts, &reserves_in, &reserves_out, &fees)
                    .unwrap();
            let actual =
                batch_quote_cpmm_approx(&amounts, &reserves_in, &reserves_out, &fees).unwrap();
            assert_eq!(actual.len(), n);
            assert_eq!(actual, reference);
            for i in 0..n {
                let effective = amounts[i] * (1.0 - fees[i]);
                assert_close(
                    actual[i],
                    reserves_out[i] * effective / (reserves_in[i] + effective),
                );
            }
        }
    }

    #[test]
    fn all_length_mismatches_fail_before_value_validation() {
        for backend in BACKENDS {
            for short_column in 0..4 {
                let mut columns = [vec![f64::NAN; 5], vec![1.0; 5], vec![1.0; 5], vec![0.0; 5]];
                columns[short_column].pop();
                assert_eq!(
                    backend(&columns[0], &columns[1], &columns[2], &columns[3]),
                    Err(QuoteBatchError::LengthMismatch {
                        amounts: columns[0].len(),
                        reserves_in: columns[1].len(),
                        reserves_out: columns[2].len(),
                        fees: columns[3].len(),
                    })
                );
            }
            assert!(matches!(
                backend(&[], &[1.0], &[], &[]),
                Err(QuoteBatchError::LengthMismatch { .. })
            ));
        }
    }

    #[test]
    fn invalid_values_are_rejected_in_vector_and_tail_positions() {
        let fields = [
            QuoteInput::Amount,
            QuoteInput::ReserveIn,
            QuoteInput::ReserveOut,
            QuoteInput::FeeFraction,
        ];
        for backend in BACKENDS {
            for (column, field) in fields.iter().copied().enumerate() {
                let mut invalid_values = vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0];
                if column == 1 || column == 2 {
                    invalid_values.extend([0.0, -0.0]);
                }
                if column == 3 {
                    invalid_values.extend([1.0, 1.1]);
                }
                for invalid_value in invalid_values {
                    for index in [0, 3, 4, 8] {
                        let mut columns = [
                            vec![10.0; 9],
                            vec![100.0; 9],
                            vec![200.0; 9],
                            vec![0.003; 9],
                        ];
                        columns[column][index] = invalid_value;
                        assert_eq!(
                            backend(&columns[0], &columns[1], &columns[2], &columns[3]),
                            Err(QuoteBatchError::InvalidValue { index, field })
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn zero_amounts_are_real_zero_quotes_and_fee_endpoints_are_explicit() {
        for backend in BACKENDS {
            let result = backend(
                &[0.0, -0.0, 10.0, 10.0, -0.0],
                &[100.0; 5],
                &[200.0; 5],
                &[0.0, 0.003, 0.0, 1.0 - f64::EPSILON, -0.0],
            )
            .unwrap();
            for i in [0, 1, 4] {
                assert_eq!(result[i].to_bits(), 0.0f64.to_bits());
            }
            assert_close(result[2], 2000.0 / 110.0);
            assert!(result[3] > 0.0);
        }
    }

    #[test]
    fn scaled_formula_avoids_numerator_and_denominator_overflow() {
        for backend in BACKENDS {
            let result = backend(
                &[f64::MAX, f64::MAX, 1.0, f64::MAX, f64::MAX],
                &[f64::MAX, f64::MIN_POSITIVE, f64::MAX, f64::MAX, f64::MAX],
                &[f64::MAX, 77.0, f64::MAX, f64::MAX, f64::MAX],
                &[0.0, 0.0, 0.0, 0.5, 0.0],
            )
            .unwrap();
            assert_eq!(result[0], f64::MAX / 2.0);
            assert_eq!(result[1], 77.0);
            assert_close(result[2], 1.0);
            assert_close(result[3], f64::MAX / 3.0);
            assert_eq!(result[4], f64::MAX / 2.0);
        }
    }

    #[test]
    fn positive_underflow_returns_an_error_instead_of_zero() {
        for backend in BACKENDS {
            for index in [0, 3, 4, 8] {
                let mut amounts = vec![1.0; 9];
                amounts[index] = f64::from_bits(1);
                assert_eq!(
                    backend(&amounts, &[1.0; 9], &[1.0; 9], &[0.9; 9]),
                    Err(QuoteBatchError::UnrepresentableOutput { index })
                );
            }
        }
    }

    #[test]
    fn quotes_match_an_independent_integer_rational_reference() {
        let mut amounts = Vec::new();
        let mut reserves_in = Vec::new();
        let mut reserves_out = Vec::new();
        let mut fees = Vec::new();
        let mut expected = Vec::new();
        for amount in 0..=128u128 {
            for reserve_in in [101u128, 1024, 10_000] {
                for reserve_out in [113u128, 2048, 13_000] {
                    for fee_bps in [0u128, 1, 30, 100, 9999] {
                        let effective_numerator = amount * (10_000 - fee_bps);
                        let numerator = effective_numerator * reserve_out;
                        let denominator = reserve_in * 10_000 + effective_numerator;
                        amounts.push(amount as f64);
                        reserves_in.push(reserve_in as f64);
                        reserves_out.push(reserve_out as f64);
                        fees.push(fee_bps as f64 / 10_000.0);
                        expected.push(numerator as f64 / denominator as f64);
                    }
                }
            }
        }
        for backend in BACKENDS {
            let result = backend(&amounts, &reserves_in, &reserves_out, &fees).unwrap();
            for (actual, expected) in result.into_iter().zip(expected.iter().copied()) {
                assert_close(actual, expected);
            }
        }
    }

    #[test]
    fn quotes_are_bounded_monotone_and_scale_with_oriented_reserves() {
        for scale in [1e-80, 1e-20, 1.0, 1e20, 1e80] {
            let amounts: Vec<_> = (0..129).map(|i| i as f64 * scale).collect();
            let reserves_in = vec![101.0 * scale; amounts.len()];
            let reserves_out = vec![203.0 * scale; amounts.len()];
            let fees = vec![0.003; amounts.len()];
            let result =
                batch_quote_cpmm_approx(&amounts, &reserves_in, &reserves_out, &fees).unwrap();
            for (i, output) in result.iter().copied().enumerate() {
                assert!(output >= 0.0 && output <= reserves_out[i]);
                if i > 0 {
                    assert!(output > result[i - 1]);
                }
                let effective = i as f64 * 0.997;
                assert_close(output / scale, 203.0 * effective / (101.0 + effective));
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx_kernel_matches_scalar_when_hardware_supports_it() {
        if !std::is_x86_feature_detected!("avx") {
            return;
        }
        for n in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 4099] {
            let amounts: Vec<_> = (0..n).map(|i| (i % 71) as f64 * 713.0).collect();
            let reserves_in: Vec<_> = (0..n).map(|i| (i % 29 + 1) as f64 * 111.0).collect();
            let reserves_out: Vec<_> = (0..n).map(|i| (i % 37 + 1) as f64 * 317.0).collect();
            let fees: Vec<_> = (0..n).map(|i| (i % 13) as f64 / 100.0).collect();
            let reference =
                batch_quote_cpmm_approx_scalar(&amounts, &reserves_in, &reserves_out, &fees)
                    .unwrap();
            let mut actual = vec![0.0; n];
            validate_inputs(&amounts, &reserves_in, &reserves_out, &fees).unwrap();
            // SAFETY: feature detection and validated, identical lengths.
            unsafe {
                fill_avx_validated(&amounts, &reserves_in, &reserves_out, &fees, &mut actual);
            }
            validate_outputs(&amounts, &reserves_out, &mut actual).unwrap();
            assert_eq!(actual, reference);
        }
    }
}
