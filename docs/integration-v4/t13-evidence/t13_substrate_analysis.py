#!/usr/bin/env python3
# t13 — análisis del sustrato congelado qa-baseline/edge_opps_live.json (26 items)
import json, sys
from collections import Counter

F = r"C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\qa-baseline\edge_opps_live.json"
items = json.load(open(F, encoding="utf-8"))
if isinstance(items, dict):
    items = items.get("items") or items.get("data") or []
print("n_items:", len(items))

FIELDS = ["kelly_fraction_bps","simulated_net_profit_usd","simulated_amount_in_usd","simulated_gross_usd",
          "simulated_costs_total_usd","simulated_cost_breakdown","route_metadata","economics","bridge",
          "confirmations","pipeline_latency_ms","expected_profit_usd","net_expected_profit_usd","roi_pct",
          "token_prices_usd","detector_id","cartridge_id","strategy_kind","risk_score","confidence_score_bps",
          "amount_in_wei","pair_symbol","dex_a","dex_b"]
for f in FIELDS:
    nn = sum(1 for i in items if i.get(f) not in (None, "", [], {}))
    print(f"{f:-<34} {nn}/26")

sim = [i for i in items if i.get("simulated_net_profit_usd") is not None]
print("\nn_con_sim:", len(sim))
for s in sim[:3]:
    print("  amount=%s gross=%s costs=%s net=%s kelly_bps=%s roi=%s breakdown=%s" % (
        s.get("simulated_amount_in_usd"), s.get("simulated_gross_usd"), s.get("simulated_costs_total_usd"),
        s.get("simulated_net_profit_usd"), s.get("kelly_fraction_bps"), s.get("simulated_roi_pct"),
        json.dumps(s.get("simulated_cost_breakdown"))[:160]))

lat = [i.get("pipeline_latency_ms") for i in items if i.get("pipeline_latency_ms") is not None]
lat.sort()
if lat:
    import statistics
    print("\nlatency n=%d min=%s p50=%s p95=%s max=%s" % (len(lat), lat[0], lat[len(lat)//2], lat[int(len(lat)*0.95)-1], lat[-1]))

print("\nrejection:", dict(Counter(i.get("rejection_reason") for i in items)))
print("strategy_kind:", dict(Counter(i.get("strategy_kind") for i in items)))
print("paper_status:", dict(Counter(str(i.get("paper_status"))[:40] for i in items)))
rr = [i for i in items if i.get("route_metadata")]
print("route_metadata n:", len(rr))
if rr:
    print("rm sample keys:", list(rr[0]["route_metadata"].keys()) if isinstance(rr[0]["route_metadata"], dict) else str(rr[0]["route_metadata"])[:200])
    print("rm sample:", json.dumps(rr[0]["route_metadata"])[:600])
