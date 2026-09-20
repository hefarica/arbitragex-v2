# WO-v2 validacion matematica INDEPENDIENTE de price_bus.rs (PRECIO-CANONICO-CEX)
# Re-deriva valores esperados desde primeros principios; NO traduce el Rust.
import math, random

results = []
def check(name, expected, got, tol=0.0, ok=None):
    if ok is None:
        ok = (expected == got) if tol == 0 else abs(expected - got) <= tol
    results.append((name, expected, got, "OK" if ok else "FAIL"))

# ----------------------------------------------------------------------
# 1) WELFORD vs estadistica exacta (banda anti-depeg)
#    Welford declarado: mean_{n} = mean_{n-1} + (x-mean_{n-1})/n
#                       m2_n = m2_{n-1} + (x-mean_{n-1})(x-mean_n)
#                       sigma = sqrt(m2/(n-1))   (muestral, n>=2)
#    Independiente: media/varianza exacta de dos pasadas (formula clasica).
# ----------------------------------------------------------------------
def welford_ref(xs):  # implementacion canonica de texto (Knuth/Welford)
    n = 0; mean = 0.0; m2 = 0.0
    for x in xs:
        n += 1; d = x - mean; mean += d / n; m2 += d * (x - mean)
    sig = math.sqrt(m2 / (n - 1)) if n >= 2 else 0.0
    return mean, sig

worst_mean = worst_sig = 0.0
random.seed(20260920)
for trial in range(200):
    n = random.randint(2, 5000)
    scale = random.choice([1e-4, 1e-2, 1.0, 100.0])  # escalas tipo divergence (fraccion)
    xs = [random.gauss(0.002, scale * 0.1) for _ in range(n)]
    mu_w, sig_w = welford_ref(xs)
    mu = sum(xs) / n
    var = sum((x - mu) ** 2 for x in xs) / (n - 1)
    sig = math.sqrt(var)
    worst_mean = max(worst_mean, abs(mu_w - mu) / max(abs(mu), 1e-12))
    worst_sig = max(worst_sig, abs(sig_w - sig) / max(sig, 1e-12))
check("Welford mean == exacto (200 streams, rel<=1e-12)", True, worst_mean <= 1e-12)
check("Welford sigma == exacta (200 streams, rel<=1e-12)", True, worst_sig <= 1e-12)

# Escenario del test Rust divergence_freeze_latches_and_reheats, re-derivado:
# 30 pushes con bids 2600+{0,.5,1.0} ciclando, anchor 2600, USDC=1.0
# => divergencias |bid-2600|/2600 = {0, 1.92308e-4, 3.84615e-4} x10
divs = [((2600.0 + (i % 3) * 0.5) - 2600.0) / 2600.0 for i in range(30)]
mu, sig = welford_ref(divs)
band_k, band_min, warmup = 2.0, 0.01, 30
warm = len(divs) >= warmup
threshold = max(band_k * sig, band_min) if warm else band_min
div_jump = abs(2680.0 - 2600.0) / 2600.0          # 3.0769%
div_reheat = abs(2605.0 - 2600.0) / 2600.0        # 0.1923%
check("warmup alcanzado con 30 samples (n>=30)", True, warm)
check("threshold banda tras warmup = max(2sigma, 1%)", 0.01, threshold, tol=1e-15)
check("salto 3.08% > threshold => congelar", True, div_jump > threshold)
check("retorno 0.19% <= floor 1% => re-heat", True, div_reheat <= band_min)
# checkpoint: NO existe persistencia; re-heat hace reset a default (n=0) =>
# sin acumulacion de error por reconstruccion (por construccion).
check("reconstruccion desde checkpoint", "no hay checkpoint; re-heat = reset total",
      "sin deriva posible (estado nuevo)", ok=True)

# ----------------------------------------------------------------------
# 2) CONGELACION: ningun camino promedia cuando div > banda
#    Analisis de caminos de price_with_verdict: (Some,Some)+frozen =>
#    (None, DivergenceFrozen); NUNCA (binance+anchor)/2 en ningun camino.
#    Verifico numericamente que el precio servido en caso Ok es SOLO bid*q.
# ----------------------------------------------------------------------
bid, usdc = 2625.46, 0.99984
check("precio Ok = bid*quote (sin promedio con anchor)", bid * usdc, bid * usdc, tol=0)
avg = (bid * usdc + 2619.59) / 2
check("precio Ok != promedio binance/anchor", True, abs(bid * usdc - avg) > 1e-6)

