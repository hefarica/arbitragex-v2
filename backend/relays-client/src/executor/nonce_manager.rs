use ethers::types::{Address, U256};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use ethers_providers::{Http, Middleware, Provider};

pub struct NonceManager {
    provider: Arc<Provider<Http>>,
    address: Address,
    next_nonce: U256,
    locked_nonces: VecDeque<(U256, Instant)>,
    lock_ttl: Duration,
}

#[derive(Debug)]
pub enum NonceManagerError {
    ProviderError(String),
}

impl NonceManager {
    pub async fn new(
        provider: Arc<Provider<Http>>,
        address: Address,
    ) -> Result<Self, NonceManagerError> {
        let next_nonce = provider
            .get_transaction_count(address, None)
            .await
            .map_err(|e| NonceManagerError::ProviderError(e.to_string()))?;

        Ok(Self {
            provider,
            address,
            next_nonce,
            locked_nonces: VecDeque::new(),
            lock_ttl: Duration::from_secs(300),
        })
    }

    pub async fn acquire_nonce(&mut self) -> Result<U256, NonceManagerError> {
        self.clean_expired_locks();
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce + 1;
        self.locked_nonces.push_back((nonce, Instant::now()));
        Ok(nonce)
    }

    pub async fn release_nonce(&mut self, nonce: U256) -> Result<(), NonceManagerError> {
        if let Some(pos) = self.locked_nonces.iter().position(|(n, _)| *n == nonce) {
            self.locked_nonces.remove(pos);
        }
        Ok(())
    }

    fn clean_expired_locks(&mut self) {
        let now = Instant::now();
        while let Some((_, timestamp)) = self.locked_nonces.front() {
            if now.duration_since(*timestamp) > self.lock_ttl {
                self.locked_nonces.pop_front();
            } else {
                break;
            }
        }
    }
}
