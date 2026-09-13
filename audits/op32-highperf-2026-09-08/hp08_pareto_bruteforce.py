# HP-08 (2026-09-08) — verify v2: frente de Pareto TRUE con grid FINO.
# La region rentable por pool es x_i < x*_i = (r1*gamma - r0)/gamma
# (pool1 ~6991 tok = 0.7% de r0; pool2 ~1982 = 0.099%; pool3 ~5995 = 1.2%).
# Grid v1 (min 2% de r0) la salto por completo — bug MIO, corregido aqui.
import itertools, math

pools = [(1_000_000.0, 1_010_000.0), (2_000_000.0, 2_008_000.0), (500_000.0, 507_500.0)]
gamma = 1.0 - 30.0 / 10_000.0
price = 1.01
gas = 20.0 * 21_000.0 * 1e-9 * price
alpha = 0.5
per_leg_ms = 12_000.0

xstar = [(r1 * gamma - r0) / gamma for (r0, r1) in pools]
print("x* por pool (breakeven bruto):", [round(x, 1) for x in xstar],
      "fracciones:", [round(x / p[0], 5) for x, p in zip(xstar, pools)])
print("breakeven neto (gross > gas): ligeramente menor que x*")

def objectives(x):
    gross = 0.0
    legs = []
    for xi, (r0, r1) in zip(x, pools):
        denom = r0 + gamma * xi
        gross += (r1 * gamma * xi) / denom - xi
        if xi > 0.0:
            legs.append((gamma * xi) / denom)
    if not legs:
        risk = 0.0
    else:
        tail = min(len(legs), max(1, math.ceil(alpha * len(legs))))
        legs.sort(reverse=True)
        risk = sum(legs[:tail]) / tail
    return (-gross + gas, risk, len(legs) * per_leg_ms)

def dominates(a, b):
    return all(u <= v for u, v in zip(a, b)) and any(u < v for u, v in zip(a, b))

# Grid fino por pool: fracciones de x* (region rentable) + null + sobrepaso
fracs = [0.0, 0.05, 0.1, 0.2, 0.35, 0.5, 0.7, 0.85, 0.95, 1.05]
pts = []
for f in itertools.product(fracs, repeat=3):
    x = [f[i] * xstar[i] for i in range(3)]
    pts.append((x, objectives(x), f))

front = []
for (x, o, f) in pts:
    if not any(dominates(o2, o) for (_, o2, _) in pts if o2 != o):
        front.append((x, o, f))

print(f"\npuntos grid={len(pts)} frente_true={len(front)}")
for x, (f1, f2, f3), fr in sorted(front, key=lambda t: t[1][0]):
    print(f"  fracciones_de_x*={tuple(round(v,2) for v in fr)} net_yield={-f1:.2f} "
          f"(f1={f1:.6f}) risk={f2:.6f} lat={f3:.0f}ms")
print("\nfrente_true >= 2:", len(front) >= 2,
      "=> assert front>=2 del test (1) PREMISA VALIDA" if len(front) >= 2 else "=> premisa invalida")
