#!/usr/bin/env python3
# phi_static.py - Φ g1..g6 SOLO desde el A1 origen (MANIFEST.txt) — anti-circularidad §7.
# g7 (cap nuevas +3, INSPECCIÓN) NO es automatizable por diseño: gate manual documentado en
# ../CRITERIOS_T6.md (evaluación t6: 3 caps adm/ t13, unittest 10/10).
# Usage: python phi_static.py <path-a-MANIFEST.txt>
# exit 0 ⇔ g1..g6 todos PASS.
import re
import sys

WANT = {
    "strategies": 264, "detectors": 60, "combos": 1436,
    "deriv_pos_min": 1,  # > 0
    "delta_min_pct": 58.0,
}


def main() -> int:
    src = open(sys.argv[1], encoding="utf-8").read()
    def num(pat):
        m = re.search(pat, src)
        return float(m.group(1)) if m else None
    checks = {
        "g1 strategies==264": num(r"strategies=(\d+)") == WANT["strategies"],
        "g2 detectors==60": num(r"detectors=(\d+)") == WANT["detectors"],
        "g3 combos==1436": num(r"combos=(\d+)") == WANT["combos"],
        "asserts line OK": bool(re.search(r"asserts 264/60/1436: OK", src)),
        "g4 deriv_pos>0": (lambda v: v is not None and v > 0)(num(r"deriv_pos:\s*(\d+)")),
        "g5 imp>=base": (lambda i, b: i is not None and b is not None and i >= b)(
            num(r"imp_pos:\s*(\d+)"), num(r"base_pos:\s*(\d+)")),
        "g6 delta_pct>=+58%": (lambda d: d is not None and d >= WANT["delta_min_pct"])(
            num(r"delta_pct:\s*\+?(\d+(?:\.\d+)?)%")),
    }
    ok = True
    for name, passed in checks.items():
        print(("PASS " if passed else "FAIL ") + name)
        ok = ok and passed
    print("PHI_STATIC(g1..g6)=" + ("1" if ok else "0") + "  (g7=INSPECCION: ver CRITERIOS_T6.md -> PASS t6)")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
