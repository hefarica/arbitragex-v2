# SKILL: Distributed Systems & Blockchain Consensus
**Level:** PhD Distributed Systems | Consensus Protocol Architect
**Specialty:** Byzantine Fault Tolerance & State Replication

## AGENT DIRECTIVE
Entiende la blockchain como un sistema distribuido con nodos adversariales. La **consensus** es el corazón. La finalidad es tu garantía.

## CORE KNOWLEDGE
- **CAP Theorem:** Consistency, Availability, Partition tolerance
- **Byzantine Generals:** Consensus con nodos maliciosos
- **PoW/PoS:** Nakamoto consensus, Casper FFG
- **Finality:** Probabilistic vs Instant vs Finalized

## CONSENSUS COMPARISON
```
Mechanism     | Finality      | Latency    | Throughput   | Decentralization
--------------|---------------|------------|--------------|-----------------
PoW (Bitcoin) | Probabilistic | 10 min     | 7 TPS        | High
PoS (ETH 2.0) | Finalized     | 12 min     | 30 TPS       | Medium
Tendermint    | Instant       | 1-3 sec    | 1000+ TPS    | Medium
HotStuff      | Instant       | <1 sec     | 10000+ TPS   | Medium
Solana        | Optimistic    | 400 ms     | 65000 TPS    | Low
```

## TRADING IMPLICATIONS
```
- ETH PoS: Esperar 2 epochs (12.8 min) para finality
- Para tx rápida: 1 epoch (6.4 min) con riesgo de reorg
- Para tx instantánea: Aceptar riesgo de reorg (<1%)
```
