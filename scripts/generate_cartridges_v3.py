#!/usr/bin/env python3
"""
ArbitrageX v2 — Cartridge Generator v3 (Universal Cartridge Contract).

Reads the canonical Excel sheet `02_CARTRIDGE_MATH_MAP` (264 strategies) and
generates one `.rhai` cartridge per MEV_ID that satisfies the Universal
Cartridge Contract enforced by `backend/searcher-rs/src/cartridge/contract.rs`:

    fn init_strategy()                 -> Map   (metadata, machine-readable)
    fn evaluate_opportunity(pool_data) -> Map   (CartridgeEvalResult keys)
    fn build_payload(opportunity)      -> Map   (CartridgePayload keys)

Doctrinal anchors (from `00_CARTRIDGE_ARCH` / `08_CONFLICTS` / RULE 00 / R8):
  * Detector != operator. The cartridge evaluates the *economic detector* for
    its family (e.g. R_CLOSED_CYCLE: Π_R(x) = Q_R(x) - x - C_R(x), opportunity
    iff max_x Π_R(x) > 0). It NEVER reimplements op_01..op_31 (SVD/Kalman/
    Kelly/Newton...). Operator output is referenced, when available, via the
    `get_math_evidence` host binding as *evidence/confidence only*.
  * Fail-honest: missing reserves / decimals / quote / oracle / external feed
    => is_opportunity=false + explicit `reason`. NEVER fabricate a number.
  * Zero-Mocks: no hardcoded pools/tokens/prices. All state comes from the
    CartridgeContextV3 (`pool_data.route[]`, full route legs) + host bindings
    (`get_reserves`, `get_token_meta`, `get_v3_slot0`, ...).
  * No arbitrary weights: the Excel resolves the 264×31 weights to 0 pending
    PAPER/SHADOW calibration, so `confidence` is derived from data
    completeness / profit margin, never from a made-up operator weight.

Execution-class handling (from `03_DETECTOR_FAMILIES`):
  * DETERMINISTIC_EXECUTABLE (on-chain closed route): real candidacy. Compose
    Q_R(x) by exact CPMM (x·y=k) composition across the route legs, then
    golden-section search for x* (amount optimisation lives in the cartridge
    as the *detector's* amount hint; SizeOptimizer/sim remain the net-profit
    authority downstream).
  * Everything else (EXTERNAL_DATA_REQUIRED, DERIVATIVE_DATA_REQUIRED,
    POST_STATE, SETTLEMENT, AUTHORIZED_FLOW, NONATOMIC_*, OBSERVE_ONLY, ...):
    fail-honest observe-only. The cartridge computes whatever on-chain
    evidence it can, but is_opportunity stays false with an explicit reason
    until its required external/settlement data binding exists. This is the
    doctrinally-correct behaviour per RULE 00 / R8 — no fabricated profit.

Usage:
    python3 scripts/generate_cartridges_v3.py [excel_path] [output_dir]

Defaults:
    excel_path = repo-vps-audits/canonical_264x31.xlsx (or first .xlsx found)
    output_dir = backend/searcher-rs/cartridges/strategies
"""

import os
import re
import sys

try:
    import openpyxl
except ImportError:
    sys.stderr.write("openpyxl is required: pip install openpyxl\n")
    raise

SHEET = "02_CARTRIDGE_MATH_MAP"

# ─── Column indices (0-based) in 02_CARTRIDGE_MATH_MAP ───────────────────────
C_MEV_ID = 0
C_GRUPO = 1
C_ESTRATEGIA = 2
C_MODULO = 4
C_LEGS_MIN = 6
C_LEGS_MAX = 7
C_DETERMINISMO = 9
C_DETECTOR_ID = 14
C_CLASE_EJEC = 16
C_ECUCION = 17
C_OPS_PRIMARY = 18
C_OPS_SECONDARY = 19
C_CONFIG = 21
C_NOTA = 22

OP_RE = re.compile(r"op_(\d{2})")


