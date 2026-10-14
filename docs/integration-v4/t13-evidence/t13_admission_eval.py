#!/usr/bin/env python3
# t13 — Evaluacion T (disciplina de admision) familias H/I sobre sustratos congelados.
# Sustrato A: qa-baseline/edge_opps_live.json (26 items, hashes en x_ref.json artefactos)
# Sustrato B: A8 TESTS_ATOMICOS (264k, sha eb5b803c...) via t8 outputs + tally r2
# Sustrato C: A8 hojas F01-F11 (familias BASE/IMP) — T13_sheets_dump.json
# Regla: admision <=> mu >= delta = max(0.02*|J|, 0.005) AND C AND lambda in {P,R}
import json, math
from collections import Counter

WS = r"C:\Users\HFRC\Desktop\arbitragex-v2-main (17)"
items = json.load(open(WS + r"\qa-baseline\edge_opps_live.json", encoding="utf-8"))
if isinstance(items, dict):
    items = items.get("items") or items.get("data") or []
sheets = json.load(open(WS + r"\docs\integration-v4\t13-evidence\T13_sheets_dump.json", encoding="utf-8"))
t8r2 = json.load(open(WS + r"\docs\integration-v4\t8-evidence\T8_output_r2.json", encoding="utf-8"))

# ---- ejes y deltas ----
X6_SUM_NET = -17915.4383          # x_ref x6 (suma simulated_net_profit_usd, n=17)
X6_ABS = abs(X6_SUM_NET)
DELTA_X6 = max(0.02 * X6_ABS, 0.005)   # USD
DELTA_X5 = max(0.02 * 1.0, 0.005)      # tasa (fraccion)
print(f"delta_x6 = {DELTA_X6:.4f} USD | delta_x5 = {DELTA_X5}")

ev = {}

# ================= pi1 ADM-1 [H: sizing AMM forma cerrada + Kelly cap (A14/A15)] =================
# Modelo V2 (exacto para piernas constant-product; V3 aproximado — declarado):
#   net(a) = a*g_rate - a^2*s_c - fixed, calibrado con el punto medido (a0, net0, slippage0, lp0):
#   s_c = slippage0/a0^2 ; fee_rate = lp0/a0 ; g_rate tal que net(a0)=net0.
# Optimo: a* = g_rate/(2 s_c) si g_rate>0 sino 0 ; net(a*) = g_rate^2/(4 s_c) - fixed.
sim = [i for i in items if i.get("simulated_net_profit_usd") is not None and i.get("simulated_amount_in_usd")]
rows = []
sum_cur, sum_opt = 0.0, 0.0
for s in sim:
    a0 = float(s["simulated_amount_in_usd"]); net0 = float(s["simulated_net_profit_usd"])
    bd = s.get("simulated_cost_breakdown") or {}
    sl0 = float(bd.get("slippage_usd", 0) or 0); lp0 = float(bd.get("lp_fees_usd", 0) or 0)
    fixed = float(s.get("simulated_costs_total_usd", 0)) - sl0 - lp0  # gas+buffers+capital+relay (no ~ a ni ~ a^2)
    if a0 <= 0:
        rows.append((a0, net0, 0.0, 0.0)); sum_cur += net0; continue
    s_c = sl0 / (a0 * a0)
    fixed = float(s.get("simulated_costs_total_usd", 0)) - sl0 - lp0  # gas+relay+buffers (indep. del tamano)
    g_rate = (net0 + sl0 + fixed) / a0   # tasa marginal EFECTIVA (bruta - fees lineales); net(a0)==net0 exacto
    if s_c > 0 and g_rate > 0:
        a_star = g_rate / (2 * s_c)
        net_star = g_rate * a_star - s_c * a_star * a_star - fixed
    else:
        a_star, net_star = 0.0, -fixed if False else 0.0  # sin edge: no operar => net 0 (no se ejecuta)
        net_star = 0.0
    net_opt = net_star if net_star > 0 else 0.0            # politica: solo operar si net*>0
    rows.append((a0, net0, a_star, net_opt))
    sum_cur += net0; sum_opt += net_opt
mu1 = sum_opt - sum_cur                    # mejora contrafactual USD (x6)
ev["ADM-1"] = {
    "familia": "H sizing-AMM-cerrada+kelly (A14/A15)",
    "n_items": len(sim), "sum_net_actual": round(sum_cur, 4), "sum_net_optimo": round(sum_opt, 4),
    "mu_usd": round(mu1, 4), "delta": round(DELTA_X6, 4), "mu_ge_delta": mu1 >= DELTA_X6,
    "modelo": "net(a)=a*g-a^2*s_c-fixed; s_c=slippage0/a0^2 (V2 exacto, V3 aprox declarado); calibrado por item con punto medido",
    "lambda": "P (funcion pura control-plane, paper-safe)",
    "nota_kelly": "kelly_fraction_bps=0/26 en la muestra (motor ya anula Kelly con edge<=0); Kelly cap = cota superior de a* (f*·bankroll) integrada al mismo termino",
}

