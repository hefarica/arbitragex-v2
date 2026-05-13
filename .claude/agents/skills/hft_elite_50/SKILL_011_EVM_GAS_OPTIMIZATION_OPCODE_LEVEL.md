# SKILL: EVM Gas Optimization at Opcode Level
**Level:** PhD Computer Science | EVM Architecture Grandmaster
**Specialty:** Smart Contract Optimization & MEV Extraction

## AGENT DIRECTIVE
Cada unidad de gas es un enemigo. Minimiza el costo computacional hasta el límite absoluto del EVM.

## OPTIMIZATION TACTICS
```solidity
// STORAGE vs MEMORY vs STACK
uint256 x = storageVar;  // SLOAD = 100-2100 gas
uint256 x = memoryVar;   // MLOAD = 3 gas
uint256 x = 1;           // PUSH1 = 3 gas

// PACKING
struct Packed {
    uint128 a;  // Slot 0
    uint128 b;  // Slot 0
}

// SHORT CIRCUIT
if (cheapCheck && expensiveCheck)

// LOOP UNROLLING
for (uint i; i < 3; ) {
    unchecked { ++i; }
}

// BITWISE vs ARITHMETIC
x * 2  →  x << 1
x / 2  →  x >> 1
x % 2  →  x & 1
```

## ACCESS LISTS (EIP-2929)
```python
access_list = [{"address": "0x...pool", "storageKeys": ["slot0", "slot1"]}]
# Reduce SLOAD de 2100 a 100 gas
```

## GAS PRICE STRATEGY (Post-EIP-1559)
```
Base Fee: Determinado por congestion (quemado)
Priority Fee: Pago al validator
Max fee: base_fee_next + priority_fee + 10% buffer
```

## PROFIT CALCULATION
```python
gas_cost = gas_used * (base_fee + priority_fee)
profit = revenue - gas_cost - flash_loan_fee - swap_fees
min_profit = gas_cost * 2
```
