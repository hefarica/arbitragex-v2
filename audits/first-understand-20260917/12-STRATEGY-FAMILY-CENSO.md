# CENSO ESTRATEGIA→FAMILIA — cartuchos individuales (2026-09-17)
> Fuente canónica: capability_matrix.json + cartridges/strategies/*.rhai. La familia es nivel SECUNDARIO;
> cada estrategia se lista individual. El allowlist enabled_strategies cubre la FAMILIA DE CONTRATO;
> los cartuchos colapsan en ella por design (§34.1) — no requieren entrada individual.

## Familia MEV-01 — dex-dex arbitrage (36 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-01-001** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-002** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-003** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-004** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-005** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-006** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-007** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-008** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-009** | R_ORDERBOOK | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-01-010** | R_ORDERBOOK | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-01-011** | R_ORDERBOOK | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-01-012** | R_ORDERBOOK | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-01-013** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-014** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-015** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-016** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-017** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-018** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-019** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-020** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-021** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-022** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-023** | R_DIRECT_INDIRECT | ✅ | ✅ | 7 | op_01, op_15, op_20, op_21, op_22, op_27, op_32 |
| **MEV-01-024** | R_DIRECT_INDIRECT | ✅ | ✅ | 7 | op_01, op_15, op_20, op_21, op_22, op_27, op_32 |
| **MEV-01-025** | R_SPLIT | ✅ | ✅ | 7 | op_15, op_19, op_20, op_21, op_22, op_27, op_32 |
| **MEV-01-026** | R_SPLIT | ✅ | ✅ | 7 | op_15, op_19, op_20, op_21, op_22, op_27, op_32 |
| **MEV-01-027** | R_BASKET_NAV | ✅ | ✅ | 8 | op_08, op_11, op_13, op_16, op_19, op_21, op_22, op_32 |
| **MEV-01-028** | R_BASKET_NAV | ✅ | ✅ | 8 | op_08, op_11, op_13, op_16, op_19, op_21, op_22, op_32 |
| **MEV-01-029** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-030** | R_COW | ✅ | ✅ | 8 | op_19, op_20, op_22, op_23, op_24, op_27, op_29, op_32 |
| **MEV-01-031** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-032** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-033** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-034** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-035** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |
| **MEV-01-036** | R_CLOSED_CYCLE | ✅ | ✅ | 9 | op_01, op_15, op_16, op_21, op_22, op_26, op_27, op_30, op_32 |

## Familia MEV-02 — cross-pool/protocol (17 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-02-001** | CF_CPMM | ✅ | ✅ | 6 | op_15, op_16, op_21, op_22, op_26, op_27 |
| **MEV-02-002** | CF_CONSTANT_SUM | ✅ | ✅ | 4 | op_15, op_19, op_21, op_22 |
| **MEV-02-003** | CF_STABLESWAP | ✅ | ✅ | 5 | op_15, op_16, op_21, op_22, op_27 |
| **MEV-02-004** | CF_WEIGHTED | ✅ | ✅ | 5 | op_15, op_16, op_19, op_21, op_22 |
| **MEV-02-005** | CF_CLAMM | ✅ | ✅ | 7 | op_10, op_15, op_16, op_21, op_22, op_27, op_30 |
| **MEV-02-006** | CF_LB | ✅ | ✅ | 5 | op_10, op_15, op_21, op_22, op_27 |
| **MEV-02-007** | CF_PMM | ✅ | ✅ | 5 | op_08, op_13, op_15, op_21, op_22 |
| **MEV-02-008** | CF_DYNAMIC | ✅ | ✅ | 6 | op_08, op_10, op_13, op_15, op_21, op_22 |
| **MEV-02-009** | CF_DYNAMIC | ✅ | ✅ | 6 | op_08, op_10, op_13, op_15, op_21, op_22 |
| **MEV-02-010** | CF_BOND | ✅ | ✅ | 5 | op_13, op_15, op_20, op_21, op_22 |
| **MEV-02-011** | CF_DYNAMIC | ✅ | ✅ | 6 | op_08, op_10, op_13, op_15, op_21, op_22 |
| **MEV-02-012** | CF_VAMM | ✅ | ✅ | 5 | op_08, op_10, op_13, op_21, op_22 |
| **MEV-02-013** | CF_DYNAMIC | ✅ | ✅ | 6 | op_08, op_10, op_13, op_15, op_21, op_22 |
| **MEV-02-014** | CF_TWAMM | ✅ | ✅ | 5 | op_05, op_08, op_21, op_22, op_27 |
| **MEV-02-015** | CF_BATCH | ✅ | ✅ | 6 | op_19, op_20, op_22, op_23, op_24, op_29 |
| **MEV-02-016** | CF_BATCH | ✅ | ✅ | 6 | op_19, op_20, op_22, op_23, op_24, op_29 |
| **MEV-02-017** | CF_CROSSINV | ✅ | ✅ | 6 | op_15, op_19, op_21, op_22, op_27, op_30 |

## Familia MEV-03 — liquidaciones (31 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-03-001** | E_POST | ✅ | ✅ | 9 | op_05, op_08, op_10, op_11, op_21, op_22, op_25, op_27, op_32 |
| **MEV-03-002** | E_POST | ✅ | ✅ | 9 | op_05, op_08, op_10, op_11, op_21, op_22, op_25, op_27, op_32 |
| **MEV-03-003** | E_POST | ✅ | ✅ | 9 | op_05, op_08, op_10, op_11, op_21, op_22, op_25, op_27, op_32 |
| **MEV-03-004** | E_POST | ✅ | ✅ | 9 | op_05, op_08, op_10, op_11, op_21, op_22, op_25, op_27, op_32 |
| **MEV-03-005** | E_POST | ✅ | ✅ | 9 | op_05, op_08, op_10, op_11, op_21, op_22, op_25, op_27, op_32 |
| **MEV-03-006** | E_POST | ✅ | ✅ | 9 | op_05, op_08, op_10, op_11, op_21, op_22, op_25, op_27, op_32 |
| **MEV-03-007** | E_LATENCY | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-03-008** | E_LATENCY | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-03-009** | E_ORACLE | ✅ | ✅ | 8 | op_05, op_08, op_10, op_11, op_21, op_22, op_25, op_32 |
| **MEV-03-010** | E_ORACLE | ✅ | ✅ | 8 | op_05, op_08, op_10, op_11, op_21, op_22, op_25, op_32 |
| **MEV-03-011** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-012** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-013** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-014** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-015** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-016** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-017** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-018** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-019** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-020** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-021** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-022** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-023** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-024** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-03-025** | E_AUCTION | ✅ | ✅ | 7 | op_08, op_19, op_21, op_22, op_23, op_24, op_32 |
| **MEV-03-026** | E_AUCTION | ✅ | ✅ | 7 | op_08, op_19, op_21, op_22, op_23, op_24, op_32 |
| **MEV-03-027** | E_LATENCY | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-03-028** | E_LATENCY | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-03-029** | OBSERVE | ✅ | ✅ | 3 | op_10, op_11, op_22 |
| **MEV-03-030** | OBSERVE | ✅ | ✅ | 3 | op_10, op_11, op_22 |
| **MEV-03-031** | E_STATE | ✅ | ✅ | 7 | op_05, op_08, op_11, op_13, op_21, op_22, op_32 |

## Familia MEV-04 — flash/TLS (31 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-04-001** | P_PEG | ✅ | ✅ | 6 | op_08, op_11, op_13, op_15, op_21, op_22 |
| **MEV-04-002** | P_PEG | ✅ | ✅ | 6 | op_08, op_11, op_13, op_15, op_21, op_22 |
| **MEV-04-003** | P_PEG | ✅ | ✅ | 6 | op_08, op_11, op_13, op_15, op_21, op_22 |
| **MEV-04-004** | P_PEG | ✅ | ✅ | 6 | op_08, op_11, op_13, op_15, op_21, op_22 |
| **MEV-04-005** | P_PEG | ✅ | ✅ | 6 | op_08, op_11, op_13, op_15, op_21, op_22 |
| **MEV-04-006** | P_WRAP | ✅ | ✅ | 5 | op_08, op_13, op_15, op_21, op_22 |
| **MEV-04-007** | P_WRAP | ✅ | ✅ | 5 | op_08, op_13, op_15, op_21, op_22 |
| **MEV-04-008** | P_WRAP | ✅ | ✅ | 5 | op_08, op_13, op_15, op_21, op_22 |
| **MEV-04-009** | P_WRAP | ✅ | ✅ | 5 | op_08, op_13, op_15, op_21, op_22 |
| **MEV-04-010** | P_4626 | ✅ | ✅ | 5 | op_08, op_13, op_15, op_21, op_22 |
| **MEV-04-011** | P_4626 | ✅ | ✅ | 5 | op_08, op_13, op_15, op_21, op_22 |
| **MEV-04-012** | P_4626 | ✅ | ✅ | 5 | op_08, op_13, op_15, op_21, op_22 |
| **MEV-04-013** | P_NAV | ✅ | ✅ | 6 | op_08, op_11, op_13, op_19, op_21, op_22 |
| **MEV-04-014** | P_NAV | ✅ | ✅ | 6 | op_08, op_11, op_13, op_19, op_21, op_22 |
| **MEV-04-015** | P_NAV | ✅ | ✅ | 6 | op_08, op_11, op_13, op_19, op_21, op_22 |
| **MEV-04-016** | P_NAV | ✅ | ✅ | 6 | op_08, op_11, op_13, op_19, op_21, op_22 |
| **MEV-04-017** | P_PEG | ✅ | ✅ | 6 | op_08, op_11, op_13, op_15, op_21, op_22 |
| **MEV-04-018** | P_PEG | ✅ | ✅ | 6 | op_08, op_11, op_13, op_15, op_21, op_22 |
| **MEV-04-019** | P_LST | ✅ | ✅ | 6 | op_08, op_11, op_13, op_21, op_22, op_23 |
| **MEV-04-020** | P_LST | ✅ | ✅ | 6 | op_08, op_11, op_13, op_21, op_22, op_23 |
| **MEV-04-021** | P_LST | ✅ | ✅ | 6 | op_08, op_11, op_13, op_21, op_22, op_23 |
| **MEV-04-022** | P_LST | ✅ | ✅ | 6 | op_08, op_11, op_13, op_21, op_22, op_23 |
| **MEV-04-023** | P_WRAP | ✅ | ✅ | 5 | op_08, op_13, op_15, op_21, op_22 |
| **MEV-04-024** | P_YIELD | ✅ | ✅ | 6 | op_08, op_13, op_15, op_16, op_21, op_22 |
| **MEV-04-025** | P_NAV | ✅ | ✅ | 6 | op_08, op_11, op_13, op_19, op_21, op_22 |
| **MEV-04-026** | P_PTYT | ✅ | ✅ | 6 | op_08, op_13, op_15, op_16, op_21, op_22 |
| **MEV-04-027** | P_PTYT | ✅ | ✅ | 6 | op_08, op_13, op_15, op_16, op_21, op_22 |
| **MEV-04-028** | P_PTYT | ✅ | ✅ | 6 | op_08, op_13, op_15, op_16, op_21, op_22 |
| **MEV-04-029** | P_YIELD | ✅ | ✅ | 6 | op_08, op_13, op_15, op_16, op_21, op_22 |
| **MEV-04-030** | P_YIELD | ✅ | ✅ | 6 | op_08, op_13, op_15, op_16, op_21, op_22 |
| **MEV-04-031** | OBSERVE | ✅ | ✅ | 3 | op_10, op_11, op_22 |

## Familia MEV-05 — cross-chain (14 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-05-001** | C_CEXDEX | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-002** | C_CEXDEX | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-003** | C_CEXDEX | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-004** | C_CEXDEX | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-005** | C_CEXDEX | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-006** | C_CEXDEX | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-007** | C_CEXDERIV | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-008** | C_CEXDERIV | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-009** | E_LATENCY | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-05-010** | E_LATENCY | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-05-011** | C_CEXDEX | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-012** | C_CEXDEX | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-013** | C_CEXDEX | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-05-014** | C_CEXDEX | ✅ | ✅ | 9 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_23, op_32 |

## Familia MEV-06 — triangular/holonomic (30 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-06-001** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-002** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-003** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-004** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-005** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-006** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-007** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-008** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-009** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-010** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-011** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-012** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-013** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-014** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-015** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-016** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-017** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-018** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-019** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-020** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-021** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-022** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-023** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-024** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-025** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-026** | X_BRIDGE | ✅ | ✅ | 8 | op_06, op_07, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-06-027** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-028** | X_PREPOS | ✅ | ✅ | 9 | op_06, op_07, op_08, op_11, op_16, op_22, op_23, op_27, op_32 |
| **MEV-06-029** | X_ORACLE | ✅ | ✅ | 8 | op_06, op_07, op_08, op_11, op_13, op_22, op_23, op_32 |
| **MEV-06-030** | X_ORACLE | ✅ | ✅ | 8 | op_06, op_07, op_08, op_11, op_13, op_22, op_23, op_32 |

## Familia MEV-07 — estadístico/latency (30 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-07-001** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-002** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-003** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-004** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-005** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-006** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-007** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-008** | D_FUNDING | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-07-009** | D_FUNDING | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_22, op_23, op_32 |
| **MEV-07-010** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-011** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-012** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-013** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-014** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-015** | D_BASIS | ✅ | ✅ | 8 | op_08, op_10, op_13, op_16, op_21, op_22, op_23, op_32 |
| **MEV-07-016** | D_OPTIONS_PARITY | ✅ | ✅ | 7 | op_08, op_11, op_13, op_19, op_21, op_22, op_32 |
| **MEV-07-017** | D_OPTIONS_PARITY | ✅ | ✅ | 7 | op_08, op_11, op_13, op_19, op_21, op_22, op_32 |
| **MEV-07-018** | D_OPTIONS_PARITY | ✅ | ✅ | 7 | op_08, op_11, op_13, op_19, op_21, op_22, op_32 |
| **MEV-07-019** | D_OPTIONS_PARITY | ✅ | ✅ | 7 | op_08, op_11, op_13, op_19, op_21, op_22, op_32 |
| **MEV-07-020** | D_OPTIONS_SURFACE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_14, op_19, op_22, op_32 |
| **MEV-07-021** | D_OPTIONS_SURFACE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_14, op_19, op_22, op_32 |
| **MEV-07-022** | D_OPTIONS_SURFACE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_14, op_19, op_22, op_32 |
| **MEV-07-023** | D_OPTIONS_SURFACE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_14, op_19, op_22, op_32 |
| **MEV-07-024** | D_OPTIONS_SURFACE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_14, op_19, op_22, op_32 |
| **MEV-07-025** | D_OPTIONS_SURFACE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_14, op_19, op_22, op_32 |
| **MEV-07-026** | D_SETTLE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_21, op_22, op_23, op_32 |
| **MEV-07-027** | D_SETTLE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_21, op_22, op_23, op_32 |
| **MEV-07-028** | D_SETTLE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_21, op_22, op_23, op_32 |
| **MEV-07-029** | D_SETTLE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_21, op_22, op_23, op_32 |
| **MEV-07-030** | D_SETTLE | ✅ | ✅ | 7 | op_08, op_11, op_13, op_21, op_22, op_23, op_32 |

## Familia MEV-08 — orderflow/toxicidad (25 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-08-001** | L_RATE | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_32 |
| **MEV-08-002** | L_RATE | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_32 |
| **MEV-08-003** | L_RATE | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_32 |
| **MEV-08-004** | L_RATE | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_32 |
| **MEV-08-005** | L_RATE | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_32 |
| **MEV-08-006** | L_COLLATERAL | ✅ | ✅ | 6 | op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-08-007** | L_COLLATERAL | ✅ | ✅ | 6 | op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-08-008** | L_COLLATERAL | ✅ | ✅ | 6 | op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-08-009** | L_RATE | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_32 |
| **MEV-08-010** | L_RATE | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_32 |
| **MEV-08-011** | L_LOOP | ✅ | ✅ | 7 | op_13, op_16, op_17, op_20, op_21, op_22, op_32 |
| **MEV-08-012** | L_LIQ | ✅ | ✅ | 9 | op_08, op_11, op_13, op_16, op_21, op_22, op_23, op_26, op_32 |
| **MEV-08-013** | L_LIQ | ✅ | ✅ | 9 | op_08, op_11, op_13, op_16, op_21, op_22, op_23, op_26, op_32 |
| **MEV-08-014** | L_LIQ | ✅ | ✅ | 9 | op_08, op_11, op_13, op_16, op_21, op_22, op_23, op_26, op_32 |
| **MEV-08-015** | L_LIQ | ✅ | ✅ | 9 | op_08, op_11, op_13, op_16, op_21, op_22, op_23, op_26, op_32 |
| **MEV-08-016** | L_LIQ | ✅ | ✅ | 9 | op_08, op_11, op_13, op_16, op_21, op_22, op_23, op_26, op_32 |
| **MEV-08-017** | L_LIQ | ✅ | ✅ | 9 | op_08, op_11, op_13, op_16, op_21, op_22, op_23, op_26, op_32 |
| **MEV-08-018** | L_AUCTION | ✅ | ✅ | 7 | op_08, op_11, op_19, op_21, op_22, op_23, op_32 |
| **MEV-08-019** | L_AUCTION | ✅ | ✅ | 7 | op_08, op_11, op_19, op_21, op_22, op_23, op_32 |
| **MEV-08-020** | L_AUCTION | ✅ | ✅ | 7 | op_08, op_11, op_19, op_21, op_22, op_23, op_32 |
| **MEV-08-021** | L_AUCTION | ✅ | ✅ | 7 | op_08, op_11, op_19, op_21, op_22, op_23, op_32 |
| **MEV-08-022** | L_COLLATERAL | ✅ | ✅ | 6 | op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-08-023** | L_COLLATERAL | ✅ | ✅ | 6 | op_08, op_11, op_13, op_21, op_22, op_32 |
| **MEV-08-024** | L_LIQ | ✅ | ✅ | 9 | op_08, op_11, op_13, op_16, op_21, op_22, op_23, op_26, op_32 |
| **MEV-08-025** | L_RATE | ✅ | ✅ | 8 | op_08, op_10, op_11, op_13, op_16, op_21, op_22, op_32 |

## Familia MEV-09 — otros (20 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-09-001** | I_ROUTE | ✅ | ✅ | 8 | op_19, op_20, op_22, op_23, op_24, op_27, op_29, op_32 |
| **MEV-09-002** | I_ROUTE | ✅ | ✅ | 8 | op_19, op_20, op_22, op_23, op_24, op_27, op_29, op_32 |
| **MEV-09-003** | I_ROUTE | ✅ | ✅ | 8 | op_19, op_20, op_22, op_23, op_24, op_27, op_29, op_32 |
| **MEV-09-004** | I_ROUTE | ✅ | ✅ | 8 | op_19, op_20, op_22, op_23, op_24, op_27, op_29, op_32 |
| **MEV-09-005** | I_BATCH | ✅ | ✅ | 7 | op_19, op_20, op_22, op_24, op_27, op_29, op_32 |
| **MEV-09-006** | I_BATCH | ✅ | ✅ | 7 | op_19, op_20, op_22, op_24, op_27, op_29, op_32 |
| **MEV-09-007** | I_BATCH | ✅ | ✅ | 7 | op_19, op_20, op_22, op_24, op_27, op_29, op_32 |
| **MEV-09-008** | I_BATCH | ✅ | ✅ | 7 | op_19, op_20, op_22, op_24, op_27, op_29, op_32 |
| **MEV-09-009** | I_DUTCH | ✅ | ✅ | 7 | op_08, op_11, op_21, op_22, op_23, op_24, op_32 |
| **MEV-09-010** | I_ORDERFLOW | ✅ | ✅ | 7 | op_11, op_22, op_23, op_24, op_25, op_27, op_32 |
| **MEV-09-011** | I_ORDERFLOW | ✅ | ✅ | 7 | op_11, op_22, op_23, op_24, op_25, op_27, op_32 |
| **MEV-09-012** | I_ROUTE | ✅ | ✅ | 8 | op_19, op_20, op_22, op_23, op_24, op_27, op_29, op_32 |
| **MEV-09-013** | I_ROUTE | ✅ | ✅ | 8 | op_19, op_20, op_22, op_23, op_24, op_27, op_29, op_32 |
| **MEV-09-014** | I_ROUTE | ✅ | ✅ | 8 | op_19, op_20, op_22, op_23, op_24, op_27, op_29, op_32 |
| **MEV-09-015** | I_ROUTE | ✅ | ✅ | 8 | op_19, op_20, op_22, op_23, op_24, op_27, op_29, op_32 |
| **MEV-09-016** | I_ROUTE | ✅ | ✅ | 8 | op_19, op_20, op_22, op_23, op_24, op_27, op_29, op_32 |
| **MEV-09-017** | I_ORDERFLOW | ✅ | ✅ | 7 | op_11, op_22, op_23, op_24, op_25, op_27, op_32 |
| **MEV-09-018** | I_ORDERFLOW | ✅ | ✅ | 7 | op_11, op_22, op_23, op_24, op_25, op_27, op_32 |
| **MEV-09-019** | OBSERVE | ✅ | ✅ | 3 | op_10, op_11, op_22 |
| **MEV-09-020** | OBSERVE | ✅ | ✅ | 3 | op_10, op_11, op_22 |

## Familia MEV-10 — otros-II (18 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-10-001** | N_IDENTICAL | ✅ | ✅ | 6 | op_08, op_10, op_11, op_21, op_22, op_23 |
| **MEV-10-002** | N_FLOOR | ✅ | ✅ | 5 | op_08, op_10, op_11, op_13, op_22 |
| **MEV-10-003** | N_FLOOR | ✅ | ✅ | 5 | op_08, op_10, op_11, op_13, op_22 |
| **MEV-10-004** | N_FLOOR | ✅ | ✅ | 5 | op_08, op_10, op_11, op_13, op_22 |
| **MEV-10-005** | N_IDENTICAL | ✅ | ✅ | 6 | op_08, op_10, op_11, op_21, op_22, op_23 |
| **MEV-10-006** | N_AMM | ✅ | ✅ | 5 | op_08, op_10, op_15, op_21, op_22 |
| **MEV-10-007** | N_AMM | ✅ | ✅ | 5 | op_08, op_10, op_15, op_21, op_22 |
| **MEV-10-008** | N_IDENTICAL | ✅ | ✅ | 6 | op_08, op_10, op_11, op_21, op_22, op_23 |
| **MEV-10-009** | N_FLOOR | ✅ | ✅ | 5 | op_08, op_10, op_11, op_13, op_22 |
| **MEV-10-010** | N_REDEEM | ✅ | ✅ | 6 | op_08, op_11, op_13, op_19, op_21, op_22 |
| **MEV-10-011** | N_REDEEM | ✅ | ✅ | 6 | op_08, op_11, op_13, op_19, op_21, op_22 |
| **MEV-10-012** | N_LIQ | ✅ | ✅ | 6 | op_08, op_11, op_16, op_21, op_22, op_23 |
| **MEV-10-013** | N_FLOOR | ✅ | ✅ | 5 | op_08, op_10, op_11, op_13, op_22 |
| **MEV-10-014** | N_IDENTICAL | ✅ | ✅ | 6 | op_08, op_10, op_11, op_21, op_22, op_23 |
| **MEV-10-015** | N_IDENTICAL | ✅ | ✅ | 6 | op_08, op_10, op_11, op_21, op_22, op_23 |
| **MEV-10-016** | N_IDENTICAL | ✅ | ✅ | 6 | op_08, op_10, op_11, op_21, op_22, op_23 |
| **MEV-10-017** | N_REDEEM | ✅ | ✅ | 6 | op_08, op_11, op_13, op_19, op_21, op_22 |
| **MEV-10-018** | N_REDEEM | ✅ | ✅ | 6 | op_08, op_11, op_13, op_19, op_21, op_22 |

## Familia MEV-11 — otros-III (12 estrategias)

| Estrategia | Detector | Rhai | Excel | #Ops | Operadores |
|---|---|---|---|---|---|
| **MEV-11-001** | M_COMPLETE | ✅ | ✅ | 6 | op_08, op_11, op_14, op_19, op_21, op_22 |
| **MEV-11-002** | M_COMPLETE | ✅ | ✅ | 6 | op_08, op_11, op_14, op_19, op_21, op_22 |
| **MEV-11-003** | M_CROSS | ✅ | ✅ | 6 | op_08, op_11, op_13, op_14, op_22, op_23 |
| **MEV-11-004** | M_LOGIC | ✅ | ✅ | 5 | op_11, op_13, op_14, op_19, op_22 |
| **MEV-11-005** | M_COMPLETE | ✅ | ✅ | 6 | op_08, op_11, op_14, op_19, op_21, op_22 |
| **MEV-11-006** | M_LOGIC | ✅ | ✅ | 5 | op_11, op_13, op_14, op_19, op_22 |
| **MEV-11-007** | M_LOGIC | ✅ | ✅ | 5 | op_11, op_13, op_14, op_19, op_22 |
| **MEV-11-008** | M_LOGIC | ✅ | ✅ | 5 | op_11, op_13, op_14, op_19, op_22 |
| **MEV-11-009** | OBSERVE | ✅ | ✅ | 3 | op_10, op_11, op_22 |
| **MEV-11-010** | OBSERVE | ✅ | ✅ | 3 | op_10, op_11, op_22 |
| **MEV-11-011** | OBSERVE | ✅ | ✅ | 3 | op_10, op_11, op_22 |
| **MEV-11-012** | M_AMM | ✅ | ✅ | 5 | op_08, op_11, op_19, op_21, op_22 |

**TOTAL: 264 estrategias individuales en 11 familias.**