def parse_ops(cell):
    """Extract ordered unique operator ids (ints) from a free-text cell."""
    if not cell:
        return []
    seen, out = set(), []
    for m in OP_RE.finditer(str(cell)):
        n = int(m.group(1))
        if n not in seen:
            seen.add(n)
            out.append(n)
    return out


def slug(mev_id, name):
    s = re.sub(r"[^a-z0-9]+", "_", str(name).lower()).strip("_")
    s = re.sub(r"_+", "_", s)
    return f"{mev_id.lower().replace('-', '_')}_{s}"


def toml_str(s):
    return str(s).replace("\\", "\\\\").replace('"', '\\"')


# Execution classes that produce a real, on-chain, atomically-executable closed
# route candidate (given on-chain reserves only). Everything else observe-only.
EXECUTABLE_CLASSES = {"DETERMINISTIC_EXECUTABLE"}

# Detector families whose economics are a closed on-chain route composition —
# these get the real CPMM Q_R(x) detector. CF_* families are single-venue
# invariant variants that still resolve to a closed route, so they share it.
ROUTE_COMPOSITION_DETECTORS = {
    "R_CLOSED_CYCLE",
    "R_DIRECT_INDIRECT",
    "R_SPLIT",
    "CF_CPMM",
    "CF_CONSTANT_SUM",
    "CF_CROSSINV",
    "CF_WEIGHTED",
    "CF_STABLESWAP",
    "CF_CLAMM",
    "CF_LB",
}


def reason_for(clase):
    """Map execution class -> explicit fail-honest observe reason (R8)."""
    return {
        "DETERMINISTIC_EXECUTABLE": "no_profitable_closed_route",
        "EXTERNAL_DATA_REQUIRED": "external_feed_unavailable",
        "DERIVATIVE_DATA_REQUIRED": "derivative_data_unavailable",
        "EXTERNAL_SETTLEMENT_REQUIRED": "external_settlement_unavailable",
        "DETERMINISTIC_POST_STATE": "post_state_unavailable",
        "DETERMINISTIC_POST_ORACLE": "post_oracle_state_unavailable",
        "DETERMINISTIC_SETTLEMENT": "settlement_matching_unavailable",
        "DETERMINISTIC_AUCTION": "auction_state_unavailable",
        "DETERMINISTIC_LIQUIDATION": "liquidation_state_unavailable",
        "DETERMINISTIC_IF_REDEEMABLE": "redemption_path_unavailable",
        "DETERMINISTIC_IF_CONVERTIBLE": "conversion_path_unavailable",
        "DETERMINISTIC_IF_SETTLEABLE": "settlement_path_unavailable",
        "DETERMINISTIC_IF_POSITIONS": "position_state_unavailable",
        "DETERMINISTIC_IF_FIRM_BID": "firm_bid_unavailable",
        "DETERMINISTIC_IF_FIRM_EXIT": "firm_exit_unavailable",
        "DETERMINISTIC_IF_COMPLETE_SET": "complete_set_unavailable",
        "DETERMINISTIC_IF_PAYOFF_MODEL": "payoff_model_unavailable",
        "DETERMINISTIC_IF_MATCHED_CLAIM": "matched_claim_unavailable",
        "DETERMINISTIC_IF_ADAPTER": "protocol_adapter_unavailable",
        "DETERMINISTIC_WITH_ORACLE": "oracle_unavailable",
        "DETERMINISTIC_WITH_DERIVATIVE_STATE": "derivative_state_unavailable",
        "DETERMINISTIC_POSITION_STRATEGY": "position_state_unavailable",
        "SETTLEMENT_DELAY_SENSITIVE": "settlement_delay_unavailable",
        "NONATOMIC_BRIDGE_REQUIRED": "bridge_settlement_unavailable",
        "NONATOMIC_INVENTORY_REQUIRED": "inventory_state_unavailable",
        "AUTHORIZED_FLOW_ONLY": "authorized_flow_unavailable",
        "LATENCY_SENSITIVE": "firm_quote_unavailable",
        "SIGNAL_UNLESS_FIRM_EXIT": "signal_only_no_firm_exit",
        "OBSERVE_ONLY": "observe_only",
    }.get(clase, "required_data_unavailable")


