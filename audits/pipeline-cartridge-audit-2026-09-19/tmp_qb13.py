import json, io, sys
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
d = json.load(open("QB_13_DETECTOR_POLICY.json", encoding="utf-8"))
print("rows:", len(d))
print("HEADER:", [str(c) for c in d[0]])
# find R_CLOSED_CYCLE and R_DIRECT_INDIRECT rows
for row in d:
    j = " | ".join(str(c)[:60] for c in row if c is not None and str(c).strip())
    if any(k in j for k in ("R_CLOSED_CYCLE", "R_DIRECT_INDIRECT", "OBSERVE")):
        print("-" * 40)
        for i, c in enumerate(row):
            if c is not None and str(c).strip():
                print(f"  [{i}] {str(c)[:150]}")
