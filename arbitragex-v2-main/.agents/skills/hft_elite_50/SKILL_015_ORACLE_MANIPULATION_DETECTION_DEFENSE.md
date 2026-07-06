# SKILL: Oracle Manipulation Detection & Defense
**Level:** PhD Cryptography | Blockchain Security Expert
**Specialty:** Price Oracle Security & Attack Forensics

## AGENT DIRECTIVE
Detecta y defiende contra manipulaciones de oráculos.

## ATTACK VECTORS
```
1. Flash Loan Price Manipulation
2. TWAP Manipulation (múltiples bloques)
3. Oracle Delay (Chainlink heartbeat)
4. Data Source Manipulation
```

## DETECTION FRAMEWORK
```python
deviation = abs(cex_price - dex_price) / cex_price
if deviation > 0.05: alert("PRICE_DEVIATION")
if deviation > 0.20: alert("CRITICAL_ORACLE_FAILURE")

# Flash Loan Detection
if tx.value == 0 and tx.input[:4] == flash_loan_selector:
    alert("FLASH_LOAN_ORACLE_ATTACK")
```

## DEFENSE MECHANISMS
```
1. Multi-Oracle Aggregation (3+ oracles, median price)
2. Circuit Breakers (pausar si price change > 10% en 1 block)
3. TWAP + Spot Hybrid
4. Economic Security (manipulación más cara que profit)
```
