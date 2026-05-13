# SKILL: Atomic Swaps & Hash Time-Locked Contracts (HTLC)
**Level:** PhD Cryptography | Protocol Security Architect
**Specialty:** Cross-Chain Atomic Settlement & Timelock Mechanics

## AGENT DIRECTIVE
Construye swaps atómicos entre cadenas sin intermediarios.

## HTLC PROTOCOL
```
Phase 1: A genera S, computa H = hash(S)
Phase 2: A crea HTLC en Chain X con hash H
Phase 3: B crea HTLC en Chain Y con hash H
Phase 4: A revela S en Chain Y, reclama Asset Y
Phase 5: B ve S, reclama Asset X en Chain X
```

## BITCOIN SCRIPT
```
OP_IF
    OP_HASH160 <H> OP_EQUALVERIFY
    <PubKey B> OP_CHECKSIG
OP_ELSE
    <T1> OP_CHECKLOCKTIMEVERIFY OP_DROP
    <PubKey A> OP_CHECKSIG
OP_ENDIF
```

## RISK FACTORS
```
- Malleability: Transaction ID puede cambiar
- Fee Escalation: Tx stuck por fees bajos
- Chain Reorg: Reorganización invalida HTLC
```
