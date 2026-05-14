//! Connector error types — R8 Fail-Honest.
//!
//! Every error variant carries enough context for the observer to
//! diagnose the failure without fabricating fallback data.

use std::fmt;

#[derive(Debug)]
pub enum ConnectorError {
    RpcConnectionFailed {
        endpoint: String,
        reason: String,
    },
    WsSubscriptionFailed {
        topic: String,
        reason: String,
    },
    ContractCallFailed {
        contract: String,
        method: String,
        reason: String,
    },
    InvalidResponse {
        expected: String,
        got: String,
    },
    ConfigMissing {
        parameter: String,
    },
    Timeout {
        operation: String,
        timeout_ms: u64,
    },
    SimulationReverted {
        reason: String,
    },
    SigningAttempted,
    DataQuality {
        source: String,
        issue: String,
    },
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RpcConnectionFailed { endpoint, reason } =>
                write!(f, "RPC connection failed: endpoint={endpoint}, reason={reason}"),
            Self::WsSubscriptionFailed { topic, reason } =>
                write!(f, "WS subscription failed: topic={topic}, reason={reason}"),
            Self::ContractCallFailed { contract, method, reason } =>
                write!(f, "Contract call failed: contract={contract}, method={method}, reason={reason}"),
            Self::InvalidResponse { expected, got } =>
                write!(f, "Invalid response: expected={expected}, got={got}"),
            Self::ConfigMissing { parameter } =>
                write!(f, "Configuration missing: parameter={parameter}"),
            Self::Timeout { operation, timeout_ms } =>
                write!(f, "Timeout: operation={operation}, timeout_ms={timeout_ms}"),
            Self::SimulationReverted { reason } =>
                write!(f, "Simulation reverted: reason={reason}"),
            Self::SigningAttempted =>
                write!(f, "Signing attempted in paper-shadow mode — CRITICAL VIOLATION"),
            Self::DataQuality { source, issue } =>
                write!(f, "Data quality: source={source}, issue={issue}"),
        }
    }
}

impl std::error::Error for ConnectorError {}
