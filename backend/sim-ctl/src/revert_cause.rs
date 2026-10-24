//! F5-SIM-FORK-REVERT (2026-10-23): name the cause of an anvil-fork revert.
//!
//! Measured on the deployed stack (2026-10-23T20:01Z, 20 sims/30 min): 17/20
//! anvil simulations died as `rpc_error: execution reverted, data 0x` — the
//! node reported a revert, the engine threw the payload away, and the row kept
//! an anonymous label. This module turns that into a NAMED cause:
//!
//! * the 4-byte selector when the node or the trace exposes revert bytes
//!   (`Error(string)`, `Panic(uint256)` and the custom errors we can name),
//! * the address of the frame that reverted (from `debug_traceCall` with the
//!   geth `callTracer`, which anvil supports — verified live against the VPS
//!   anvil at 172.18.0.9:8545),
//! * `pc`/`opcode` of the failing instruction (from a bounded JS tracer that
//!   records the last executed step; callTracer alone does not expose them),
//! * the revert string / `revertReason` when it exists.
//!
//! Reject semantics are UNCHANGED: this only NAMES a cause that already
//! existed. `fork_revert:*` is an opportunity-quality verdict, NOT a
//! capability gap (see `persistence::is_sim_capability_gap`, which has no
//! catch-all and therefore keeps rejecting it exactly like the old
//! `rpc_error: … reverted` string).

use ethers::providers::{ProviderError, RpcError};
use serde_json::Value;

/// Same budget the two other fail_reason producers use.
pub const MAX_REASON_CHARS: usize = 200;

/// Well-known 4-byte selectors → the name we publish.
///
/// Sources (computed with `cast sig "<signature>"`, 2026-10-23):
/// `Error(string)` and `Panic(uint256)` are the Solidity built-ins;
/// `TRANSFER_FROM_FAILED()` is UniswapV2's `TransferHelper`;
/// the rest are OpenZeppelin / router / executor custom errors already present
/// in this repo's Solidity surface.
pub const KNOWN_SELECTORS: &[(&str, &str)] = &[
    ("0x08c379a0", "error_string"),                    // Error(string)
    ("0x4e487b71", "panic"),                           // Panic(uint256)
    ("0xedf429db", "transfer_from_failed"),            // TransferHelper.TRANSFER_FROM_FAILED()
    ("0xe450d38c", "erc20_insufficient_balance"), // ERC20InsufficientBalance(address,uint256,uint256)
    ("0x118cdaa7", "ownable_unauthorized_account"), // OwnableUnauthorizedAccount(address)
    ("0x3ee5aeb5", "reentrancy_guard_reentrant_call"), // ReentrancyGuardReentrantCall()
    ("0xd28d3eb5", "insufficient_output_amount"), // InsufficientOutputAmount(uint256,uint256)
    ("0xbb55fd27", "insufficient_liquidity"),     // InsufficientLiquidity()
    ("0x5274afe7", "safe_erc20_failed_operation"), // SafeERC20FailedOperation(address)
    ("0x81ceff30", "swap_failed"),                // ArbitrageExecutor.SwapFailed()
    ("0x4e88422a", "insufficient_profit"),        // InsufficientProfit(uint256,uint256)
];

/// A named cause for one fork revert. Every field is OPTIONAL and only set
/// when the node actually exposed it — absence is absence (R8), never a zero.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RevertCause {
    /// Short machine name: `transfer_from_failed`, `panic_0x11`,
    /// `revert_selector_0xdeadbeef`, `revert_no_data`, `error_string`, …
    pub name: String,
    /// 4-byte selector, lowercase hex with `0x` prefix.
    pub selector: Option<String>,
    /// Decoded `Error(string)` payload or the tracer's `revertReason`.
    pub revert_string: Option<String>,
    /// Address of the frame that reverted (`to` of the deepest failing call).
    pub address: Option<String>,
    /// Program counter of the failing instruction (JS tracer).
    pub pc: Option<u64>,
    /// Opcode mnemonic of the failing instruction (usually `REVERT`).
    pub opcode: Option<String>,
}

