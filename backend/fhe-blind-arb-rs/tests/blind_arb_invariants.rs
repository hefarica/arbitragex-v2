use fhe_blind_arb_rs::*;
use std::sync::Arc;

#[test]
fn z_fhe_01_backrun_simple() {
    let arb = FheBlindArbitrator::new(Arc::new(StubFheBackend));
    let user = _test_only_plain_to_cipher(100);
    let reserves = _test_only_plain_to_cipher(1000);
    let res = arb.compute_optimal_blind_backrun(&user, &reserves).unwrap();
    assert_eq!(_test_only_decode_stub(&res).unwrap(), 900);
}

#[test]
fn z_fhe_02_underflow_es_error_no_panic() {
    let arb = FheBlindArbitrator::new(Arc::new(StubFheBackend));
    let user = _test_only_plain_to_cipher(2000); // mayor que reserves
    let reserves = _test_only_plain_to_cipher(1000);
    let res = arb.compute_optimal_blind_backrun(&user, &reserves);
    assert_eq!(res.err(), Some(FheError::HomomorphicUnderflow));
}

#[test]
fn z_fhe_03_ciphertext_mal_formado() {
    let arb = FheBlindArbitrator::new(Arc::new(StubFheBackend));
    let bad = CipherU64::from_bytes(vec![1, 2, 3]); // longitud != 8
    let good = _test_only_plain_to_cipher(100);
    let res = arb.compute_optimal_blind_backrun(&bad, &good);
    assert_eq!(res.err(), Some(FheError::MalformedCipher));
}

#[test]
fn z_fhe_04_slippage_aplica_multiplicacion() {
    let arb = FheBlindArbitrator::new(Arc::new(StubFheBackend));
    let user = _test_only_plain_to_cipher(100);
    let reserves = _test_only_plain_to_cipher(1000);
    let res = arb.compute_with_slippage(&user, &reserves, 2).unwrap();
    assert_eq!(_test_only_decode_stub(&res).unwrap(), 1800); // (1000-100)*2
}

#[test]
fn z_fhe_05_slippage_overflow_es_error() {
    let arb = FheBlindArbitrator::new(Arc::new(StubFheBackend));
    let user = _test_only_plain_to_cipher(0);
    let reserves = _test_only_plain_to_cipher(u64::MAX / 2);
    let res = arb.compute_with_slippage(&user, &reserves, 5);
    assert!(matches!(res, Err(FheError::InvalidOperation(_))));
}

#[test]
fn z_fhe_06_cipher_es_opaco() {
    // No hay forma pública de construir CipherU64 con valor conocido
    // excepto via _test_only_*. Validamos que el API no expone .value().
    let c = _test_only_plain_to_cipher(42);
    assert_eq!(c.as_bytes().len(), 8);
}

#[test]
fn z_fhe_07_zero_reserves_zero_backrun() {
    let arb = FheBlindArbitrator::new(Arc::new(StubFheBackend));
    let user = _test_only_plain_to_cipher(0);
    let reserves = _test_only_plain_to_cipher(0);
    let res = arb.compute_optimal_blind_backrun(&user, &reserves).unwrap();
    assert_eq!(_test_only_decode_stub(&res).unwrap(), 0);
}
