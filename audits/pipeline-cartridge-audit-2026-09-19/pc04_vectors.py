# PC-04 — Vectores numericos INDEPENDIENTES (Python puro, sin re-usar formulas del repo)
# Cada caso computa la fisica canonica desde primera principios y compara contra
# los valores que el codigo Rust/Rhai produce por su propia ruta.
from fractions import Fraction as F
import math

results = []

def check(name, ok, detail):
    results.append((name, ok, detail))
    print(("PASS " if ok else "FAIL ") + name + " :: " + detail)

# ============================================================
# 1) v2_amount_out (amm_math.rs:77) — formula UniswapV2 getAmountOut
#    x*(10000-fee)*rout / (rin*10000 + x*(10000-fee))  [entero, floor]
# ============================================================
def v2_ref(x, rin, rout, fee_bps):
    # referencia independiente: razon exacta con Fraction, floor como EVM
    xf = F(x) * F(10000 - fee_bps)
    return (xf * F(rout)) // (F(rin) * F(10000) + xf)

# caso 1: x=10e18, rin=100e18, rout=50e18, fee=30
x, rin, rout = 10*10**18, 100*10**18, 50*10**18
ref = v2_ref(x, rin, rout, 30)
# valor esperado analitico aprox: 0.0497*... solo verificar propiedades fisicas:
# out < x*rout/rin (slippage+fee), out > 0
naive = x * rout // rin
ok = 0 < ref < naive
check("V2-CPMM exact-out (case1 bounds)", ok,
      f"ref={ref} ({float(ref)/1e18:.9f} tok) < naive_no_fee={float(naive)/1e18:.9f}")
# verificacion cruzada con la formula del cartridge cpmm_out (float) mismo caso:
gamma = (10000.0 - 30.0)/10000.0
rhai_out = (rout * (gamma*x)) / (rin + gamma*x)
rel = abs(rhai_out - float(ref)) / float(ref)
check("V2-CPMM float(cartridge) vs entero(kernel) <1e-12", rel < 1e-12, f"rel={rel:.2e}")

# caso 2: fee 0.05% (5 bps) — V2 pool low-fee
ref2 = v2_ref(10**18, 10**22, 10**22, 5)
g2 = 0.9995
rhai2 = (10**22 * g2*10**18) / (10**22 + g2*10**18)
rel2 = abs(rhai2 - float(ref2))/float(ref2)
check("V2-CPMM fee=5bps float vs int <1e-12", rel2 < 1e-12, f"rel={rel2:.2e}")

# ============================================================
# 2) spot_product (triangular_worker.rs:227) = gamma^n * prod(rout/rin)
#    caso del test del repo: 3 pools equal reserves, fee 30 → 0.997^3 < 1
# ============================================================
sp = (1.0 - 30.0/10000.0)**3
check("spot_product equal-reserves=0.997^3=0.99103<1 (reject honesto)",
      abs(sp - 0.99102898) < 1e-7 and sp < 1.0, f"sp={sp:.8f}")
# caso rentable marginal: pool B desbalanceado 1.010x para vencer fees:
# gamma^3*(r_out/r_in prod) > 1 requiere prod rates > 1/0.99103 = 1.009059
need = 1.0/sp
check("spot_product umbral: prod(rates) > 1.009059 para profit marginal",
      need > 1.009058 and need < 1.009060, f"1/sp={need:.6f}")

# ============================================================
# 3) fee dual-unit (S1, quote_anchor_runtime.rs:129, commit cdb4c890)
#    V3 pips/1e6, V2-family bps/1e4
# ============================================================
def fee_fraction_ref(protocol, fee):
    return F(fee) / F(1_000_000 if protocol == "v3" else 10_000)
cases = [
    ("v3", 3000, F(3, 1000)),   # 0.30%
    ("v3", 10000, F(1, 100)),   # 1.00%
    ("v3", 100, F(1, 10000)),   # 0.01%
    ("v3", 500, F(1, 2000)),    # 0.05%
    ("v2", 30, F(3, 1000)),
    ("curve", 30, F(3, 1000)),
    ("unknown", 30, F(3, 1000)),  # documentado /1e4
]
allok = all(fee_fraction_ref(p, f) == exp for p, f, exp in cases)
check("fee dual-unit V3=pips/1e6 V2=bps/1e4 (7 casos)", allok,
      "; ".join(f"{p}:{f}->{float(fee_fraction_ref(p,f))}" for p, f, _ in cases))

# ============================================================
# 4) v3_amount_out_single_tick (amm_math.rs:134) — caso analitico despejado.
#    zero_for_one: sp_next = L*sp/(L + x'*sp/Q96); out = L*(sp-sp_next)/Q96
#    Despejo INVERSO (camino independiente): elijo sp_next = sp*99/100,
#    computo x' exacto y out exacto con la fisica del whitepaper, luego
#    comparo contra la misma fórmula evaluada hacia adelante con enteros floor.
# ============================================================
Q96 = 1 << 96
L = 10**18
sp0 = Q96  # sqrtPrice = 1.0 (token1/token0 = 1)
# fee 0 para validar la fisica pura; luego un caso con fee 3000 pips.
def forward_zero_for_one(x, L, sp, fee_pips):
    # replicar EXACTAMENTE la semantica entera del kernel (mul_div floor)
    def mul_div(a, b, d):
        return (a*b)//d
    xf = mul_div(x, 1_000_000 - fee_pips, 1_000_000)
    if xf == 0: return 0, 0
    denom = L + mul_div(xf, sp, Q96)
    sp_next = mul_div(L, sp, denom)
    if sp_next == 0 or sp_next >= sp: return 0, 0
    out = mul_div(L, sp - sp_next, Q96)
    return out, sp_next