impl RevertCause {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// `fork_revert:<name>[ selector=0x…][ address=0x…][ pc=<n> op=…][ msg="…"]`
    ///
    /// Order matters: the most identifying facts come first so a 200-char
    /// truncation cannot eat the selector/address.
    pub fn fail_reason(&self) -> String {
        let mut out = format!("fork_revert:{}", self.name);
        if let Some(sel) = &self.selector {
            out.push_str(&format!(" selector={sel}"));
        }
        if let Some(addr) = &self.address {
            out.push_str(&format!(" address={addr}"));
        }
        if let Some(pc) = self.pc {
            out.push_str(&format!(" pc={pc}"));
        }
        if let Some(op) = &self.opcode {
            out.push_str(&format!(" op={op}"));
        }
        if let Some(msg) = &self.revert_string {
            let msg = truncate_chars(msg, 80);
            out.push_str(&format!(" msg=\"{msg}\""));
        }
        truncate_chars(&out, MAX_REASON_CHARS)
    }

    /// Fill only the fields this cause does not have yet (never overwrite a
    /// fact the node already gave us).
    pub fn merge_missing(&mut self, other: &RevertCause) {
        if self.selector.is_none() {
            self.selector = other.selector.clone();
        }
        if self.revert_string.is_none() {
            self.revert_string = other.revert_string.clone();
        }
        if self.address.is_none() {
            self.address = other.address.clone();
        }
        if self.pc.is_none() {
            self.pc = other.pc;
        }
        if self.opcode.is_none() {
            self.opcode = other.opcode.clone();
        }
    }
}

/// Char-boundary-safe truncation (same rule as `sim_engine::truncate_chars`).
pub fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn selector_hex(data: &[u8]) -> Option<String> {
    if data.len() < 4 {
        return None;
    }
    Some(format!("0x{}", hex::encode(&data[..4])))
}

fn known_name(selector: &str) -> Option<&'static str> {
    KNOWN_SELECTORS
        .iter()
        .find(|(sel, _)| *sel == selector)
        .map(|(_, name)| *name)
}

/// Decode `Error(string)` / `Panic(uint256)` / a known custom error from raw
/// revert bytes. Returns `None` when there is nothing to name (empty payload).
pub fn cause_from_revert_data(data: &[u8]) -> Option<RevertCause> {
    let selector = selector_hex(data)?;
    let payload = &data[4..];
    match selector.as_str() {
        // Error(string): offset(32) | length(32) | utf8 bytes
        "0x08c379a0" => {
            let text = decode_abi_string(payload);
            let mut cause = RevertCause::new("error_string");
            cause.selector = Some(selector);
            cause.revert_string = text;
            Some(cause)
        }
        // Panic(uint256): 32-byte code (only the low 8 bytes are ever used;
        // Solidity panic codes are tiny)
        "0x4e487b71" => {
            let code = payload.get(..32).and_then(word_to_u64);
            let mut cause = match code {
                Some(c) => RevertCause::new(format!("panic_0x{c:02x}")),
                None => RevertCause::new("panic"),
            };
            cause.selector = Some(selector);
            cause.revert_string = code.map(|c| format!("panic code {c}"));
            Some(cause)
        }
        other => {
            let name = known_name(other)
                .map(str::to_string)
                .unwrap_or_else(|| format!("revert_selector_{other}"));
            let mut cause = RevertCause::new(name);
            cause.selector = Some(selector);
            Some(cause)
        }
    }
}

/// Low 8 bytes (big-endian) of a 32-byte ABI word — offsets, lengths and
/// Solidity panic codes all fit in a u64.
fn word_to_u64(word: &[u8]) -> Option<u64> {
    let start = word.len().checked_sub(8)?;
    let last8: [u8; 8] = word.get(start..)?.try_into().ok()?;
    Some(u64::from_be_bytes(last8))
}

fn decode_abi_string(payload: &[u8]) -> Option<String> {
    let offset = word_to_u64(payload.get(..32)?)? as usize;
    let start = offset.checked_add(32)?;
    let len_bytes = payload.get(offset..start)?;
    let len = word_to_u64(len_bytes)? as usize;
    let end = start.checked_add(len)?;
    let raw = payload.get(start..end)?;
    Some(
        String::from_utf8_lossy(raw)
            .trim_end_matches('\0')
            .to_string(),
    )
}

