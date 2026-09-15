//! Disposable Redis integration for the EXACT production Rust merge/retry and Lua.
//! No production URL/config is read. Explicit opt-in + fixed loopback port only.
use crate::v3_fee::{publish_v3_index, V3FeePips, INDEX_COMPARE_AND_SET};
use std::{
    future::{ready, Future},
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{Arc, Barrier},
    task::{Context, Poll, Waker},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn command(args: &[&str]) -> Result<Option<String>, &'static str> {
    if std::env::var("ARBX_V3_FEE_DISPOSABLE_REDIS").as_deref() != Ok("1") {
        return Err("disposable_redis_opt_in_required");
    }
    let target = SocketAddr::from(([127, 0, 0, 1], 36379));
    let mut stream = TcpStream::connect_timeout(&target, Duration::from_secs(3))
        .map_err(|_| "fixture_connect_failed")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| "fixture_timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| "fixture_timeout")?;
    write!(stream, "*{}\r\n", args.len()).map_err(|_| "fixture_write")?;
    for part in args {
        write!(stream, "${}\r\n{}\r\n", part.len(), part).map_err(|_| "fixture_write")?;
    }
    let mut reader = BufReader::new(stream);
    let mut header = String::new();
    reader.read_line(&mut header).map_err(|_| "fixture_read")?;
    if !header.ends_with("\r\n") {
        return Err("fixture_header_invalid");
    }
    let value = &header[1..header.len() - 2];
    match header.as_bytes()[0] {
        b'+' | b':' => Ok(Some(value.to_string())),
        b'$' => {
            let n: i64 = value.parse().map_err(|_| "fixture_length_invalid")?;
            if n == -1 {
                return Ok(None);
            }
            if !(0..=1_048_576).contains(&n) {
                return Err("fixture_unbounded_response");
            }
            let mut bytes = vec![0u8; n as usize + 2];
            reader.read_exact(&mut bytes).map_err(|_| "fixture_body")?;
            if !bytes.ends_with(b"\r\n") {
                return Err("fixture_body_invalid");
            }
            bytes.truncate(n as usize);
            String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| "fixture_encoding")
        }
        _ => Err("fixture_redis_error"),
    }
}

fn immediate<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous test transport cannot suspend"),
    }
}
struct Fixture(String);
impl Fixture {
    fn new() -> Self {
        assert_eq!(command(&["PING"]).unwrap().as_deref(), Some("PONG"));
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(format!(
            "arbx:v3-fee-test:rust:{}:{stamp}",
            std::process::id()
        ))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = command(&["DEL", &self.0]);
    }
}
fn observed(value: u32) -> V3FeePips {
    let mut bytes = [0; 32];
    bytes[28..].copy_from_slice(&value.to_be_bytes());
    V3FeePips::from_abi_word(&bytes).unwrap()
}
fn cas(key: &str, prior: Option<String>, next: String) -> Result<bool, &'static str> {
    match command(&[
        "EVAL",
        INDEX_COMPARE_AND_SET,
        "1",
        key,
        if prior.is_some() { "1" } else { "0" },
        prior.as_deref().unwrap_or(""),
        &next,
    ])?
    .as_deref()
    {
        Some("1") => Ok(true),
        Some("0") => Ok(false),
        _ => Err("fixture_invalid_cas_response"),
    }
}

#[test]
#[ignore = "requires explicit disposable Redis on 127.0.0.1:36379"]
fn v3_review_real_redis_racing_pools_both_survive_production_retry() {
    let fixture = Fixture::new();
    let barrier = Arc::new(Barrier::new(2));
    let joins: Vec<_> = [
        ("0x7995430a85156b2d40d5bb701608788cf84019e3", 3000),
        ("0x26f35b980f3b791ac3f7c09ff152815c0dcb5bf3", 500),
    ]
    .into_iter()
    .map(|(address, fee)| {
        let key = fixture.0.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            let mut first = true;
            immediate(publish_v3_index(
                address,
                observed(fee),
                || {
                    let prior = command(&["GET", &key]);
                    if first {
                        first = false;
                        barrier.wait();
                    }
                    ready(prior)
                },
                |prior, next| ready(cas(&key, prior, next)),
            ))
        })
    })
    .collect();
    let mut attempts: Vec<_> = joins
        .into_iter()
        .map(|j| j.join().unwrap().unwrap())
        .collect();
    attempts.sort_unstable();
    assert_eq!(attempts, vec![1, 2]);
    let rows: Vec<crate::reserves::V3PoolInfo> =
        serde_json::from_str(&command(&["GET", &fixture.0]).unwrap().unwrap()).unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|r| r.fee_bps == 3000));
    assert!(rows.iter().any(|r| r.fee_bps == 500));
}