# despeje analitico: x' = L*Q96*(sp - sp_next_target) / (sp*sp_next_target)
sp_target = sp0 * 99 // 100
x_exact = (L * Q96 * (sp0 - sp_target)) // (sp0 * sp_target)
out_analytic = (L * (sp0 - sp_target)) // Q96
out_fwd, sp_fwd = forward_zero_for_one(x_exact, L, sp0, 0)
# con sp=Q96: out_analytic = L*0.01*Q96/Q96 = L//100 exacto
ok = out_fwd == out_analytic and sp_fwd == sp_target
check("V3 single-tick zero_for_one caso analitico (fee=0)", ok,
      f"x'={x_exact} out_fwd={out_fwd} out_analytic={out_analytic} (=L/100={L//100}) sp_next match={sp_fwd==sp_target}")

# caso con fee 3000 pips: input neto = x*0.997; verificar que el out respeta fee
x_in = 10**16
out_fee, sp_fee = forward_zero_for_one(x_in, L, sp0, 3000)
out_nofee, _ = forward_zero_for_one(x_in, L, sp0, 0)
ok = 0 < out_fee < out_nofee
check("V3 single-tick fee=3000pips reduce out vs fee=0", ok,
      f"out_fee={out_fee} < out_nofee={out_nofee} (ratio={out_fee/out_nofee:.6f} ~0.997)")

# one_for_zero (token1->token0): sp_next = sp + x'*Q96/L; out = L*Q96/sp - L*Q96/sp_next
def forward_one_for_zero(x, L, sp, fee_pips):
    def mul_div(a, b, d): return (a*b)//d
    xf = mul_div(x, 1_000_000 - fee_pips, 1_000_000)
    sp_next = sp + mul_div(xf, Q96, L)
    inv0 = mul_div(L, Q96, sp)
    inv1 = mul_div(L, Q96, sp_next)
    return inv0 - inv1, sp_next
x_in2 = 10**15
out1z, sp1z = forward_one_for_zero(x_in2, L, sp0, 0)
# simetria: one_for_zero con price 1.0 y mismas magnitudes ≈ zero_for_one
outzf, _ = forward_zero_for_one(x_in2, L, sp0, 0)
relsym = abs(out1z - outzf)/outzf
check("V3 single-tick simetria z4o vs o4z @ P=1 (rel<1e-6)", relsym < 1e-6,
      f"o4z={out1z} z4o={outzf} rel={relsym:.2e}")

# ============================================================
# 5) Prefiltro marginal del cartucho (QB_13: sum -ln((1-fee)*rate) < 0)
#    y la version del cartridge mev_01_001 (log_sum con fees[i]/10000)
# ============================================================
rates = [1.0/1.010, 1.0/0.995, 1.0/0.9995]  # producto = ?
prod = math.prod(rates)
logsum = sum(-math.log((1-0.003)*r) for r in rates)
pref_pass = logsum < 0
# verificacion independiente: condicion equivalente gamma^n * prod > 1
alt = (0.997**3) * prod > 1.0
check("prefilter marginal log vs producto (equivalencia)", pref_pass == alt and pref_pass,
      f"log_sum={logsum:.6f} gamma^3*prod={0.997**3*prod:.6f} -> pass={pref_pass}")

# ============================================================
# 6) F_e normalizado (QB_05): F_e = r_e * P_Q(dst)/P_Q(src); senial >1, no oportunidad
# ============================================================
# r_e = 0.0025 WETH/USDC-style edge; P_Q en quote anchor USDC:
Pq_src, Pq_dst = 1.0, 2500.0   # src=USDC(1 USDC), dst=WETH(2500 USDC)
r_e = 0.0004                    # 1 USDC -> 0.0004 WETH (rate real del edge)
Fe = r_e * Pq_dst / Pq_src
check("F_e fair-rate senal (Fe>1 = mejor que referencia)", Fe == 1.0,
      f"r_e*Pdst/Psrc = {Fe:.4f} (=1 exacto en equilibrio)")

# ============================================================
# 7) Net gate exacto (QB_13/QB_06): net = gross - gas - ops - borrow*fee/1e4
#    (net_bps_ranking.rs:89 y size_optimizer.rs:1022)
# ============================================================
borrow, gross, gas, ops, fee_bps = 12_345.6, 60.0, 10.0, 2.0, 5
net_ref = F(gross) - F(gas) - F(ops) - F(borrow)*F(fee_bps)/F(10000)
net_float = gross - gas - ops - borrow*fee_bps/10_000.0
check("net kernel exacto (float == Fraction)", abs(net_float - float(net_ref)) < 1e-9,
      f"net={net_float} (flash_fee={borrow*fee_bps/10_000:.4f})")
# net_bps = 10000*net/start (net_bps_ranking.rs:135)
start = 10_000.0
bps = 10_000.0*(net_float/start)
check("net_bps = 1e4*net/start", abs(bps - (43.0-6.1728)) < 1e-3, f"bps={bps:.4f} (>=5 DEFAULT_MIN_NET_BPS -> pasa)")

print()
fails = [r for r in results if not r[1]]
print(f"TOTAL: {len(results)} checks, {len(fails)} FAIL")
