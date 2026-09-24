"""WO-PC9: exact card valuations + strict completeness; PriceBus is the ONLY oracle.

Consumes an explicit, immutable PriceBus export and event-bound quote evidence.
Does NOT connect to exchanges, change config, authorize trades or invent liquidity,
simulation results, risk scores, prices or fee values. Test fixtures are NOT live data.
"""
from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal, localcontext
import re
from typing import Any

from .integrity import HASH, BLOCK_HASH, digest, canonical_bytes, validate_envelope
from .math_reference import decimal, uint

ADDRESS = re.compile(r"^0x[0-9a-fA-F]{40}$")
COST_NAMES = ("gas", "lp_fees", "slippage", "flash_fee", "relay_fee", "capital_cost", "failure_buffer", "ops_overhead")


def exact(value: Any) -> Decimal:
    if not isinstance(value, str) or len(value) > 160 or not re.fullmatch(r"-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?", value):
        raise ValueError("exact_decimal_string_required")
    return decimal(value)


def text(value: Decimal) -> str:
    if not value.is_finite():
        raise ValueError("non_finite")
    out = format(value, "f")
    if "." in out:
        out = out.rstrip("0").rstrip(".")
    return "0" if out in {"", "-0"} else out


def asset_key(chain_id: int, address: str) -> str:
    if isinstance(chain_id, bool) or not isinstance(chain_id, int) or chain_id <= 0 or not ADDRESS.fullmatch(address):
        raise ValueError("invalid_chain_asset_identity")
    return f"{chain_id}:{address.lower()}"


