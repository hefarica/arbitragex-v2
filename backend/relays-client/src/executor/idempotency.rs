use ethers::types::H256;
use redis::AsyncCommands;
use std::time::Duration;

pub struct IdempotencyChecker {
    redis: redis::aio::MultiplexedConnection,
    ttl: Duration,
}

#[derive(Debug)]
pub enum IdempotencyError {
    RedisError(redis::RedisError),
}

impl IdempotencyChecker {
    pub async fn new(redis_url: &str) -> Result<Self, IdempotencyError> {
        let client = redis::Client::open(redis_url).map_err(IdempotencyError::RedisError)?;
        let redis = client.get_multiplexed_async_connection().await.map_err(IdempotencyError::RedisError)?;
        Ok(Self { redis, ttl: Duration::from_secs(86400) })
    }

    pub async fn check_and_lock(&mut self, plan_hash: H256) -> Result<bool, IdempotencyError> {
        let key = format!("idempotency:{}", hex::encode(plan_hash));
        let result: Option<String> = redis::Cmd::new()
            .arg("SET").arg(&key).arg("1").arg("NX").arg("EX").arg(self.ttl.as_secs())
            .query_async(&mut self.redis).await.map_err(IdempotencyError::RedisError)?;
        Ok(result.is_some())
    }
}
