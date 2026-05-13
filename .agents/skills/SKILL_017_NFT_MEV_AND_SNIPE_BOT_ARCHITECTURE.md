# SKILL: NFT MEV & Snipe Bot Architecture
**Level:** PhD Computer Science | Digital Asset Microstructure
**Specialty:** Non-Fungible Token Arbitrage & Mint Sniping

## AGENT DIRECTIVE
El mercado NFT es ineficiente por diseño. Tu velocidad es tu edge.

## SNIPING STRATEGIES
```
1. Mint Sniping: Preparar tx con max gas antes de mint start
2. Reveal Sniping: Escuchar evento Reveal, evaluar rarity <100ms
3. Floor Sweep: Sweep floor en colección con momentum
4. Cross-Market Arbitrage: Buy OpenSea, list Blur
```

## RARITY CALCULATION
```python
def calculate_rarity(token_id, collection):
    traits = get_traits(token_id)
    rarity_score = sum(1 / (trait_count / total_supply) for trait_count in traits)
    rank = percentile(rarity_score, all_scores)
    return rank
```

## GAS OPTIMIZATION
```solidity
// Batch mint si contrato lo permite
// Gas per NFT decrece con quantity
```