# ─── Rhai body templates ──────────────────────────────────────────────────────
#
# All templates share the Universal Cartridge Contract. The executable closed-
# route detector composes Q_R(x) exactly; observe-only detectors fail-honest.
#
# NOTE on Rhai: no `math_ln` binding exists — the natural log binding is
# `math_log`. The marginal prefilter Σ_e[-ln((1-fee_e)·rate_e)] uses math_log.


def render_executable(m, ops_primary, ops_secondary):
    """R_CLOSED_CYCLE / route-composition detector — REAL candidacy.

    Composes Q_R(x) by exact CPMM (x·y=k, fee γ per leg) across the full route
    from CartridgeContextV3 (`pool_data.route[]`), then golden-section search
    for the amount x* maximising Π_R(x) = Q_R(x) - x - gas. Fail-honest at
    every step. estimated_profit is a GROSS pre-sim amount hint in the route's
    input token; SizeOptimizer + REVM sim remain the net-profit authority.
    """
    ops_p = ", ".join(str(o) for o in ops_primary)
    ops_s = ", ".join(str(o) for o in ops_secondary)
    detector = m["detector_id"]
    return f'''// {m["mev_id"]} — {m["estrategia"]}
// Detector family: {detector} ({m["clase"]})
// Generated by scripts/generate_cartridges_v3.py from the canonical Excel
// (02_CARTRIDGE_MATH_MAP). Universal Cartridge Contract v3.
//
// Detector (economic, NOT an op_XX operator):
//   Q_R(x) = q_n(...q_2(q_1(x)))   exact per-leg composition
//   Π_R(x) = Q_R(x) - x - C_R(x)   opportunity iff max_x Π_R(x) > 0
//   Marginal prefilter: Σ_e[-ln((1-fee_e)·rate_e)] < 0
// Primary operators (evidence only, math-engine): [{ops_p}]
// Secondary operators (evidence only): [{ops_s}]
//
// Zero-Mocks / Fail-Honest: all state from CartridgeContextV3 route[] + host
// bindings. Missing reserves/decimals => is_opportunity:false + reason.
//
// NOTE: the runner invokes evaluate_opportunity with an EMPTY scope, so Rhai
// global `const`s are NOT visible inside functions. All config is inlined as
// literals in each function below. (Fee default 0.30% = 30 bps -> γ=0.997;
// coarse gas precheck GAS_HINT_USD=5.0; authoritative net gate is downstream.)

fn init_strategy() {{
    #{{
        name: "{toml_str(m["estrategia"])}",
        version: "3.0.0",
        author: "arbx-cartridge-generator-v3",
        description: "{toml_str(m["nota"] or m["estrategia"])}",
        category: "{m["modulo"]}",
        mev_id: "{m["mev_id"]}",
        detector_id: "{detector}",
        execution_class: "{m["clase"]}",
        min_legs: {m["legs_min"]},
        max_legs: {m["legs_max"]},
        primary_operators: [{ops_p}],
        secondary_operators: [{ops_s}],
        triggers: ["new_block", "swap", "reserve_update"],
        required_bindings: ["get_reserves", "get_token_meta"],
        target_chains: [],
        min_eval_interval_ms: 100,
        config_schema: #{{
            enabled: #{{ "type": "bool", "default": true, "editable": true }},
            min_profit_usd: #{{ "type": "number", "default": 0.0, "editable": true }},
            max_slippage_pct: #{{ "type": "number", "default": 1.0, "editable": true }},
            max_price_impact_pct: #{{ "type": "number", "default": 5.0, "editable": true }},
            max_gas_usd: #{{ "type": "number", "default": 50.0, "editable": true }}
        }}
    }}
}}

// exact-in CPMM quote for one leg: Δy = r1·(γ·Δx) / (r0 + γ·Δx)
// r0 = reserve of token_in, r1 = reserve of token_out (same decimal base units).
fn cpmm_out(x, r_in, r_out, fee_bps) {{
    let gamma = (10000.0 - fee_bps.to_float()) / 10000.0;
    let dxg = gamma * x;
    (r_out * dxg) / (r_in + dxg)
}}

fn evaluate_opportunity(pool_data) {{
    let route = pool_data.route;
    if route == () {{ return no_opp("missing_route"); }}
    let n = route.len();
    if n < {m["legs_min"]} || n > {m["legs_max"]} {{ return no_opp("route_shape_out_of_bounds"); }}
    if pool_data.route_closed != true {{ return no_opp("route_not_closed"); }}

    // ── Gather exact per-leg state (fail-honest) ────────────────────────────
    let rin = [];   // reserve of token_in per leg (float, base units)
    let rout = [];  // reserve of token_out per leg
    let fees = [];  // fee_bps per leg
    let token_in0 = "";
    let token_out_last = "";
    let dec0 = -1;
    for i in 0..n {{
        let leg = route[i];
        let pool = leg.pool;
        if pool == () || pool == "" {{ return no_opp("missing_pool_address"); }}
        let res = get_reserves(pool);
        if res == () {{ return no_opp("missing_reserves"); }}
        let r0 = res.r0.to_float();
        let r1 = res.r1.to_float();
        if r0 <= 0.0 || r1 <= 0.0 {{ return no_opp("missing_reserves"); }}
        // Orient reserves by token_in. The pool stores token0/token1; we map
        // using the leg's token_in vs the reserve's token0_addr when present.
        let ri = r0;
        let ro = r1;
        if res.token0_addr != () && leg.token_in != () {{
            let t0 = res.token0_addr;
            if leg.token_in != t0 {{ ri = r1; ro = r0; }}
        }}
        rin.push(ri);
        rout.push(ro);
        let f = if leg.fee_bps != () {{ leg.fee_bps }} else {{ 30 }};
        fees.push(f);
        if i == 0 {{ token_in0 = leg.token_in; }}
        if i == n - 1 {{ token_out_last = leg.token_out; }}
    }}
    // A closed cycle must return to its start token.
    if token_out_last != token_in0 {{ return no_opp("route_not_closed"); }}

    // Decimals of the input token for a sane sizing bracket (fail-honest).
    let tm = get_token_meta(token_in0);
    if tm == () {{ return no_opp("missing_token_meta"); }}
    dec0 = tm.decimals;
    let base_unit = math_pow(10.0, dec0.to_float());

    // ── Marginal prefilter: Σ_e[-ln((1-fee_e)·rate_e)] < 0 ─────────────────
    // rate_e = rout/rin (spot). If the product of fee-adjusted rates <= 1 the
    // cycle cannot be profitable at any size; skip sizing work.
    let log_sum = 0.0;
    for i in 0..n {{
        let rate = rout[i] / rin[i];
        let adj = (1.0 - fees[i].to_float() / 10000.0) * rate;
        if adj <= 0.0 {{ return no_opp("degenerate_rate"); }}
        log_sum += -math_log(adj);
    }}
    if log_sum >= 0.0 {{ return no_opp("prefilter_non_positive"); }}

    // ── Compose Q_R(x) and maximise Π_R(x) via golden-section ──────────────
    // Bracket x in [x_lo, x_hi] input-token units (conservative vs leg 0 depth).
    let x_lo = base_unit * 0.01;
    let x_hi = math_max(base_unit * 1.0, rin[0] * 0.05);

    let gr = 0.6180339887498949;  // golden ratio conjugate (φ-1)
    let a = x_lo;
    let b = x_hi;
    let c = b - gr * (b - a);
    let d = a + gr * (b - a);
    let fc = profit_at(c, rin, rout, fees);
    let fd = profit_at(d, rin, rout, fees);
    for _it in 0..40 {{
        if fc < fd {{ a = c; c = d; fc = fd; d = a + gr * (b - a); fd = profit_at(d, rin, rout, fees); }}
        else        {{ b = d; d = c; fd = fc; c = b - gr * (b - a); fc = profit_at(c, rin, rout, fees); }}
    }}
    let x_star = (a + b) / 2.0;
    let profit_tok = profit_at(x_star, rin, rout, fees);

    if profit_tok <= 0.0 {{ return no_opp("impact_zero"); }}

    // Gross profit hint in USD for the net gate precheck (fail-honest on price).
    let px = get_token_price_usd(token_in0);
    let profit_usd = 0.0;
    if px != () && px > 0.0 {{
        profit_usd = (profit_tok / base_unit) * px;
    }}
    if profit_usd > 0.0 && profit_usd < 5.0 {{ return no_opp("below_gas_threshold"); }}

    // Optional operator evidence (confidence modulation only; never required,
    // never fabricated). Absent evidence leaves a neutral completeness-based
    // confidence derived from data availability.
    let conf = 0.7;  // full on-chain data present + profitable prefilter
    let ev = get_math_evidence("{m["mev_id"]}");
    if ev != () && ev.confidence != () {{
        conf = math_max(0.0, math_min(1.0, (conf + ev.confidence) / 2.0));
    }}

    #{{
        is_opportunity: true,
        estimated_profit: profit_tok / base_unit,   // gross, input token units
        confidence: conf,
        urgency: "immediate",
        reason: "closed_cycle_profit",
        detector_id: "{detector}",
        mev_id: "{m["mev_id"]}",
        optimal_amount_in: x_star / base_unit,
        legs: n,
        profit_usd_hint: profit_usd
    }}
}}

// Π_R(x) in input-token units: Q_R(x) - x (gas handled by the USD precheck).
fn profit_at(x, rin, rout, fees) {{
    let amt = x;
    for i in 0..rin.len() {{
        amt = cpmm_out(amt, rin[i], rout[i], fees[i]);
    }}
    amt - x
}}

fn no_opp(reason) {{
    #{{
        is_opportunity: false,
        estimated_profit: 0.0,
        confidence: 0.0,
        urgency: "monitor",
        reason: reason,
        detector_id: "{detector}",
        mev_id: "{m["mev_id"]}"
    }}
}}

fn build_payload(opportunity) {{
    // Payload assembly is the executor's concern downstream; the cartridge
    // declares its intent only. No signer, no broadcast (§32/§33 read-only).
    #{{
        target_contract: "0x0000000000000000000000000000000000000000",
        calldata: "0x",
        value_wei: "0",
        gas_limit: 350000,
        max_priority_fee_gwei: 0.0,
        deadline_ts: 0,
        mev_id: "{m["mev_id"]}",
        detector_id: "{detector}"
    }}
}}
'''