#[test]
#[ignore = "requires explicit disposable Redis on 127.0.0.1:36379"]
fn v3_review_real_redis_invalid_index_is_preserved_by_production_retry() {
    let fixture = Fixture::new();
    command(&["SET", &fixture.0, "malformed-index"]).unwrap();
    let result = immediate(publish_v3_index(
        "0x7995430a85156b2d40d5bb701608788cf84019e3",
        observed(3000),
        || ready(command(&["GET", &fixture.0])),
        |prior, next| ready(cas(&fixture.0, prior, next)),
    ));
    assert_eq!(result, Err("redis_v3_index_invalid"));
    assert_eq!(
        command(&["GET", &fixture.0]).unwrap().as_deref(),
        Some("malformed-index")
    );
}

#[test]
#[ignore = "requires explicit disposable Redis on 127.0.0.1:36379"]
fn v3_review_real_redis_bootstrap_races_hydration_in_both_commit_orders() {
    use crate::v3_fee::publish_v3_bootstrap;
    use std::sync::{Condvar, Mutex};
    const A: &str = "0x7995430a85156b2d40d5bb701608788cf84019e3";
    const B: &str = "0x26f35b980f3b791ac3f7c09ff152815c0dcb5bf3";
    for bootstrap_wins in [false, true] {
        let fixture = Fixture::new();
        let barrier = Arc::new(Barrier::new(2));
        let done = Arc::new((Mutex::new(false), Condvar::new()));
        let joins: Vec<_> = [false, true]
            .into_iter()
            .map(|is_bootstrap| {
                let key = fixture.0.clone();
                let barrier = barrier.clone();
                let done = done.clone();
                std::thread::spawn(move || {
                    let winner = is_bootstrap == bootstrap_wins;
                    let mut first_read = true;
                    let mut first_cas = true;
                    let read = || {
                        let prior = command(&["GET", &key]);
                        if first_read {
                            first_read = false;
                            barrier.wait();
                        }
                        ready(prior)
                    };
                    let update = |prior, next| {
                        if first_cas && !winner {
                            let (lock, cv) = &*done;
                            let guard = lock.lock().unwrap();
                            let waited = cv
                                .wait_timeout_while(guard, Duration::from_secs(5), |finished| {
                                    !(*finished)
                                })
                                .unwrap();
                            assert!(*waited.0, "winner did not complete before fixture timeout");
                        }
                        let result = cas(&key, prior, next);
                        if first_cas && winner {
                            assert_eq!(result, Ok(true));
                            let (lock, cv) = &*done;
                            *lock.lock().unwrap() = true;
                            cv.notify_all();
                        }
                        first_cas = false;
                        ready(result)
                    };
                    if is_bootstrap {
                        // Simulate a PG snapshot taken before fee hydration (30 vs3000).
                        let rows = vec![
                            crate::reserves::V3PoolInfo {
                                pool_addr: A.into(),
                                fee_bps: 30,
                            },
                            crate::reserves::V3PoolInfo {
                                pool_addr: B.into(),
                                fee_bps: 500,
                            },
                        ];
                        immediate(publish_v3_bootstrap(&rows, read, update))
                    } else {
                        immediate(publish_v3_index(A, observed(3000), read, update))
                    }
                })
            })
            .collect();
        let mut attempts: Vec<_> = joins
            .into_iter()
            .map(|j| j.join().unwrap().unwrap())
            .collect();
        attempts.sort_unstable();
        assert_eq!(attempts, vec![1, 2]);
        let rows: Vec<crate::reserves::V3PoolInfo> =
            serde_json::from_str(&command(&["GET", &fixture.0]).unwrap().unwrap()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter().find(|r| r.pool_addr == A).unwrap().fee_bps,
            3000
        );
        assert_eq!(rows.iter().find(|r| r.pool_addr == B).unwrap().fee_bps, 500);
    }
}
