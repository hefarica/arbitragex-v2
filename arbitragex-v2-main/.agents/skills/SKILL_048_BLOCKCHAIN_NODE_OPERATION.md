# SKILL: Blockchain Node Operation & Validation
**Level:** PhD Distributed Systems | Blockchain Infrastructure Expert
**Specialty:** Node Synchronization & Validator Economics

## AGENT DIRECTIVE
Tu nodo es tu conexión a la verdad. No confíes en RPCs de terceros. Ejecuta tu propio nodo.

## NODE TYPES
```
Type          | Storage    | RAM      | Use Case
--------------|------------|----------|------------------
Full Node     | 1-2 TB     | 16 GB    | Validation
Archive Node  | 10-20 TB   | 32 GB    | Historical queries
Validator     | 2 TB       | 32 GB    | Consensus
RPC Node      | 2 TB       | 64 GB    | Serving dApps
```

## ETHEREUM SETUP
```bash
# Execution Client (Geth)
geth --mainnet --syncmode snap --http --ws --authrpc.jwtsecret /secrets/jwt.hex

# Consensus Client (Lighthouse)
lighthouse bn --network mainnet --http --execution-endpoint http://localhost:8551

# Validator (if staking)
lighthouse vc --network mainnet --suggested-fee-recipient 0x...
```

## VALIDATOR ECONOMICS
```
Stake: 32 ETH per validator
APR: 3-5% (variable)
Costs: Hardware $2000, Electricity $50/month, Internet $100/month
Risks: Slashing (-0.5 to -16 ETH), Inactivity leak (-0.01 ETH/day)
MEV-Boost: +200-500% revenue via relay network
```