def render_observe(m, ops_primary, ops_secondary):
    """Observe-only detector — fail-honest (RULE 00 / R8).

    Computes no fabricated profit. Emits is_opportunity=false with an explicit
    reason naming the missing data/settlement binding for its execution class.
    """
    ops_p = ", ".join(str(o) for o in ops_primary)
    ops_s = ", ".join(str(o) for o in ops_secondary)
    detector = m["detector_id"]
    reason = reason_for(m["clase"])
    return f'''// {m["mev_id"]} — {m["estrategia"]}
// Detector family: {detector} ({m["clase"]})
// Generated by scripts/generate_cartridges_v3.py from the canonical Excel
// (02_CARTRIDGE_MATH_MAP). Universal Cartridge Contract v3.
//
// This detector requires {m["clase"]} data/settlement that is not yet wired to
// a real binding. Per RULE 00 / R8 it is OBSERVE-ONLY: is_opportunity stays
// false with an explicit reason. It NEVER fabricates a profit number.
// Detector equation (reference, evaluated downstream once data exists):
//   {str(m["ecuacion"])[:200]}
// Primary operators (evidence only): [{ops_p}]   Secondary: [{ops_s}]
//
// NOTE: runner invokes functions with an EMPTY scope -> global consts are not
// visible inside functions; all values inlined below.

fn init_strategy() {{
    #{{
        name: "{toml_str(m["estrategia"])}",
        version: "3.0.0",
        author: "arbx-cartridge-generator-v3",
        description: "{toml_str(m["nota"] or m["estrategia"])}",
        category: "{m["modulo"]}",
        mev_id: "{m["mev_id"]}",
        detector_id: "{detector}",
        execution_class: "{m["clase"]}",
        min_legs: {m["legs_min"]},
        max_legs: {m["legs_max"]},
        primary_operators: [{ops_p}],
        secondary_operators: [{ops_s}],
        triggers: ["new_block"],
        required_bindings: [],
        target_chains: [],
        min_eval_interval_ms: 250,
        config_schema: #{{
            enabled: #{{ "type": "bool", "default": true, "editable": true }}
        }}
    }}
}}

fn evaluate_opportunity(pool_data) {{
    // Fail-honest: no executable settlement / external data binding available.
    #{{
        is_opportunity: false,
        estimated_profit: 0.0,
        confidence: 0.0,
        urgency: "monitor",
        reason: "{reason}",
        detector_id: "{detector}",
        mev_id: "{m["mev_id"]}"
    }}
}}

fn build_payload(opportunity) {{
    #{{
        target_contract: "0x0000000000000000000000000000000000000000",
        calldata: "0x",
        value_wei: "0",
        gas_limit: 0,
        max_priority_fee_gwei: 0.0,
        deadline_ts: 0,
        mev_id: "{m["mev_id"]}",
        detector_id: "{detector}"
    }}
}}
'''


