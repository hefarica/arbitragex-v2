#!/usr/bin/env python3
# c_evaluator.py - α7 FALSATOR reconstruction of the CONTRACT evaluator C(theta).
# Conditions transcribed VERBATIM from the contract (t17 task) as evaluated by t9
# (C_THETA_verdict.md lines 5-6). This is the ATTACKER's instrument: it does not
# produce claims, it only decides whether theta stops the evaluator.
#   C=1 iff |S|=264 AND |D|=60 AND |Omega|=1436 AND forall s 3<=|mask(s)|<=7
#         AND forall h d(h)=245/262/260/233/233/203 AND mask(MEV-05-003)={2}
#         AND forall j |Pi_j|=(53,31,31,14,30,30,25,20,18,12)_j
# Modes: --canonical drops condition 7 (the documented transcription errata;
#         captain decision pending, escalado [A] of t9).
# Exits 0 only if ALL conditions hold; exits 1 (STOP) on first violation.
import csv, json, sys
from pathlib import Path

def evaluate(ssot_dir: Path, canonical: bool):
    failures = []
    rows = list(csv.DictReader(open(ssot_dir/"detector_policy.csv", encoding="utf-8-sig")))
    strat = json.loads((ssot_dir/"strategy_hop_map.json").read_text(encoding="utf-8"))["strategies"]
    # cond 1: |S|=264
    if len(strat) != 264:
        failures.append(f"cond1 |S|={len(strat)} != 264")
    # cond 2: |D|=60
    if len(rows) != 60:
        failures.append(f"cond2 |D|={len(rows)} != 60")
    # cond 3: |Omega|=1436 (sum of hops across masks)
    omega = sum(len(s["mask"].split("/")) for s in strat)
    if omega != 1436:
        failures.append(f"cond3 |Omega|={omega} != 1436")
    # cond 4: 3<=|mask(s)|<=7 for all s
    bad = [s["mev_id"] for s in strat if not (3 <= len(s["mask"].split("/")) <= 7)]
    if bad:
        failures.append(f"cond4 mask size out of [3,7]: {bad[:5]}")
    # cond 5: d(h)=245/262/260/233/233/203 for h=2..7
    from collections import Counter
    dh = Counter(h for s in strat for h in (int(x) for x in s["mask"].split("/")))
    want = {2: 245, 3: 262, 4: 260, 5: 233, 6: 233, 7: 203}
    if dict(dh) != want:
        failures.append(f"cond5 d(h)={dict(sorted(dh.items()))} != {want}")
    # cond 7 (literal): mask(MEV-05-003)={2}
    if not canonical:
        m = next((s for s in strat if s["mev_id"] == "MEV-05-003"), None)
        got = set(int(x) for x in m["mask"].split("/")) if m else None
        if got != {2}:
            failures.append(f"cond7 mask(MEV-05-003)={sorted(got) if got else None} != {{2}}")
    # cond 8 (Pi_j): family strategy counts (R_+CF_ merged) = 53,31,31,30,30,25,20,18,14,12
    fam = Counter()
    for r in rows:
        f = r["FAMILIA"]
        if f in ("R_", "CF_"): f = "R_+CF_"
        fam[f] += int(r["ESTRATEGIAS"])
    fam = Counter({k: v for k, v in fam.items() if v > 0})  # OBSERVE (telemetry-only, 0 strategies) excluded
    # contract tuple (t17 task text / C_THETA_verdict.md:6): |Pi_j|=(53,31,31,14,30,30,25,20,18,12)_j
    # compared as a MULTISET of per-family counts (family assignment is not part of the contract).
    got_counts = sorted(fam.values(), reverse=True)
    want_counts = sorted([53, 31, 31, 14, 30, 30, 25, 20, 18, 12], reverse=True)
    if got_counts != want_counts:
        failures.append(f"cond8 |Pi_j| counts={got_counts} != {want_counts}")
    return failures

if __name__ == "__main__":
    ssot = Path(sys.argv[1])
    canonical = "--canonical" in sys.argv
    fails = evaluate(ssot, canonical)
    mode = "canonical(6 cond)" if canonical else "literal(7 cond)"
    if fails:
        print(f"C(theta)=0 [{mode}] - EVALUATOR STOPS:")
        for f in fails: print("  FAIL " + f)
        sys.exit(1)
    print(f"C(theta)=1 [{mode}] - all conditions hold")
    sys.exit(0)
