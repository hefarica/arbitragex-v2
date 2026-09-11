//! Exact USD feed reads shared by simulation and receipt accounting.
//! State is pinned with EIP-1898; transport errors never disclose RPC URLs.
use anyhow::{anyhow, bail, Context, Result};
use ethers::types::{Address, Bytes, H256, U256};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Duration};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feed {
    pub address: Address,
    pub quote: String,
    pub max_age_secs: u64,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainFeeds {
    pub native_usd: Feed,
    pub assets_usd: BTreeMap<String, Feed>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price {
    pub answer: U256,
    pub decimals: u8,
    pub updated_at: u64,
}

pub fn configured_chain(chain: u64) -> Result<ChainFeeds> {
    let config: BTreeMap<String, ChainFeeds> = serde_json::from_str(
        &std::env::var("ARBX_ACCOUNTING_FEEDS_JSON").context("oracle_feeds_missing")?,
    )
    .map_err(|_| anyhow!("oracle_feeds_invalid"))?;
    config
        .get(&chain.to_string())
        .cloned()
        .ok_or_else(|| anyhow!("oracle_chain_feeds_missing"))
}

#[derive(Clone)]
pub struct OracleRpc {
    client: reqwest::Client,
    url: String,
}
impl OracleRpc {
    pub fn from_url(url: &str) -> Result<Self> {
        let parsed = reqwest::Url::parse(url).map_err(|_| anyhow!("oracle_rpc_invalid"))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            bail!("oracle_rpc_scheme");
        }
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?,
            url: url.to_owned(),
        })
    }
    pub fn from_env(chain: u64) -> Result<Self> {
        Self::from_url(&std::env::var(format!("RPC_HTTP_{chain}")).context("oracle_rpc_missing")?)
    }
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let r = self
            .client
            .post(&self.url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .await
            .map_err(|_| anyhow!("oracle_rpc_transport"))?
            .error_for_status()
            .map_err(|_| anyhow!("oracle_rpc_http"))?
            .json::<Value>()
            .await
            .map_err(|_| anyhow!("oracle_rpc_json"))?;
        if r.get("error").is_some() {
            bail!("oracle_rpc_method_failed:{method}");
        }
        r.get("result")
            .filter(|v| !v.is_null())
            .cloned()
            .ok_or_else(|| anyhow!("oracle_rpc_result_missing:{method}"))
    }
    pub async fn read(&self, target: Address, calldata: &str, block: H256) -> Result<Vec<u8>> {
        if block.is_zero() || target.is_zero() {
            bail!("oracle_state_identity_invalid");
        }
        let r = self
            .call(
                "eth_call",
                json!([{"to":format!("{target:#x}"),"data":calldata},
            {"blockHash":format!("{block:#x}"),"requireCanonical":true}]),
            )
            .await?;
        r.as_str()
            .filter(|v| v.starts_with("0x"))
            .ok_or_else(|| anyhow!("oracle_rpc_hex_invalid"))?
            .parse::<Bytes>()
            .map(|b| b.to_vec())
            .map_err(|_| anyhow!("oracle_rpc_hex_invalid"))
    }
    pub async fn price(&self, feed: &Feed, block: H256, block_ts: u64) -> Result<Price> {
        let (round, decimals) = tokio::try_join!(
            self.read(feed.address, "0xfeaf968c", block),
            self.read(feed.address, "0x313ce567", block)
        )?;
        parse_price(&round, &decimals, feed, block_ts)
    }
    pub async fn token_decimals(&self, token: Address, block: H256) -> Result<u8> {
        parse_decimals(&self.read(token, "0x313ce567", block).await?)
    }
}
fn word(bytes: &[u8], i: usize) -> Result<U256> {
    bytes
        .get(i * 32..(i + 1) * 32)
        .map(U256::from_big_endian)
        .ok_or_else(|| anyhow!("oracle_abi_short_word"))
}
pub fn parse_decimals(bytes: &[u8]) -> Result<u8> {
    if bytes.len() != 32 {
        bail!("oracle_decimals_abi_invalid");
    }
    let n = word(bytes, 0)?;
    if n > 77.into() {
        bail!("oracle_decimals_unsupported");
    }
    Ok(n.as_u32() as u8)
}
pub fn parse_price(bytes: &[u8], decimals: &[u8], feed: &Feed, block_ts: u64) -> Result<Price> {
    if feed.address.is_zero() || feed.quote != "USD" || !(1..=86400).contains(&feed.max_age_secs) {
        bail!("oracle_feed_invalid");
    }
    if bytes.len() != 160 {
        bail!("oracle_feed_abi_invalid");
    }
    let round = word(bytes, 0)?;
    let answer = word(bytes, 1)?;
    let updated = word(bytes, 3)?;
    // answeredInRound is deprecated and may be zero; its declared ABI is uint80.
    if round.is_zero()
        || round >= (U256::one() << 80)
        || word(bytes, 4)? >= (U256::one() << 80)
        || answer.is_zero()
        || answer.bit(255)
        || updated.is_zero()
        || word(bytes, 2)? > updated
        || updated > block_ts.into()
        || U256::from(block_ts) - updated > feed.max_age_secs.into()
    {
        bail!("oracle_price_stale_or_invalid");
    }
    Ok(Price {
        answer,
        decimals: parse_decimals(decimals)?,
        updated_at: updated.as_u64(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn words(v: &[U256]) -> Vec<u8> {
        v.iter()
            .flat_map(|x| {
                let mut b = [0; 32];
                x.to_big_endian(&mut b);
                b
            })
            .collect()
    }
    fn feed() -> Feed {
        Feed {
            address: Address::from_low_u64_be(1),
            quote: "USD".into(),
            max_age_secs: 60,
        }
    }
    fn round(x: U256, ts: u64) -> Vec<u8> {
        words(&[1.into(), x, 900.into(), ts.into(), 0.into()])
    }
    #[test]
    fn exact_large_answer_and_deprecated_round_zero() {
        let answer = (U256::one() << 200) + 7;
        assert_eq!(
            parse_price(&round(answer, 990), &words(&[8.into()]), &feed(), 1000).unwrap(),
            Price {
                answer,
                decimals: 8,
                updated_at: 990
            }
        );
    }
    #[test]
    fn invalid_prices_and_time_cannot_be_used() {
        for (answer, ts) in [
            (U256::MAX, 990),
            (U256::zero(), 990),
            (U256::one(), 1001),
            (U256::one(), 939),
        ] {
            assert!(parse_price(&round(answer, ts), &words(&[8.into()]), &feed(), 1000).is_err());
        }
        assert!(parse_price(&round(1.into(), 940), &words(&[8.into()]), &feed(), 1000).is_ok());
        assert!(parse_price(
            &words(&[1.into(), 100.into(), 991.into(), 990.into(), 0.into()]),
            &words(&[8.into()]),
            &feed(),
            1000
        )
        .is_err());
    }
    #[test]
    fn wrong_quote_and_abi_rejected() {
        let good = round(100.into(), 990);
        let d = words(&[8.into()]);
        let mut f = feed();
        f.quote = "ETH".into();
        assert!(parse_price(&good, &d, &f, 1000).is_err());
        assert!(parse_price(&good[..159], &d, &feed(), 1000).is_err());
        assert!(parse_price(&good, &words(&[256.into()]), &feed(), 1000).is_err());
        let mut bad = good;
        bad[0] = 1;
        assert!(parse_price(&bad, &d, &feed(), 1000).is_err());
        assert!(parse_decimals(&[0; 31]).is_err());
    }
}
