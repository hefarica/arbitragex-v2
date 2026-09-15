//! Transport laboratory only: all listeners bind ephemeral loopback ports.
//! These are not observed opportunities and never contact a chain or signer.
use axum::{extract::State, routing::post, Json, Router};
use ethers::types::{Address, H256};
use serde_json::{json, Value};
use shared_rs::{oracle_snapshot::OracleRpc, rpc_failover::parse_csv};
use std::{sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle};

#[derive(Clone)]
struct Lab {
    reply: Arc<Mutex<Value>>,
    calls: Arc<Mutex<Vec<Value>>>,
    delay: Duration,
}
struct Server {
    url: String,
    lab: Lab,
    task: JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn serve(State(lab): State<Lab>, Json(request): Json<Value>) -> Json<Value> {
    lab.calls.lock().await.push(request.clone());
    tokio::time::sleep(lab.delay).await;
    if request["method"] == "eth_chainId" {
        return Json(lab.reply.lock().await.clone());
    }
    Json(json!({"jsonrpc":"2.0","id":request["id"],"result":"0x07"}))
}
async fn server(reply: Value, delay: Duration) -> Server {
    let lab = Lab {
        reply: Arc::new(Mutex::new(reply)),
        calls: Arc::new(Mutex::new(vec![])),
        delay,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/rpc", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/rpc", post(serve))
        .with_state(lab.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Server { url, lab, task }
}
fn reply(chain: u64) -> Value {
    json!({"jsonrpc":"2.0","id":1,"result":format!("0x{chain:x}")})
}
fn error(result: anyhow::Result<OracleRpc>) -> String {
    match result {
        Ok(_) => panic!("unexpected verified provider"),
        Err(e) => format!("{e:#}"),
    }
}

#[test]
fn bare_query_equals_is_not_a_provider_label() {
    let url = "https://rpc.invalid/v1/key=value?api_key=secret&option=a=b";
    let values = parse_csv(url).unwrap();
    assert_eq!(values, vec![("primary".to_string(), url.to_string())]);
}
#[test]
fn named_query_equals_and_mixed_csv_are_preserved() {
    let urls = parse_csv("first=https://one.invalid/?key=a=b,https://two.invalid/?key=c").unwrap();
    assert_eq!(urls[0].1, "https://one.invalid/?key=a=b");
    assert_eq!(urls[1].0, "primary");
    assert_eq!(urls[1].1, "https://two.invalid/?key=c");
}
#[tokio::test]
async fn nine_named_providers_reach_the_last_valid_chain() {
    let mut servers = Vec::new();
    for _ in 0..8 {
        servers.push(server(reply(10), Duration::ZERO).await);
    }
    servers.push(server(reply(1), Duration::ZERO).await);
    let csv = servers
        .iter()
        .enumerate()
        .map(|(i, s)| format!("vendor{i}={}", s.url))
        .collect::<Vec<_>>()
        .join(",");
    let rpc = OracleRpc::from_config(1, &csv).await.unwrap();
    assert_eq!(rpc.endpoint(), servers[8].url);
    for s in &servers[..8] {
        assert!(s
            .lab
            .calls
            .lock()
            .await
            .iter()
            .all(|r| r["method"] == "eth_chainId"));
    }
}
#[tokio::test]
async fn public_chain_ids_are_not_silently_rewritten() {
    for chain in [1, 10, 137, 8453, 42161, 11155111, 9007199254740993] {
        let s = server(reply(chain), Duration::ZERO).await;
        assert!(OracleRpc::from_config(chain, &s.url).await.is_ok());
    }
}
#[tokio::test]
async fn full_snapshot_parameters_use_the_selected_provider() {
    let wrong = server(reply(10), Duration::ZERO).await;
    let good = server(reply(1), Duration::ZERO).await;
    let rpc = OracleRpc::from_config(1, &format!("bad={},good={}", wrong.url, good.url))
        .await
        .unwrap();
    let block = H256::from_low_u64_be(42);
    assert_eq!(
        rpc.read(Address::from_low_u64_be(1), "0x313ce567", block)
            .await
            .unwrap(),
        vec![7]
    );
    assert_eq!(rpc.endpoint(), good.url);
    let calls = good.lab.calls.lock().await;
    assert_eq!(
        calls.last().unwrap()["params"][1],
        json!({"blockHash":format!("{block:#x}"),"requireCanonical":true})
    );
    assert_eq!(calls.last().unwrap()["params"][0]["data"], "0x313ce567");
    assert_eq!(wrong.lab.calls.lock().await.len(), 1);
}
#[tokio::test]
async fn slow_primary_does_not_prevent_a_healthy_secondary() {
    let slow = server(reply(1), Duration::from_secs(10)).await;
    let good = server(reply(1), Duration::ZERO).await;
    let rpc = OracleRpc::from_config(1, &format!("first={},second={}", slow.url, good.url))
        .await
        .unwrap();
    assert_eq!(rpc.endpoint(), good.url);
}
#[tokio::test]
async fn no_usable_http_configuration_never_falls_back_to_a_public_url() {
    for raw in [
        "",
        ",,",
        "ftp://rpc.invalid",
        "ws=wss://rpc.invalid",
        "malformed=",
        "https://rpc.invalid/#fragment",
    ] {
        assert_eq!(
            error(OracleRpc::from_config(1, raw).await),
            "oracle_rpc_no_http_provider"
        );
    }
}
#[tokio::test]
async fn duplicate_urls_are_probed_once_even_under_distinct_names() {
    let s = server(reply(10), Duration::ZERO).await;
    assert_eq!(
        error(OracleRpc::from_config(1, &format!("one={},two={}", s.url, s.url)).await),
        "oracle_rpc_no_verified_provider"
    );
    assert_eq!(s.lab.calls.lock().await.len(), 1);
}
#[tokio::test]
async fn wrong_chain_and_rpc_errors_do_not_leak_response_data() {
    let s = server(
        json!({"jsonrpc":"2.0","id":1,"error":{"message":"SECRET_KEY_MUST_NOT_ESCAPE"}}),
        Duration::ZERO,
    )
    .await;
    let message =
        error(OracleRpc::from_config(1, &format!("private={}?token=secret", s.url)).await);
    assert_eq!(message, "oracle_rpc_no_verified_provider");
    assert!(!message.contains("secret") && !message.contains("SECRET"));
}
#[tokio::test]
async fn malformed_chain_quantities_never_pass() {
    for result in [
        json!(1),
        json!(null),
        json!("1"),
        json!("0x"),
        json!("0x01"),
        json!("0xzz"),
        json!("0x10000000000000000"),
    ] {
        let s = server(
            json!({"jsonrpc":"2.0","id":1,"result":result}),
            Duration::ZERO,
        )
        .await;
        assert_eq!(
            error(OracleRpc::from_config(1, &s.url).await),
            "oracle_rpc_no_verified_provider"
        );
    }
}
#[tokio::test]
async fn mismatched_response_id_and_protocol_do_not_pass() {
    for result in [
        json!({"jsonrpc":"2.0","id":2,"result":"0x1"}),
        json!({"id":1,"result":"0x1"}),
        json!({"jsonrpc":"1.0","id":1,"result":"0x1"}),
    ] {
        let s = server(result, Duration::ZERO).await;
        assert_eq!(
            error(OracleRpc::from_config(1, &s.url).await),
            "oracle_rpc_no_verified_provider"
        );
    }
}
#[tokio::test]
async fn failed_selection_recovers_on_the_next_attempt() {
    let s = server(reply(10), Duration::ZERO).await;
    assert!(OracleRpc::from_config(1, &s.url).await.is_err());
    *s.lab.reply.lock().await = reply(1);
    assert!(OracleRpc::from_config(1, &s.url).await.is_ok());
    assert_eq!(s.lab.calls.lock().await.len(), 2);
}
#[tokio::test]
async fn configuration_and_chain_limits_are_enforced_before_network_io() {
    assert_eq!(
        error(OracleRpc::from_config(0, "https://rpc.invalid").await),
        "oracle_rpc_chain_invalid"
    );
    assert_eq!(
        error(OracleRpc::from_config(1, &"x".repeat(65537)).await),
        "oracle_rpc_config_too_large"
    );
    let csv = (0..33)
        .map(|i| format!("https://vendor{i}.invalid"))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        error(OracleRpc::from_config(1, &csv).await),
        "oracle_rpc_provider_limit"
    );
}
