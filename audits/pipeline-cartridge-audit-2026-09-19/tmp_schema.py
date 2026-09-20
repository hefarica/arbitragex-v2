import json, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
X = r"C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\audits\pipeline-cartridge-audit-2026-09-19\xlsx_extract"

def load(n):
    with open(X + "\\" + n + ".json", encoding="utf-8") as f:
        return json.load(f)

for name in ["ULTRA_11_STRATEGY_CATALOG", "QB_11_STRATEGY_HOP_MAP"]:
    d = load(name)
    print("=" * 25, name, "rows:", len(d))
    for i in (0, 1, 2, len(d) - 1):
        row = d[i]
        print(f"-- row {i} ({len(row)} cols):")
        for j, c in enumerate(row):
            if c is not None and str(c).strip():
                print(f"   [{j}] {str(c)[:90]}")
