//! Z-LOCK — 8 invariantes del Atomic Cross-Rollup State Locker.

use state_lock_rs::{
    LockError, LockResponse, LockStatus, LockTicket, MockLockClient, SharedSequencerLocker,
};
use std::time::Duration;

fn locker_with(mock: MockLockClient, ms: u64) -> SharedSequencerLocker<MockLockClient> {
    SharedSequencerLocker::new(mock, Duration::from_millis(ms))
        .expect("timeout > 0 en construcción de test")
}

#[tokio::test]
async fn z_lock_01_acquire_default_ok() {
    let mock = MockLockClient::new();
    let locker = locker_with(mock, 500);
    let r = locker.lock_slot(42, "arbitrum-one").await.unwrap();
    assert_eq!(r.ticket.slot, 42);
    assert_eq!(r.status, LockStatus::Acquired);
}

#[tokio::test]
async fn z_lock_02_slot_zero_rechazado() {
    let mock = MockLockClient::new();
    let locker = locker_with(mock, 500);
    let err = locker.lock_slot(0, "arb").await.unwrap_err();
    assert!(matches!(err, LockError::InvalidSlot(_)));
}

#[tokio::test]
async fn z_lock_03_auction_lost_se_propaga() {
    let mock = MockLockClient::new();
    mock.set_acquire(Ok(LockResponse {
        ticket: LockTicket {
            slot: 7,
            rollup_id: "x".into(),
            nonce: 0,
        },
        status: LockStatus::Lost,
        backend_signature: "s".into(),
    }))
    .await;
    let locker = locker_with(mock, 500);
    let err = locker.lock_slot(7, "x").await.unwrap_err();
    assert_eq!(err, LockError::AuctionLost);
}

#[tokio::test]
async fn z_lock_04_desync_se_propaga() {
    let mock = MockLockClient::new();
    mock.set_acquire(Err(LockError::SequencerDesync)).await;
    let locker = locker_with(mock, 500);
    let err = locker.lock_slot(1, "x").await.unwrap_err();
    assert_eq!(err, LockError::SequencerDesync);
}

#[tokio::test]
async fn z_lock_05_timeout_zero_rechazado_en_constructor() {
    let mock = MockLockClient::new();
    match SharedSequencerLocker::new(mock, Duration::from_millis(0)) {
        Ok(_) => panic!("se esperaba error"),
        Err(e) => assert!(matches!(e, LockError::NetworkError(_))),
    }
}

#[tokio::test]
async fn z_lock_06_unlock_propaga_remote_error() {
    let mock = MockLockClient::new();
    mock.set_release(Err(LockError::RemoteError {
        status: 503,
        body: "unavailable".into(),
    }))
    .await;
    let locker = locker_with(mock, 500);
    let t = LockTicket {
        slot: 1,
        rollup_id: "r".into(),
        nonce: 0,
    };
    let err = locker.unlock(&t).await.unwrap_err();
    match err {
        LockError::RemoteError { status, .. } => assert_eq!(status, 503),
        _ => panic!("se esperaba RemoteError"),
    }
}

#[tokio::test]
async fn z_lock_07_status_check_pending() {
    let mock = MockLockClient::new();
    mock.set_status(Ok(LockStatus::Pending)).await;
    let locker = locker_with(mock, 500);
    let t = LockTicket {
        slot: 1,
        rollup_id: "r".into(),
        nonce: 0,
    };
    assert_eq!(locker.status(&t).await.unwrap(), LockStatus::Pending);
}

#[tokio::test]
async fn z_lock_08_rollup_vacio_rechazado_por_backend() {
    let mock = MockLockClient::new();
    let locker = locker_with(mock, 500);
    let err = locker.lock_slot(5, "").await.unwrap_err();
    assert!(matches!(err, LockError::InvalidSlot(_)));
}
