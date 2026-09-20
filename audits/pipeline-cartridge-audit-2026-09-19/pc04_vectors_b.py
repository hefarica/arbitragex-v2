# PC-04b — correcciones de constantes del vector independiente (los 5 FAIL previos
# eran errores del vector, no del repo). Constantes recalculadas de forma independiente.
from fractions import Fraction as F
import math

results = []
def check(name, ok, detail):
    results.append((name, ok, detail))
    print(("PASS " if ok else "FAIL ") + name + " :: " + detail)

# 1) 0.997^3 recalculado: 997*997*997 = 991026973 -> /1e9
g3 = F(997, 1000) ** 3
check("spot_product equal-reserves = 0.991026973 < 1 (reject honesto)",
      float(g3) == 0.991026973 and g3 < 1, f"0.997^3 = {float(g3):.9f}")

# 2) umbral: prod rates > 1/0.997^3
inv = 1.0 / float(g3)
check("spot_product umbral prod(rates) > 1.009054",
      abs(inv - 1.009054) < 1e-6, f"1/0.997^3 = {inv:.9f}")

# 3) V3 single-tick: la diferencia de 1 wei viene del floor del inverso.
#    Verificar con tolerancia <= floor-error (2 wei) y ademas la identidad
#    exacta cuando x' divide exacto: usar sp_target = sp*99/100 NO divide
#    exacto; usar delta exacto: sp_next = sp - k, k | (L*(sp-sp_next)) etc.
#    Alternativa limpia: elegir L, sp, x' tal que todas las divisiones sean
#    exactas: L=2^60, sp=Q96, x'=L/1000 (delta sp = x'*sp/Q96 / (1+...)).
#    En su lugar: verificacion de conservacion — out calculado hacia atras
#    desde sp_next debe reproducir x' con error <= redondeo floor (rel 1e-15).
Q96 = 1 << 96
L = 10**18
sp0 = Q96
def forward_zero_for_one(x, L, sp, fee_pips):
    def mul_div(a, b, d): return (a*b)//d
    xf = mul_div(x, 1_000_000 - fee_pips, 1_000_000)
    denom = L + mul_div(xf, sp, Q96)
    sp_next = mul_div(L, sp, denom)
    out = mul_div(L, sp - sp_next, Q96)
    return out, sp_next
sp_target = sp0 * 99 // 100
x_inv = (L * Q96 * (sp0 - sp_target)) // (sp0 * sp_target)
out_analytic = (L * (sp0 - sp_target)) // Q96   # = L//100 exacto = 10^16
out_fwd, sp_fwd = forward_zero_for_one(x_inv, L, sp0, 0)
# x_inv fue floor-eado: el forward parte de un x LIGERAMENTE menor =>
# sp_fwd >= sp_target y out_fwd <= out_analytic, diferencia acotada por
# el error relativo del floor de x_inv (x_inv ~ 1.01e16, error <= 1 -> rel 1e-16).
rel_out = abs(out_fwd - out_analytic) / out_analytic
rel_sp = abs(sp_fwd - sp_target) / sp_target
check("V3 single-tick analitico (tolerancia floor rel<=1e-15)",
      rel_out <= 1e-15 and rel_sp <= 1e-15 and sp_fwd >= sp_target,
      f"out_rel={rel_out:.2e} sp_rel={rel_sp:.2e} (diferencia 1 wei por floor del inverso)")

# 4) prefilter: la equivalencia log vs producto ES la asercion (ambos lados
#    deben coincidir en el veredicto, pase lo que pase). Caso reject y caso pass.
def prefilter_log(rates, fee):
    return sum(-math.log((1-fee)*r) for r in rates)
def prefilter_prod(rates, fee):
    return math.prod(rates) * (1-fee)**len(rates) > 1.0
r_reject = [1/1.010, 1/0.995, 1/0.9995]
r_pass = [1.004, 1.002, 1.001]   # prod 1.007 > 1/0.997^3=1.00905? NO -> 1.007*0.991=0.998 reject
r_pass2 = [1.006, 1.003, 1.001]  # prod = 1.01001 -> *0.991027 = 1.00098 > 1 pass
c1 = (prefilter_log(r_reject, 0.003) < 0) == prefilter_prod(r_reject, 0.003)
c2 = (prefilter_log(r_pass2, 0.003) < 0) == prefilter_prod(r_pass2, 0.003)
v1 = prefilter_log(r_reject, 0.003)  # > 0 => reject
v2 = prefilter_log(r_pass2, 0.003)   # < 0 => pass
check("prefilter log==producto (caso reject y caso pass)",
      c1 and c2 and v1 > 0 and v2 < 0,
      f"reject: logsum={v1:+.6f}(>0) pass: logsum={v2:+.6f}(<0) gamma3prod_pass={prefilter_prod(r_pass2,0.003)}")

# 5) net_bps: 60-10-2=48; flash=6.1728; net=41.8272; bps sobre start=10k
net = 60.0 - 10.0 - 2.0 - 12345.6*5/10000
bps = 10000.0 * net / 10000.0
check("net=41.8272 bps=41.8272 (>=5 pasa DEFAULT_MIN_NET_BPS)",
      abs(net - 41.8272) < 1e-9 and abs(bps - 41.8272) < 1e-9 and bps >= 5.0,
      f"net={net:.4f} bps={bps:.4f}")

print()
fails = [r for r in results if not r[1]]
print(f"TOTAL: {len(results)} checks, {len(fails)} FAIL")
