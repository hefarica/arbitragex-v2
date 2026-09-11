//! Ethereum/Sepolia Shanghai through Osaka/BPO2. Unknown chains fail closed.
//! Schedule: https://github.com/ethereum/go-ethereum/blob/master/params/config.go
use crate::lazy_db::ChainSnapshot;
use ethers::types::U256 as EU256;
use revm::context::BlockEnv;
use revm::primitives::{hardfork::SpecId, Address, B256, U256};

pub fn osaka_spec() -> SpecId {
    SpecId::OSAKA
}

pub fn fork(chain: u64, timestamp: u64) -> Result<(SpecId, Option<u64>), &'static str> {
    let [shanghai, cancun, prague, osaka, bpo1, bpo2] = match chain {
        1 => [
            1681338455, 1710338135, 1746612311, 1764798551, 1765290071, 1767747671,
        ],
        11155111 => [
            1677557088, 1706655072, 1741159776, 1760427360, 1761017184, 1761607008,
        ],
        _ => return Err("unsupported_execution_chain"),
    };
    if timestamp < shanghai {
        return Err("pre_shanghai_snapshot_unsupported");
    }
    let spec = if timestamp >= osaka {
        SpecId::OSAKA
    } else if timestamp >= prague {
        SpecId::PRAGUE
    } else if timestamp >= cancun {
        SpecId::CANCUN
    } else {
        SpecId::SHANGHAI
    };
    let fraction = if timestamp >= bpo2 {
        Some(11684671)
    } else if timestamp >= bpo1 {
        Some(8346193)
    } else if timestamp >= prague {
        Some(5007716)
    } else if timestamp >= cancun {
        Some(3338477)
    } else {
        None
    };
    Ok((spec, fraction))
}

pub fn environment(s: &ChainSnapshot) -> Result<(BlockEnv, SpecId), &'static str> {
    let (spec, fraction) = fork(s.chain_id, s.timestamp)?;
    let h = &s.header;
    let small = |v: EU256| -> Result<u64, &'static str> {
        if v > EU256::from(u64::MAX) {
            Err("header_integer_overflow")
        } else {
            Ok(v.as_u64())
        }
    };
    if h.hash != Some(s.hash)
        || h.number.map(|n| n.as_u64()) != Some(s.number)
        || h.timestamp != EU256::from(s.timestamp)
        || !h.difficulty.is_zero()
    {
        return Err("snapshot_header_mismatch");
    }
    let mut block = BlockEnv {
        number: U256::from(s.number),
        timestamp: U256::from(s.timestamp),
        beneficiary: Address::from_slice(h.author.ok_or("header_coinbase_missing")?.as_bytes()),
        gas_limit: small(h.gas_limit)?,
        basefee: small(h.base_fee_per_gas.ok_or("header_basefee_missing")?)?,
        difficulty: U256::ZERO,
        prevrandao: Some(B256::from(h.mix_hash.ok_or("header_randao_missing")?.0)),
        blob_excess_gas_and_price: None,
        ..Default::default()
    };
    if block.gas_limit <= 21_000 {
        return Err("header_gas_limit_invalid");
    }
    if let Some(fraction) = fraction {
        let excess = small(h.excess_blob_gas.ok_or("header_excess_blob_gas_missing")?)?;
        block.set_blob_excess_gas_and_price(excess, fraction);
    }
    Ok((block, spec))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fork_boundaries_and_chain_isolation() {
        assert_eq!(fork(1, 1746612310).unwrap().0, SpecId::CANCUN);
        assert_eq!(fork(1, 1746612311).unwrap().0, SpecId::PRAGUE);
        assert_eq!(fork(1, 1764798551).unwrap().0, SpecId::OSAKA);
        assert_eq!(fork(1, 1767747671).unwrap().1, Some(11684671));
        assert_eq!(fork(11155111, 1761607008).unwrap().1, Some(11684671));
        assert_eq!(fork(1, 1761607008).unwrap().0, SpecId::PRAGUE);
        assert!(fork(42161, 1761607008).is_err());
        assert!(fork(1, 1).is_err());
    }
    #[test]
    fn missing_execution_header_rejected() {
        let s = ChainSnapshot {
            chain_id: 1,
            number: 1,
            hash: ethers::types::H256::repeat_byte(1),
            timestamp: 1767747671,
            header: Default::default(),
        };
        assert!(environment(&s).is_err());
    }
}