/// Revert payload carried by an `eth_call` provider error.
///
/// ethers 2.x wraps JSON-RPC error responses in `ProviderError::JsonRpcClientError`;
/// the `RpcError::as_error_response()` accessor is the supported way to reach
/// `{ code, message, data }`. Some nodes only put the payload in the message as
/// `… data 0x…`, so that shape is covered too.
pub fn revert_data_from_provider_error(e: &ProviderError) -> Option<Vec<u8>> {
    if let Some(err) = e.as_error_response() {
        if let Some(bytes) = revert_data_from_jsonrpc_data(err.data.as_ref()) {
            return Some(bytes);
        }
    }
    revert_data_from_message(&e.to_string())
}

/// The `data` field of a JSON-RPC error response, when it holds revert bytes.
fn revert_data_from_jsonrpc_data(data: Option<&Value>) -> Option<Vec<u8>> {
    match data {
        Some(Value::String(hexdata)) => decode_hex(hexdata).filter(|b| !b.is_empty()),
        _ => None,
    }
}

/// Pull `0x…` revert bytes out of a node message.
///
/// Deliberately conservative: only accepts a hex blob of >= 4 bytes that the
/// message labels as revert data (`data 0x…`, `return data 0x…`, or a bare
/// `execution reverted: 0x…`). Anything else means "no data" — never a guess.
pub fn revert_data_from_message(msg: &str) -> Option<Vec<u8>> {
    for needle in ["data ", "return data ", "execution reverted: 0x"] {
        if let Some(idx) = msg.find(needle) {
            let rest = &msg[idx + needle.len()..];
            if let Some(bytes) = decode_hex(rest.trim()) {
                if bytes.len() >= 4 {
                    return Some(bytes);
                }
            }
        }
    }
    None
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim().trim_start_matches("0x");
    let end = s
        .char_indices()
        .find(|(_, c)| !c.is_ascii_hexdigit())
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let s = &s[..end];
    if s.is_empty() || s.len() % 2 != 0 {
        return None;
    }
    hex::decode(s).ok()
}

/// Deepest failing frame of a geth `callTracer` trace.
///
/// The tracer returns `{ to, input, output, error, revertReason, calls: [...] }`;
/// the frame that actually reverted is the deepest one with `error`, and its
/// `to` is the address we were missing. `output` carries the revert bytes when
/// the callee reverted with data, `revertReason` the decoded string otherwise.
pub fn cause_from_call_trace(trace: &Value) -> Option<RevertCause> {
    fn walk<'a>(frame: &'a Value, depth: usize, best: &mut Option<(&'a Value, usize)>) {
        if frame.get("error").is_some() {
            let replace = match best {
                Some((_, d)) => depth >= *d,
                None => true,
            };
            if replace {
                *best = Some((frame, depth));
            }
        }
        if let Some(calls) = frame.get("calls").and_then(Value::as_array) {
            for call in calls {
                walk(call, depth + 1, best);
            }
        }
    }

    let mut best: Option<(&Value, usize)> = None;
    walk(trace, 0, &mut best);
    let (frame, _depth) = best?;

    let address = frame.get("to").and_then(Value::as_str).map(str::to_string);
    let revert_reason = frame
        .get("revertReason")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let output = frame
        .get("output")
        .and_then(Value::as_str)
        .and_then(decode_hex);

    let mut cause = output
        .as_deref()
        .and_then(cause_from_revert_data)
        .or_else(|| revert_reason.as_deref().map(cause_from_revert_text))
        .unwrap_or_else(|| RevertCause::new("revert_no_data"));
    cause.address = address;
    if cause.revert_string.is_none() {
        cause.revert_string = revert_reason;
    }
    Some(cause)
}

/// Name a human-readable revert string (`revertReason` / node message).
pub fn cause_from_revert_text(text: &str) -> RevertCause {
    let lower = text.to_ascii_lowercase();
    let name = if text.contains("TRANSFER_FROM_FAILED") {
        "transfer_from_failed"
    } else if text.contains("INSUFFICIENT_OUTPUT_AMOUNT")
        || lower.contains("insufficient output amount")
    {
        "insufficient_output_amount"
    } else if text.contains("STF") || lower.contains("safeerc20 failed operation") {
        "safe_erc20_failed_operation"
    } else if lower.contains("insufficient liquidity") {
        "insufficient_liquidity"
    } else if lower.contains("insufficient balance") {
        "erc20_insufficient_balance"
    } else if lower.contains("reentrancy") {
        "reentrancy_guard_reentrant_call"
    } else if lower.contains("execution reverted") {
        "revert_no_data"
    } else {
        "revert_string"
    };
    let mut cause = RevertCause::new(name);
    cause.revert_string = Some(truncate_chars(text, 120));
    cause
}