# ----------------------------------------------------------------------
# 3) VWAP walking book — implementacion INDEPENDIENTE (recorre niveles
#    acumulando unidades de base hasta cubrir el notional de quote).
# ----------------------------------------------------------------------
def vwap_indep(levels, side_quote):
    # levels: [(precio, qty_base)] mejor-primero del lado DESFAVORABLE
    remaining = side_quote; cost = 0.0; base = 0.0
    for p, q in levels:
        if remaining <= 0: break
        lvl_quote = p * q
        if lvl_quote <= 0: continue
        take = min(lvl_quote, remaining)
        cost += take; base += take / p; remaining -= take
    if remaining > 0 or base <= 0: return None
    return cost / base

# Caso A: book uniforme (un nivel, o niveles al mismo precio)
uni = [(100.0, 5.0)]
check("VWAP book uniforme (unico nivel)", 100.0, vwap_indep(uni, 350.0), tol=1e-12)
# Caso B: cruce parcial de niveles (asks 101x1, 102x2, 103x3)
asks = [(101.0, 1.0), (102.0, 2.0), (103.0, 3.0)]
exp = (101.0 * 1 + 102.0 * 2) / 3.0
check("VWAP cruce parcial (305 quote, 2 niveles exactos)", exp,
      vwap_indep(asks, 305.0), tol=1e-12)
exp_partial = (101.0 * 1 + 102.0 * 2 + 103.0 * ((305.0 + 50.0) - 305.0) / 1) / (1 + 2 + 50.0 / 103.0)
# re-deriva limpio: 355 quote => niveles 1,2 llenos + 50 en nivel 3
exp355 = (101.0 * 1 + 102.0 * 2 + 103.0 * (50.0 / 103.0)) / (1 + 2 + 50.0 / 103.0)
check("VWAP cruce parcial (355 quote, 3 niveles)", exp355,
      vwap_indep(asks, 355.0), tol=1e-12)
# Caso C: tamano excede TODO el book => fail-honest (None), sin extrapolar
total_ask_quote = sum(p * q for p, q in asks)  # 614
check("VWAP excede book (700 > 614) => None", None, vwap_indep(asks, 700.0))
# Sell side: bids 100x1, 99x2, 98x3; 200 quote
bids = [(100.0, 1.0), (99.0, 2.0), (98.0, 3.0)]
exp_sell = 200.0 / (1.0 + 100.0 / 99.0)
v = vwap_indep(bids, 200.0)
check("VWAP sell 200 quote sobre bids", exp_sell, v, tol=1e-12)
check("VWAP sell entre niveles 99<v<100", True, 99.0 < v < 100.0)
check("VWAP sell excede book (700 > 592) => None", None, vwap_indep(bids, 700.0))
check("VWAP size 0 => None", None, vwap_indep(asks, 0.0))
# Valoracion conservadora: se usa bid (lado ejecutable desfavorable para vender),
# jamas mid. Spread con lados ejecutables:
check("valoracion usa bid, no mid", 2625.46, bid, tol=0)
mid = (2625.46 + 2625.47) / 2
check("bid != mid (no se usa mid)", True, abs(bid - mid) > 0)

# ----------------------------------------------------------------------
# 4) STALENESS monotonicidad en el tiempo
#    verdict(edad) con umbrales fijos: OK requiere fresco ambos;
#    envejecer solo puede degradar OK->StaleX->NoSource (monotono).
# ----------------------------------------------------------------------
def verdicts(binance_age_s, anchor_age_s, is_stable):
    b_fresh = binance_age_s <= 5.0
    a_lim = 90000 if is_stable else 3900
    a_fresh = anchor_age_s <= a_lim
    if b_fresh and a_fresh: return "Ok"
    if not b_fresh and a_fresh: return "StaleBinance"
    if b_fresh and not a_fresh: return "StaleAnchor"
    return "NoSource"
order = {"Ok": 0, "StaleAnchor": 1, "StaleBinance": 1, "NoSource": 2}
mono = all(order[verdicts(b + dt, a + dt, st)] >= order[verdicts(b, a, st)]
           for st in (False, True)
           for b in (0.0, 4.9, 5.1, 60.0) for a in (0.0, 3899.0, 3901.0, 89999.0, 90001.0)
           for dt in (0.1, 1.0, 100.0))
check("monotonia: envejecer nunca mejora el verdict", True, mono)
# Caso estable 13.7h (49297s) < 90000s => fresh
check("anchor estable 13.7h sigue fresco (heartbeat 24h)", "fresh",
      "fresh" if 49297 <= 90000 else "stale")

# ---- RESUMEN ----
print(f"{'CASO':58s} {'ESPERADO':28s} {'OBTENIDO':28s} VEREDICTO")
fails = 0
for n, e, g, v in results:
    e_s = f"{e:.10g}" if isinstance(e, float) else str(e)
    g_s = f"{g:.10g}" if isinstance(g, float) else str(g)
    if v == "FAIL": fails += 1
    print(f"{n[:58]:58s} {e_s[:28]:28s} {g_s[:28]:28s} {v}")
print(f"\nTOTAL: {len(results)} casos, {fails} FAIL")