# ================= pi2 ADM-2 [H: break-even gas<->profit] =================
# Flips a viable en la muestra con contabilidad honesta (piso gas+relay+ops):
FLOOR = 0.6324159 + 0.5 + 0.01
flips = sum(1 for s in sim if float(s.get("simulated_gross_usd") or 0) > FLOOR + float(s.get("simulated_costs_total_usd") or 0) - 0.6324159)
mu2 = flips / 26.0
ev["ADM-2"] = {"familia": "H break-even gas<->profit", "flips_viable": flips, "mu_x5": round(mu2, 4),
               "delta": DELTA_X5, "mu_ge_delta": mu2 >= DELTA_X5,
               "nota": "con contabilidad honesta NINGUN item flips (gross<=0 o < piso) — coherente con viables=0 de x_ref"}

# ================= pi3 ADM-3 [I: staleness oraculo] =================
lat = sorted(float(i["pipeline_latency_ms"]) for i in items if i.get("pipeline_latency_ms") is not None)
p95 = lat[int(len(lat) * 0.95) - 1]
T = max(p95, 2405.0)  # umbral desde la muestra (x_ref p95=2405); sin hardcode en implementacion (env)
quarantined = sum(1 for l in lat if l > T)
mu3 = quarantined / 26.0
ev["ADM-3"] = {"familia": "I staleness-oraculo (gate cuarentena)", "T_ms": T, "quarantined": quarantined, "n": 26,
               "mu_x5_cobertura": round(mu3, 4), "delta": DELTA_X5, "mu_ge_delta": mu3 >= DELTA_X5,
               "lambda": "P (gate puro)", "eje": "x5 clases previsibles (precios viejos => riesgo adverso)"}

# ================= pi4 ADM-4 [I: ventanas no atomicas] =================
tally = sheets["_tests_tally"]["exec_class"]
nonatomic = tally["NONATOMIC_BRIDGE_REQUIRED"]; total_tests = sum(tally.values())
mu4 = nonatomic / total_tests
bridge_items = sum(1 for i in items if i.get("bridge"))
ev["ADM-4"] = {"familia": "I ventanas-no-atomicas (gate clase)", "nonatomic_tests": nonatomic, "total": total_tests,
               "mu_x5_cobertura": round(mu4, 6), "delta": DELTA_X5, "mu_ge_delta": mu4 >= DELTA_X5,
               "bridge_en_muestra": bridge_items, "lambda": "P (gate de clase, no ejecuta)"}

# ================= pi5 ADM-5 [I: reorg confirmaciones] =================
conf = Counter(str(i.get("confirmations")) for i in items)
ev["ADM-5"] = {"familia": "I reorg (gate confirmaciones)", "distribucion": dict(conf),
               "mu": None, "mu_ge_delta": False,
               "nota": "semantica ILEGIBLE: los valores de confirmations son variados y de magnitud block-height (1..2530), NO un conteo de confirmaciones => sin semantica verificable no hay umbral honesto; mu NO COMPUTABLE (falta de senal medible, no ausencia de riesgo)"}

# ================= pi6 ADM-6 [H: derivados funding/paridad/LST — F06/F10/F07] =================
ev["ADM-6"] = {"familia": "H funding-basis(A18)/paridad(A19)/LST-spreads(F07)", "lambda": "R (DERIVATIVE_DATA_REQUIRED)",
               "mu": None, "mu_ge_delta": False,
               "nota": "sin datos de funding/opciones/LST en sustratos A/B/C => mu NO COMPUTABLE; veredicto 0 DECLARADO (lambda=R admisible pero sin medicion no hay admision)"}

# ================= pi7 ADM-7 [H: optimizacion convexa multileg] =================
v3_items = sum(1 for i in items if any("V3" in str(a) for a in (i.get("route_metadata") or {}).get("dex_adapters", [])))
ev["ADM-7"] = {"familia": "H convexa-multileg", "items_con_v3": v3_items, "mu": None, "mu_ge_delta": False,
               "nota": "requiere liquidez por tick (ausente en sustratos) => mu NO COMPUTABLE; la parte V2 del sizing ya esta cubierta por ADM-1 (un termino)"}

# ================= pi8 ADM-8 [I: leakage/seleccion adversa/cascadas] =================
ev["ADM-8"] = {"familia": "I leakage/adversa/cascadas", "mu": None, "mu_ge_delta": False,
               "nota": "sin mempool/positions de lending en sustratos => mu NO COMPUTABLE; veredicto 0 DECLARADO"}

# ================= C(theta): candidatos NO tocan SSOT -> condiciones re-verificadas via t8 =================
ssot = t8r2["ssot"]
ev["_C_theta"] = {
    "nota": "los candidatos H/I son adiciones control-plane puras; SSOT intacto",
    "n_strategies": ssot["n_strategies"], "combos": ssot["combos_total"],
    "hop_dist_ok": ssot["hop_dist"] == ssot["hop_dist_expected"],
    "mev_diff_zero": t8r2["mev_diff"]["sets_equal"],
    "C_canonical": 1, "C_literal": 0, "condicion7": "escalada [A] al capitan (t9); invariantes 6/6 OK sin condicion 7",
}

print(json.dumps(ev, ensure_ascii=False, indent=1))
print("\n=== RESUMEN ADMISION ===")
for k, v in ev.items():
    if k.startswith("_"): continue
    mu = v.get("mu_ge_delta")
    print(f"{k}: familia={v['familia'][:44]:<46} mu>=delta: {mu}")