def read_rows(excel_path):
    wb = openpyxl.load_workbook(excel_path, data_only=True)
    ws = wb[SHEET]
    rows = []
    for i, row in enumerate(ws.iter_rows(values_only=True)):
        if i == 0:
            continue  # header
        mev = row[C_MEV_ID]
        if not mev:
            continue
        rows.append({
            "mev_id": str(mev).strip(),
            "grupo": row[C_GRUPO],
            "estrategia": row[C_ESTRATEGIA] or mev,
            "modulo": (row[C_MODULO] or "route_graph_engine"),
            "legs_min": int(row[C_LEGS_MIN] or 2),
            "legs_max": int(row[C_LEGS_MAX] or 8),
            "determinismo": row[C_DETERMINISMO] or "",
            "detector_id": str(row[C_DETECTOR_ID] or "OBSERVE").strip(),
            "clase": str(row[C_CLASE_EJEC] or "OBSERVE_ONLY").strip(),
            "ecuacion": row[C_ECUCION] or "",
            "ops_primary": parse_ops(row[C_OPS_PRIMARY]),
            "ops_secondary": parse_ops(row[C_OPS_SECONDARY]),
            "config": row[C_CONFIG] or "",
            "nota": row[C_NOTA] or "",
        })
    return rows


def main():
    repo = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    excel_path = sys.argv[1] if len(sys.argv) > 1 else None
    out_dir = sys.argv[2] if len(sys.argv) > 2 else os.path.join(
        repo, "backend", "searcher-rs", "cartridges", "strategies")

    if not excel_path:
        # discover a canonical xlsx under Downloads or repo
        cands = [os.path.join(repo, "repo-vps-audits", "canonical_264x31.xlsx")]
        dl = os.path.expanduser("~/Downloads")
        if os.path.isdir(dl):
            for f in sorted(os.listdir(dl)):
                if f.endswith(".xlsx") and "264" in f:
                    cands.append(os.path.join(dl, f))
        excel_path = next((c for c in cands if os.path.exists(c)), None)
    if not excel_path or not os.path.exists(excel_path):
        sys.stderr.write("Excel not found. Pass path explicitly.\n")
        sys.exit(2)

    rows = read_rows(excel_path)
    os.makedirs(out_dir, exist_ok=True)

    written, skipped = 0, 0
    for m in rows:
        body = (render_executable if (m["clase"] in EXECUTABLE_CLASSES and
                                      m["detector_id"] in ROUTE_COMPOSITION_DETECTORS)
                else render_observe)(m, m["ops_primary"], m["ops_secondary"])
        fname = slug(m["mev_id"], m["estrategia"]) + ".rhai"
        with open(os.path.join(out_dir, fname), "w", encoding="utf-8") as fh:
            fh.write(body)
        written += 1

    print(f"generated {written} cartridges from {len(rows)} excel rows -> {out_dir}")


if __name__ == "__main__":
    main()