/// Last-resort naming for a revert-shaped node message. Returns `None` when
/// the error is NOT a revert (transport/timeout/…) so the caller keeps the
/// exact pre-fix behaviour for those.
pub fn cause_from_error_message(msg: &str) -> Option<RevertCause> {
    let lower = msg.to_ascii_lowercase();
    if !lower.contains("revert") {
        return None;
    }
    Some(cause_from_revert_text(msg))
}

/// `pc` + `opcode` of the last executed step, from the bounded JS tracer
/// (`{"lastStep": {"pc": …, "op": …}}`).
pub fn cause_step_from_js_trace(result: &Value) -> Option<(u64, String)> {
    let step = result.get("lastStep")?;
    let pc = step.get("pc").and_then(Value::as_u64)?;
    let op = step.get("op").and_then(Value::as_str)?.to_string();
    Some((pc, op))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod f5_fork_revert_tests {
    use super::*;

    fn hex_of(bytes: &[u8]) -> String {
        format!("0x{}", hex::encode(bytes))
    }

    /// The exact string that 17/20 sims carried before this fix.
    #[test]
    fn anonymous_revert_with_empty_data_is_named_and_not_rpc_error() {
        let cause = cause_from_error_message("JsonRpcError: execution reverted, data 0x")
            .expect("a revert message must yield a cause");
        assert_eq!(cause.name, "revert_no_data");
        let reason = cause.fail_reason();
        assert!(reason.starts_with("fork_revert:revert_no_data"), "{reason}");
        assert!(!reason.contains("rpc_error"), "{reason}");
    }

    /// Transport errors keep the old family verbatim (semantics unchanged).
    #[test]
    fn transport_error_is_not_named_as_a_revert() {
        assert!(cause_from_error_message("connection reset by peer").is_none());
        assert!(cause_from_error_message("timeout").is_none());
    }

    /// `TransferHelper: TRANSFER_FROM_FAILED` — the real string seen 1/20.
    #[test]
    fn transfer_from_failed_string_is_named() {
        let cause = cause_from_error_message(
            "JsonRpcError: execution reverted: TransferHelper: TRANSFER_FROM_FAILED",
        )
        .expect("revert message");
        assert_eq!(cause.name, "transfer_from_failed");
    }

    /// A 4-byte custom error selector (unknown) must be published verbatim.
    #[test]
    fn unknown_selector_is_published() {
        let data = hex::decode("1b2c3d4e").unwrap();
        let cause = cause_from_revert_data(&data).expect("selector present");
        assert_eq!(cause.name, "revert_selector_0x1b2c3d4e");
        assert_eq!(cause.selector.as_deref(), Some("0x1b2c3d4e"));
    }

    /// Known selector: TransferHelper.TRANSFER_FROM_FAILED() = 0xedf429db.
    #[test]
    fn known_selector_is_mapped_to_its_name() {
        let data = hex::decode("edf429db").unwrap();
        let cause = cause_from_revert_data(&data).expect("selector present");
        assert_eq!(cause.name, "transfer_from_failed");
    }

    /// `Panic(0x11)` → arithmetic underflow, the FICHA REA-4 edge.
    #[test]
    fn panic_0x11_is_named() {
        let mut data = hex::decode("4e487b71").unwrap();
        data.extend_from_slice(&[0u8; 31]);
        data.push(0x11);
        let cause = cause_from_revert_data(&data).expect("panic payload");
        assert_eq!(cause.name, "panic_0x11");
        assert_eq!(cause.revert_string.as_deref(), Some("panic code 17"));
    }

    /// `Error(string)` round-trip.
    #[test]
    fn error_string_is_decoded() {
        let msg = "insufficient output amount";
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0u8; 31]);
        payload.push(32); // offset
        payload.extend_from_slice(&[0u8; 31]);
        payload.push(msg.len() as u8); // length
        payload.extend_from_slice(msg.as_bytes());
        let mut data = hex::decode("08c379a0").unwrap();
        data.extend_from_slice(&payload);
        let cause = cause_from_revert_data(&data).expect("error string");
        assert_eq!(cause.name, "error_string");
        assert_eq!(cause.revert_string.as_deref(), Some(msg));
    }

    /// callTracer: deepest failing frame gives the ADDRESS we were missing.
    #[test]
    fn call_trace_names_the_failing_frame_address() {
        let trace: Value = serde_json::from_str(
            r#"{
              "from":"0x1111111111111111111111111111111111111111",
              "to":"0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
              "input":"0x38ed1739",
              "error":"execution reverted",
              "calls":[
                {"type":"CALL","from":"0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
                 "to":"0xdac17f958d2ee523a2206206994597c13d831ec7",
                 "input":"0x23b872dd","error":"execution reverted",
                 "output":"0x"}
              ]
            }"#,
        )
        .unwrap();
        let cause = cause_from_call_trace(&trace).expect("failing frame");
        assert_eq!(cause.name, "revert_no_data");
        assert_eq!(
            cause.address.as_deref(),
            Some("0xdac17f958d2ee523a2206206994597c13d831ec7")
        );
        let reason = cause.fail_reason();
        assert!(reason.contains("address=0xdac17f"), "{reason}");
    }

    /// callTracer with revert bytes on the failing frame.
    #[test]
    fn call_trace_uses_output_selector() {
        let trace: Value = serde_json::from_str(
            r#"{
              "type":"CALL","to":"0x1111111111111111111111111111111111111111",
              "error":"execution reverted",
              "output":"0xedf429db"
            }"#,
        )
        .unwrap();
        let cause = cause_from_call_trace(&trace).expect("frame");
        assert_eq!(cause.name, "transfer_from_failed");
        assert_eq!(cause.selector.as_deref(), Some("0xedf429db"));
    }

    /// JS tracer result → pc + opcode (verified live against anvil:
    /// pc=473, op=REVERT for a garbage call to the V2 router).
    #[test]
    fn js_trace_step_yields_pc_and_opcode() {
        let v: Value = serde_json::from_str(
            r#"{"failed":true,"lastStep":{"pc":473,"op":"REVERT","depth":1}}"#,
        )
        .unwrap();
        assert_eq!(
            cause_step_from_js_trace(&v),
            Some((473, "REVERT".to_string()))
        );
    }

    /// Full label shape + 200-char budget; selector/address survive truncation.
    #[test]
    fn fail_reason_shape_and_budget() {
        let mut cause = RevertCause::new("transfer_from_failed");
        cause.selector = Some("0xedf429db".into());
        cause.address = Some("0xdac17f958d2ee523a2206206994597c13d831ec7".into());
        cause.pc = Some(473);
        cause.opcode = Some("REVERT".into());
        cause.revert_string = Some("x".repeat(300));
        let reason = cause.fail_reason();
        assert_eq!(
            reason.split(" msg=").next().unwrap(),
            "fork_revert:transfer_from_failed selector=0xedf429db \
             address=0xdac17f958d2ee523a2206206994597c13d831ec7 pc=473 op=REVERT"
        );
        assert!(reason.chars().count() <= MAX_REASON_CHARS);
    }

    /// The provider-error data path reads the JSON-RPC `data` field — the
    /// shape ethers exposes through `RpcError::as_error_response()` when the
    /// node DOES carry revert bytes.
    #[test]
    fn jsonrpc_error_data_is_decoded() {
        let data = Value::String("0xedf429db".to_string());
        let bytes = revert_data_from_jsonrpc_data(Some(&data)).expect("data");
        assert_eq!(hex_of(&bytes), "0xedf429db");
        let cause = cause_from_revert_data(&bytes).expect("cause");
        assert_eq!(cause.name, "transfer_from_failed");

        // Empty payload → nothing (never a fabricated cause).
        assert!(revert_data_from_jsonrpc_data(Some(&Value::String("0x".to_string()))).is_none());
        assert!(revert_data_from_jsonrpc_data(None).is_none());
    }

    /// A node message with the payload inline (anvil sometimes does this).
    #[test]
    fn revert_data_from_message_extracts_hex() {
        let data = revert_data_from_message(
            "execution reverted, data 0xedf429db0000000000000000000000000000000000000000",
        )
        .expect("data");
        assert_eq!(&data[..4], &hex::decode("edf429db").unwrap()[..]);
        // Not a revert payload → nothing.
        assert!(revert_data_from_message("execution reverted, data 0x").is_none());
    }
}