def positive_time(value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise ValueError("invalid_timestamp")
    return value


@dataclass(frozen=True)
class CardPolicy:
    """All policy limits are caller supplied, never installed as trading defaults."""
    now_ms: int
    quote_max_age_ms: int
    snapshot_max_age_ms: int
    max_clock_skew_ms: int
    max_hops: int
    required_fields: tuple[str, ...]
    na_reasons: dict[str, tuple[str, ...]]

    def validate(self) -> None:
        positive_time(self.now_ms)
        if any(isinstance(x, bool) or not isinstance(x, int) or x < 0 for x in
               (self.quote_max_age_ms, self.snapshot_max_age_ms, self.max_clock_skew_ms)):
            raise ValueError("invalid_policy_time_budget")
        if not 2 <= self.max_hops <= 64:
            raise ValueError("invalid_hop_budget")
        if not self.required_fields or len(set(self.required_fields)) != len(self.required_fields):
            raise ValueError("explicit_unique_required_fields_needed")


class CanonicalPriceView:
    """Strict consumer of arbx.pricebus.export.v1, NOT another price source.

    Exporter must preserve source observation times, quote currency conversion,
    AssetKey, source references and hashes. A bare {symbol: price} is insufficient.
    Self-consistent hashes do not attest the source's truth.
    """
    def __init__(self, snapshot: dict[str, Any], policy: CardPolicy):
        policy.validate()
        if snapshot.get("schema") != "arbx.pricebus.export.v1" or snapshot.get("source") != "canonical_pricebus":
            raise ValueError("canonical_pricebus_export_required")
        body = {k: v for k, v in snapshot.items() if k != "snapshot_hash"}
        if snapshot.get("snapshot_hash") != digest(body):
            raise ValueError("pricebus_snapshot_hash_mismatch")
        if not HASH.fullmatch(str(snapshot.get("policy_hash", ""))):
            raise ValueError("pricebus_policy_provenance_missing")
        generated = positive_time(snapshot.get("generated_at_ms"))
        if generated > policy.now_ms + policy.max_clock_skew_ms or policy.now_ms - generated > policy.snapshot_max_age_ms:
            raise ValueError("pricebus_snapshot_stale_or_future")
        if not isinstance(snapshot.get("prices"), dict):
            raise ValueError("pricebus_prices_missing")
        # Snapshot is copied through the canonical profile; caller mutations cannot
        # make two card fields use different versions after the atomic read.
        import json
        self.snapshot = json.loads(canonical_bytes(snapshot))
        self.policy = policy

    def price(self, asset: str) -> tuple[Decimal, dict[str, Any]]:
        record = self.snapshot["prices"].get(asset)
        if not isinstance(record, dict):
            raise ValueError("pricebus_asset_not_covered")
        if record.get("asset_key") != asset:
            raise ValueError("pricebus_asset_key_mismatch")
        verdict = record.get("verdict")
        if verdict in {"no_live_price", "price_divergence_binance_chainlink", "frozen"}:
            raise ValueError(str(verdict))
        if verdict not in {"ok", "anchor_only", "stale_binance"}:
            # stale_anchor is a flagged unverified quote, not a verified USD price.
            raise ValueError("pricebus_anchor_not_verified")
        if record.get("currency") != "USD" or record.get("purpose") != "valuation":
            raise ValueError("pricebus_unit_or_purpose_mismatch")
        observed = positive_time(record.get("observed_at_ms"))
        expires = positive_time(record.get("valid_until_ms"))
        if expires < observed or observed > self.policy.now_ms + self.policy.max_clock_skew_ms or expires < self.policy.now_ms:
            raise ValueError("pricebus_price_stale_or_future")
        refs = record.get("source_references")
        if not isinstance(refs, list) or not refs or any(not isinstance(r, str) or not r for r in refs):
            raise ValueError("pricebus_source_reference_missing")
        hashes = record.get("input_hashes")
        if not isinstance(hashes, list) or not hashes or any(not HASH.fullmatch(str(h)) for h in hashes):
            raise ValueError("pricebus_raw_evidence_missing")
        p = exact(record.get("price_usd"))
        if p <= 0:
            raise ValueError("pricebus_non_positive_price")
        return p, record


def strict_card_audit(envelope: dict[str, Any], policy: CardPolicy) -> dict[str, Any]:
    """Missing applicable data => repair failure, NEVER success with a placeholder.

    NOT_APPLICABLE requires a field-specific reason from the versioned strategy
    applicability contract. Unknown fields remain visible and must pass schema
    validation too; completeness does not mean just the currently rendered cells.
    """
    policy.validate()
    problems: list[dict[str, str]] = [{"field": "$envelope", "reason": e} for e in validate_envelope(envelope)]
    fields = envelope.get("fields", {})
    if not isinstance(fields, dict):
        fields = {}
    for name in dict.fromkeys((*policy.required_fields, *fields.keys())):
        f = fields.get(name)
        if not isinstance(f, dict):
            problems.append({"field": name, "reason": "applicable_field_missing"}); continue
        state = f.get("state")
        if state == "not_applicable":
            if f.get("reason") not in policy.na_reasons.get(name, ()):
                problems.append({"field": name, "reason": "not_applicable_without_strategy_rule"})
            continue
        if state not in {"observed", "computed"}:
            problems.append({"field": name, "reason": f"applicable_field_{state}"}); continue
        val = f.get("value")
        if isinstance(val, str) and val.strip().lower() in {"", "—", "-", "no computado", "n/a", "nan", "infinity"}:
            problems.append({"field": name, "reason": "placeholder_not_a_computed_value"})
        if f.get("unit") in {"USD", "percent", "bps", "probability", "ratio"}:
            try:
                number = exact(val)
                if f["unit"] == "probability" and not Decimal(0) <= number <= Decimal(1):
                    raise ValueError("probability_out_of_range")
            except (ValueError, TypeError):
                problems.append({"field": name, "reason": "numeric_unit_requires_exact_valid_decimal"})
        if f.get("context_id") != envelope.get("context_id"):
            problems.append({"field": name, "reason": "cross_context_value"})
        expiry = f.get("valid_until_ms")
        if isinstance(expiry, bool) or not isinstance(expiry, int) or expiry < policy.now_ms:
            problems.append({"field": name, "reason": "value_expired_or_expiry_missing"})
        source = f.get("source", {})
        stamp = source.get("as_of_ms") if isinstance(source, dict) else None
        if isinstance(stamp, int) and stamp > policy.now_ms + policy.max_clock_skew_ms:
            problems.append({"field": name, "reason": "future_source_time"})
    # Dedupe WOs by event/context/field/cause, no per-poll issue storm.
    tickets = []
    for p in problems:
        identity = {"event_id": envelope.get("event_id"), "context_id": envelope.get("context_id"), **p}
        tickets.append({**identity, "work_order_id": "WO-PC9-" + digest(identity)[:20],
                        "status": "OPEN", "required_action": "repair_producer_or_transport_and_replay_same_context"})
    unique = {t["work_order_id"]: t for t in tickets}
    return {"policy": "feedback-cards-reales-tiempo-real", "complete": not problems,
            "status": "COMPLETE_WITHIN_SUPPLIED_EVIDENCE" if not problems else "REPAIR_REQUIRED",
            "required_fields": len(policy.required_fields), "work_orders": list(unique.values()),
            "production_verified": False, "execution_authorized": False,
            "limitation": "Source authenticity and delivered/rendered receipts require independent live evidence."}


def compute_closed_cycle_card(request: dict[str, Any], snapshot: dict[str, Any], policy: CardPolicy) -> dict[str, Any]:
    """Revalue an actual event-bound closed-cycle quote ledger at ONE PriceBus view.

    Not a replacement for the strategy's quoting/sizing/simulation kernel. Every
    quantity must originate in its protocol-specific quote. Exact Decimal math
    preserves negative profit and non-18 decimals. No gross-as-net substitution.
    """
    policy.validate()
    identity = ("event_id", "strategy_id", "source_commit", "strategy_revision", "config_hash", "context_id", "route_hash")
    envelope = {"schema": "arbx.field-lineage.v1", **{k: request.get(k) for k in identity}, "fields": {}}
    fields: dict[str, Any] = envelope["fields"]
    faults: list[dict[str, str]] = []
    prices = None
    try:
        prices = CanonicalPriceView(snapshot, policy)
    except (ValueError, TypeError, KeyError) as exc:
        faults.append({"field": "price_snapshot", "reason": str(exc)})
    sources = []
    expiries = []

    def failure(name: str, reason: str, unit: str = "USD") -> None:
        fields[name] = {"state": "not_computed", "value": None, "unit": unit, "reason": reason}

    def result(name: str, value: Any, unit: str, refs: list[str], until: int, transform: str) -> None:
        fields[name] = {"state": "computed", "value": value, "unit": unit,
                        "context_id": request["context_id"], "valid_until_ms": until,
                        "source": {"kind": "calculation", "reference": "real_cards.compute_closed_cycle_card",
                            "as_of_ms": policy.now_ms, "transform_id": transform, "transform_version": "1",
                            "input_hashes": refs}}

    def valuation(raw: str, asset: str) -> tuple[Decimal, list[str], int]:
        qty = uint(raw)
        meta = request.get("token_metadata", {}).get(asset)
        if not isinstance(meta, dict) or meta.get("asset_key") != asset:
            raise ValueError("token_identity_metadata_missing")
        dec = meta.get("decimals")
        if isinstance(dec, bool) or not isinstance(dec, int) or not 0 <= dec <= 255:
            raise ValueError("token_decimals_missing_or_invalid")
        if not meta.get("source_reference") or not HASH.fullmatch(str(meta.get("evidence_hash", ""))):
            raise ValueError("token_metadata_provenance_missing")
        if prices is None:
            raise ValueError("pricebus_unavailable")
        p, rec = prices.price(asset)
        with localcontext() as ctx:
            ctx.prec = 512
            value = Decimal(qty).scaleb(-dec) * p
        return value, [rec_hash, meta["evidence_hash"], *rec["input_hashes"]], rec["valid_until_ms"]

    rec_hash = snapshot.get("snapshot_hash", "")
    legs = request.get("legs")
    ledger = []
    values = []
    try:
        if request.get("route_kind") != "closed_cycle":
            raise ValueError("family_specific_valuation_required_not_cpmm_fallback")
        if not isinstance(legs, list) or not 2 <= len(legs) <= policy.max_hops:
            raise ValueError("invalid_full_route_length")
        if not BLOCK_HASH.fullmatch(str(request.get("block_hash", ""))):
            raise ValueError("canonical_block_hash_required")
        if legs[0]["asset_in"] != legs[-1]["asset_out"]:
            raise ValueError("not_a_closed_asset_cycle")
        previous = None
        for i, leg in enumerate(legs):
            if leg.get("context_id") != request.get("context_id") or leg.get("block_hash") != request["block_hash"]:
                raise ValueError("quote_context_or_block_mismatch")
            if not HASH.fullmatch(str(leg.get("quote_evidence_hash", ""))) or not leg.get("protocol") or not leg.get("quote_reference"):
                raise ValueError("protocol_quote_provenance_missing")
            stamp = positive_time(leg.get("quoted_at_ms"))
            if stamp > policy.now_ms + policy.max_clock_skew_ms or policy.now_ms - stamp > policy.quote_max_age_ms:
                raise ValueError("quote_stale_or_future")
            if previous and (previous["asset_out"] != leg["asset_in"] or uint(previous["amount_out_raw"]) != uint(leg["amount_in_raw"])):
                raise ValueError("broken_leg_quantity_or_asset_continuity")
            # No mechanism changes a qty to match the next leg: mismatch is an error.
            vi, ri, ei = valuation(leg["amount_in_raw"], leg["asset_in"])
            vo, ro, eo = valuation(leg["amount_out_raw"], leg["asset_out"])
            until = min(ei, eo, stamp + policy.quote_max_age_ms)
            refs = list(dict.fromkeys([leg["quote_evidence_hash"], *ri, *ro]))
            sources.extend(refs); expiries.append(until)
            result(f"legs[{i}].in_usd", text(vi), "USD", refs, until, "raw_amount_times_pricebus")
            result(f"legs[{i}].out_usd", text(vo), "USD", refs, until, "raw_amount_times_pricebus")
            with localcontext() as ctx:
                ctx.prec = 512
                delta = vo - vi
            result(f"legs[{i}].delta_usd", text(delta), "USD", refs, until, "leg_mark_value_delta_not_realized_profit")
            ledger.append({"index": i, "asset_in": leg["asset_in"], "asset_out": leg["asset_out"],
                           "amount_in_raw": str(uint(leg["amount_in_raw"])), "amount_out_raw": str(uint(leg["amount_out_raw"])),
                           "quote_reference": leg["quote_reference"]})
            values.append((vi, vo)); previous = leg
        if values[0][0] <= 0:
            raise ValueError("positive_principal_required_for_roi")
        refs, until = list(dict.fromkeys(sources)), min(expiries)
        result("capital_in_usd", text(values[0][0]), "USD", refs, until, "raw_amount_times_pricebus")
        result("gross_out_usd", text(values[-1][1]), "USD", refs, until, "raw_amount_times_pricebus")
        with localcontext() as ctx:
            ctx.prec = 512
            gross = values[-1][1] - values[0][0]
        result("gross_profit_usd", text(gross), "USD", refs, until, "final_value_minus_initial_value")
        result("hop_count", len(legs), "count", refs, until, "full_ledger_length")
    except (ValueError, KeyError, TypeError) as exc:
        faults.append({"field": "route_ledger", "reason": str(exc)})
        # Don't retain a partly rendered coherent-looking ledger from a broken route.
        for name in tuple(fields):
            if name.startswith("legs["):
                del fields[name]
        ledger = []; values = []
        for name in ("capital_in_usd", "gross_out_usd", "gross_profit_usd", "hop_count"):
            failure(name, str(exc), "count" if name == "hop_count" else "USD")

    costs = request.get("costs", {})
    total = Decimal(0)  # additive identity for actually enumerated costs, not a fallback.
    costs_ok = True
    for name in COST_NAMES:
        key = f"costs.{name}"
        c = costs.get(name) if isinstance(costs, dict) else None
        try:
            if not isinstance(c, dict):
                raise ValueError("applicable_cost_input_missing")
            if c.get("state") == "not_applicable":
                reason = c.get("reason")
                if reason not in policy.na_reasons.get(key, ()):
                    raise ValueError("not_applicable_without_strategy_rule")
                fields[key] = {"state": "not_applicable", "value": None, "unit": "USD", "reason": reason}
                continue
            if c.get("context_id") != request.get("context_id"):
                raise ValueError("cost_context_mismatch")
            expiry = positive_time(c.get("valid_until_ms"))
            if expiry < policy.now_ms:
                raise ValueError("cost_stale")
            if not HASH.fullmatch(str(c.get("evidence_hash", ""))) or not c.get("reference"):
                raise ValueError("cost_provenance_missing")
            if c.get("basis") == "native_gas":
                # Integer wei product; the mark oracle never supplies gas units.
                meta = request.get("token_metadata", {}).get(c.get("asset_key"), {})
                if c.get("asset_key") != request.get("native_asset_key") or meta.get("native_currency") is not True or meta.get("decimals") != 18:
                    raise ValueError("evm_gas_requires_verified_native_wei_asset")
                raw = uint(c["gas_units"]) * uint(c["effective_gas_price_wei"])
                amount, extra, expiry_price = valuation(str(raw), c["asset_key"])
                refs = [c["evidence_hash"], *extra]; expiry = min(expiry, expiry_price)
            elif c.get("basis") == "usd_computed":
                amount = exact(c.get("amount_usd")); refs = [c["evidence_hash"]]
            else:
                raise ValueError("explicit_cost_basis_required")
            if amount < 0:
                raise ValueError("negative_cost_requires_rebate_contract")
            if c.get("treatment") not in {"deduct", "included_in_quote"}:
                raise ValueError("fee_treatment_required_to_avoid_double_count")
            if c["treatment"] == "deduct":
                with localcontext() as ctx:
                    ctx.prec = 512; total += amount
            result(key, text(amount), "USD", refs, expiry, "cost_conversion_" + c["basis"])
            fields[key]["treatment"] = c["treatment"]
            sources.extend(refs); expiries.append(expiry)
        except (ValueError, KeyError, TypeError) as exc:
            failure(key, str(exc)); costs_ok = False
    if values and costs_ok:
        with localcontext() as ctx:
            ctx.prec = 512
            net = exact(fields["gross_profit_usd"]["value"]) - total
            roi = net / values[0][0] * Decimal(100)
            bps = net / values[0][0] * Decimal(10_000)
        refs, until = list(dict.fromkeys(sources)), min(expiries)
        for name, val, unit in (("additional_cost_usd", total, "USD"), ("net_profit_usd", net, "USD"),
                                ("roi_pct", roi, "percent"), ("net_bps", bps, "bps")):
            result(name, text(val), unit, refs, until, "quote_ledger_less_nonembedded_costs")
            if unit in {"percent", "bps"}:
                from fractions import Fraction
                rational = Fraction(net) / Fraction(values[0][0]) * (100 if unit == "percent" else 10000)
                fields[name]["exact_rational"] = {"numerator": str(rational.numerator), "denominator": str(rational.denominator)}
                fields[name]["display_rounding"] = "Decimal precision=512, ROUND_HALF_EVEN; exact_rational is authoritative"
    else:
        for name, unit in (("additional_cost_usd", "USD"), ("net_profit_usd", "USD"), ("roi_pct", "percent"), ("net_bps", "bps")):
            failure(name, "upstream_quote_or_cost_incomplete", unit)
    # Risk, confidence, target and EVM results can only come from their OWN producers.
    # Collision is refused: consumers cannot overwrite independently calculated net.
    for name, record in request.get("producer_fields", {}).items():
        if name in fields:
            faults.append({"field": name, "reason": "producer_collision_with_computed_field"})
        else:
            fields[name] = record
    for name in policy.required_fields:
        if name not in fields:
            failure(name, "applicable_producer_output_missing", "producer_defined")
    audit = strict_card_audit(envelope, policy)
    for f in faults:
        ticket = {"event_id": request.get("event_id"), "context_id": request.get("context_id"), **f}
        audit["work_orders"].append({**ticket, "work_order_id": "WO-PC9-" + digest(ticket)[:20], "status": "OPEN"})
    if faults:
        audit["complete"] = False; audit["status"] = "REPAIR_REQUIRED"
    return {"schema": "arbx.real-card.v1", "envelope": envelope, "ledger": ledger,
            "audit": audit, "lifecycle_rejection_reason": request.get("rejection_reason"),
            "profit_semantics": "valuation_of_quote_ledger_not_guaranteed_execution_profit",
            "execution_authorized": False}
