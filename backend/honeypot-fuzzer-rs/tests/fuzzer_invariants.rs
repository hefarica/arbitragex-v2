//! Z-FUZZ — invariantes del Dynamic Honeypot Fuzzer.

use honeypot_fuzzer_rs::{
    DynamicHoneypotFuzzer, FuzzVerdict, FuzzerError, StubEvmSandbox, TokenAddress,
    DEFAULT_MAX_TAX_BPS,
};
use std::time::Duration;

fn token(first: u8, second: u8) -> TokenAddress {
    let mut b = [0u8; 20];
    b[0] = first;
    b[1] = second;
    TokenAddress::new(b)
}

fn fuzzer(probe: u128, max_bps: u128, ms: u64) -> DynamicHoneypotFuzzer<StubEvmSandbox> {
    DynamicHoneypotFuzzer::new(
        StubEvmSandbox::default(),
        probe,
        max_bps,
        Duration::from_millis(ms),
    )
    .expect("config válida en test")
}

#[tokio::test]
async fn z_fuzz_01_token_limpio_clean_zero_tax() {
    let f = fuzzer(1_000_000, DEFAULT_MAX_TAX_BPS, 500);
    let v = f.vivisect(token(0x01, 0x00)).await.unwrap();
    assert_eq!(v, FuzzVerdict::Clean { tax_bps: 0 });
}

#[tokio::test]
async fn z_fuzz_02_buy_bloqueado_honeypot() {
    let f = fuzzer(1_000_000, DEFAULT_MAX_TAX_BPS, 500);
    let v = f.vivisect(token(0xBB, 0x00)).await.unwrap();
    match v {
        FuzzVerdict::Honeypot { reason } => assert!(reason.contains("buy")),
        _ => panic!("se esperaba Honeypot"),
    }
}

#[tokio::test]
async fn z_fuzz_03_sell_bloqueado_honeypot() {
    let f = fuzzer(1_000_000, DEFAULT_MAX_TAX_BPS, 500);
    let v = f.vivisect(token(0xDE, 0x00)).await.unwrap();
    match v {
        FuzzVerdict::Honeypot { reason } => assert!(reason.contains("sell")),
        _ => panic!("se esperaba Honeypot"),
    }
}

#[tokio::test]
async fn z_fuzz_04_tax_dentro_de_limite_clean() {
    let f = fuzzer(1_000_000, DEFAULT_MAX_TAX_BPS, 500);
    // tax byte = 5 → 50 bps por lado → ~100 bps round trip (~1%)
    let v = f.vivisect(token(0xAA, 5)).await.unwrap();
    match v {
        FuzzVerdict::Clean { tax_bps } => assert!(tax_bps > 0 && tax_bps <= DEFAULT_MAX_TAX_BPS),
        _ => panic!("se esperaba Clean con tax > 0"),
    }
}

#[tokio::test]
async fn z_fuzz_05_tax_destructivo_rechazado() {
    let f = fuzzer(1_000_000, DEFAULT_MAX_TAX_BPS, 500);
    // tax byte = 100 → 1000 bps por lado → ~1900 bps round trip > 1000 max
    let err = f.vivisect(token(0xAA, 100)).await.unwrap_err();
    match err {
        FuzzerError::DestructiveTax { tax_bps, max_bps } => {
            assert!(tax_bps > max_bps);
            assert_eq!(max_bps, DEFAULT_MAX_TAX_BPS);
        }
        _ => panic!("se esperaba DestructiveTax"),
    }
}

#[tokio::test]
async fn z_fuzz_06_probe_zero_rechazado_en_constructor() {
    match DynamicHoneypotFuzzer::new(
        StubEvmSandbox::default(),
        0,
        DEFAULT_MAX_TAX_BPS,
        Duration::from_millis(100),
    ) {
        Ok(_) => panic!("se esperaba InvalidConfig"),
        Err(e) => assert!(matches!(e, FuzzerError::InvalidConfig(_))),
    }
}

#[tokio::test]
async fn z_fuzz_07_max_tax_invalido_rechazado() {
    match DynamicHoneypotFuzzer::new(
        StubEvmSandbox::default(),
        100,
        10_000,
        Duration::from_millis(100),
    ) {
        Ok(_) => panic!("se esperaba InvalidConfig"),
        Err(e) => assert!(matches!(e, FuzzerError::InvalidConfig(_))),
    }
}

#[tokio::test]
async fn z_fuzz_08_timeout_se_dispara() {
    let stub = StubEvmSandbox::default();
    stub.set_delay(Duration::from_millis(200)).await;
    let f = DynamicHoneypotFuzzer::new(
        stub,
        1_000_000,
        DEFAULT_MAX_TAX_BPS,
        Duration::from_millis(30),
    )
    .unwrap();
    let err = f.vivisect(token(0x01, 0x00)).await.unwrap_err();
    assert!(matches!(err, FuzzerError::Timeout(_)));
}

#[tokio::test]
async fn z_fuzz_09_determinismo_misma_entrada_mismo_veredicto() {
    let f = fuzzer(1_000_000, DEFAULT_MAX_TAX_BPS, 500);
    let a = f.vivisect(token(0xAA, 5)).await.unwrap();
    let b = f.vivisect(token(0xAA, 5)).await.unwrap();
    assert_eq!(a, b);
}
