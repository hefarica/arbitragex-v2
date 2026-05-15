//! Z-EPBS — invariantes del ePBS Direct Client.

use epbs_client_rs::{
    encode_hex, EpbsDirectClient, PayloadHeader, RelayError, RelayResponse, StubEpbsRelay,
};
use std::time::Duration;

fn header_demo(bytes: usize) -> PayloadHeader {
    PayloadHeader {
        parent_hash_hex: encode_hex(&[0x11; 32]),
        fee_recipient_hex: encode_hex(&[0x22; 20]),
        gas_limit: 30_000_000,
        timestamp: 1_715_000_000,
        payload_bytes: vec![0xCC; bytes],
    }
}

fn client_with(stub: StubEpbsRelay, ms: u64) -> EpbsDirectClient<StubEpbsRelay> {
    EpbsDirectClient::new(stub, Duration::from_millis(ms)).expect("timeout > 0")
}

#[tokio::test]
async fn z_epbs_01_submit_ok_default() {
    let stub = StubEpbsRelay::new();
    let cli = client_with(stub, 1000);
    let r = cli.submit(42, header_demo(64)).await.unwrap();
    assert_eq!(r.slot, 42);
    assert!(r.accepted);
    assert!(r.block_hash_hex.starts_with("0x"));
}

#[tokio::test]
async fn z_epbs_02_slot_zero_rechazado() {
    let stub = StubEpbsRelay::new();
    let cli = client_with(stub, 1000);
    let err = cli.submit(0, header_demo(64)).await.unwrap_err();
    assert_eq!(err, RelayError::InvalidSlot(0));
}

#[tokio::test]
async fn z_epbs_03_slot_max_rechazado() {
    let stub = StubEpbsRelay::new();
    let cli = client_with(stub, 1000);
    let bad = u64::MAX / 2;
    let err = cli.submit(bad, header_demo(64)).await.unwrap_err();
    assert!(matches!(err, RelayError::InvalidSlot(_)));
}

#[tokio::test]
async fn z_epbs_04_payload_vacio_rechazado() {
    let stub = StubEpbsRelay::new();
    let cli = client_with(stub, 1000);
    let err = cli.submit(1, header_demo(0)).await.unwrap_err();
    assert_eq!(err, RelayError::PayloadEmpty);
}

#[tokio::test]
async fn z_epbs_05_payload_demasiado_grande_rechazado() {
    let stub = StubEpbsRelay::new();
    let cli = client_with(stub, 1000);
    let err = cli.submit(1, header_demo(9 * 1024 * 1024)).await.unwrap_err();
    assert!(matches!(err, RelayError::PayloadTooLarge(_)));
}

#[tokio::test]
async fn z_epbs_06_timeout_se_dispara() {
    let stub = StubEpbsRelay::new();
    stub.set_delay(Duration::from_millis(200)).await;
    let cli = client_with(stub, 30);
    let err = cli.submit(1, header_demo(8)).await.unwrap_err();
    assert!(matches!(err, RelayError::Timeout(_)));
}

#[tokio::test]
async fn z_epbs_07_http_error_se_propaga() {
    let stub = StubEpbsRelay::new();
    stub.set_next(Err(RelayError::Http {
        status: 502,
        body: "bad gateway".into(),
    }))
    .await;
    let cli = client_with(stub, 500);
    let err = cli.submit(7, header_demo(16)).await.unwrap_err();
    match err {
        RelayError::Http { status, .. } => assert_eq!(status, 502),
        _ => panic!("se esperaba Http"),
    }
}

#[tokio::test]
async fn z_epbs_08_slot_mismatch_es_invalid_response() {
    let stub = StubEpbsRelay::new();
    stub.set_next(Ok(RelayResponse {
        slot: 999,
        block_hash_hex: encode_hex(&[0xAA; 32]),
        builder_signature_hex: encode_hex(&[0xBB; 96]),
        accepted: true,
    }))
    .await;
    let cli = client_with(stub, 500);
    let err = cli.submit(7, header_demo(16)).await.unwrap_err();
    assert!(matches!(err, RelayError::InvalidResponse(_)));
}

#[tokio::test]
async fn z_epbs_09_block_hash_no_hex_es_invalid_response() {
    let stub = StubEpbsRelay::new();
    stub.set_next(Ok(RelayResponse {
        slot: 7,
        block_hash_hex: "not-hex".into(),
        builder_signature_hex: encode_hex(&[0xBB; 96]),
        accepted: true,
    }))
    .await;
    let cli = client_with(stub, 500);
    let err = cli.submit(7, header_demo(16)).await.unwrap_err();
    assert!(matches!(err, RelayError::InvalidResponse(_)));
}
