//! Explicit per-chain activation, including mainnet. Every signing attempt
//! checks this policy; malformed configuration never expands the allowlist.
pub const DEFAULT_LIVE_CHAINS: &[u64] = &[11_155_111];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LiveExecDenied {
    #[error("live execution is disabled (ARBX_LIVE_EXEC_ENABLED != 'true')")]
    NotEnabled,
    #[error("chain_id {got} is not in the live-execution allowlist {allowed:?}")]
    ChainNotAllowed { got: u64, allowed: Vec<u64> },
}

#[derive(Debug, Clone)]
pub struct LiveExecPolicy {
    pub enabled: bool,
    pub allowed_chains: Vec<u64>,
}
impl LiveExecPolicy {
    pub fn from_env() -> Self {
        Self::from_raw(
            std::env::var("ARBX_LIVE_EXEC_ENABLED").ok().as_deref(),
            std::env::var("ARBX_LIVE_EXEC_CHAINS").ok().as_deref(),
        )
    }
    pub fn from_raw(enabled: Option<&str>, chains: Option<&str>) -> Self {
        let parsed = match chains {
            None => Some(DEFAULT_LIVE_CHAINS.to_vec()),
            Some(s) => s
                .split(',')
                .map(|x| x.trim().parse::<u64>().ok().filter(|x| *x > 0))
                .collect::<Option<Vec<_>>>(),
        };
        let mut allowed_chains = parsed.unwrap_or_default();
        allowed_chains.sort_unstable();
        allowed_chains.dedup();
        Self {
            enabled: enabled == Some("true"),
            allowed_chains,
        }
    }
    pub fn assert_broadcast_allowed(&self, chain_id: u64) -> Result<(), LiveExecDenied> {
        if !self.enabled {
            return Err(LiveExecDenied::NotEnabled);
        }
        if !self.allowed_chains.contains(&chain_id) {
            return Err(LiveExecDenied::ChainNotAllowed {
                got: chain_id,
                allowed: self.allowed_chains.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_disabled() {
        let p = LiveExecPolicy::from_raw(None, None);
        assert_eq!(
            p.assert_broadcast_allowed(1),
            Err(LiveExecDenied::NotEnabled)
        );
        assert_eq!(
            p.assert_broadcast_allowed(11_155_111),
            Err(LiveExecDenied::NotEnabled)
        );
    }
    #[test]
    fn explicit_mainnet_is_supported() {
        let p = LiveExecPolicy::from_raw(Some("true"), Some("1,11155111"));
        assert!(p.assert_broadcast_allowed(1).is_ok());
        assert!(p.assert_broadcast_allowed(11_155_111).is_ok());
        assert!(p.assert_broadcast_allowed(137).is_err());
    }
    #[test]
    fn invalid_allowlist_never_partially_activates() {
        for s in ["", "1,garbage", "1,", "0,1", "-1", "1,,11155111"] {
            let p = LiveExecPolicy::from_raw(Some("true"), Some(s));
            assert!(p.allowed_chains.is_empty());
            assert!(p.assert_broadcast_allowed(1).is_err());
        }
    }
    #[test]
    fn exact_true_only() {
        for s in ["", "false", "1", "TRUE", "true "] {
            assert!(!LiveExecPolicy::from_raw(Some(s), Some("1")).enabled);
        }
    }
    #[test]
    fn enabled_default_is_sepolia() {
        let p = LiveExecPolicy::from_raw(Some("true"), None);
        assert!(p.assert_broadcast_allowed(11_155_111).is_ok());
        assert!(p.assert_broadcast_allowed(1).is_err());
    }
}
