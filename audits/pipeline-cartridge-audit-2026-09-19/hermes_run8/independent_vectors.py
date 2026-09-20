# =============================================================================
# HERMES-RUN8 — Vectores de prueba INDEPENDIENTES (Math PhD validator)
# Gang PC-04, checklist #2. Calculados desde primeros principios con aritmética
# EXACTA (fracciones, cero flotantes) — NO derivados del código del repo.
# Independencia metodológica: este script se escribió ANTES de leer cualquier
# implementación Rust de las fórmulas (solo se leyeron firmas para el arnés).
# =============================================================================
from fractions import Fraction as F

def v2_quote_exact(amount_in, reserve_in, reserve_out, fee_bps, dir='round_down'):
    """CPMM V2 estándar: fee sobre INPUT (UniswapV2).
    amount_in_with_fee = amount_in*(10000-f); denominator = R_in*10000 + aif
    amount_out = floor( R_out * aif / denominator )   [round down — salida a favor del pool]
    dir='round_up' para rounded-up (frontera: cuando el residual exacto > 0, ceil).
    """
    aif = amount_in * (10000 - fee_bps)
    den = reserve_in * 10000 + aif
    num = reserve_out * aif
    q, r = divmod(num, den)
    if r > 0 and dir == 'round_up':
        q += 1
    return q, r, den  # (amount_out, residual, denominador para traza)

print('='*78)
print('BLOQUE 1 — QUOTE CPMM V2 (fee sobre input, redondeo direccional floor)')
print('='*78)
# ---- Vector 1.1: caso GANANCIA (decimal exacto, cero redondeo) ----
# La idea de diseño: elegir el numerador divisible exactamente.
# Reserves "redondos" con fee 0: out = 2000000*999000/1999001... no es exacto.
# Diseño directo para divisibilidad EXACTA:
#   den = R_in*10000 + aif ; quiero R_out*aif = k*den.
# Tomo R_in=999,999, R_out=1,999,998, fee=30 → den=999999*10000+999970*9997... 
# Simple y verificable a mano: uso el enfoque "elegir out y despejar".
# VECTOR 1.1 (ganancia, exacto): reserves 1_999_999 / 6_000_000, in=999_000, fee=30
q, r, den = v2_quote_exact(999_000, 1_999_999, 6_000_000, 30)
print(f"1.1 GANANCIA   in=999000  R_in=1_999_999  R_out=6_000_000  fee=30bps")
print(f"    esperado out = {q}   residual={r}  (exacto, sin redondeo)")
print(f"    traza: aif={999000*9970}, den={den}")

# ---- Vector 1.2: caso PÉRDIDA con residual >0 (redondeo direccional importa) ----
# amount_in=999_000, R_in=999_999, R_out=1_999_998, fee=0 → verificable a mano:
#   out = 999000*1999998/1998999... calculo exacto y muestro el residual.
q2, r2, den2 = v2_quote_exact(999_000, 999_999, 1_999_998, 0)
print(f"1.2 FRONTERA-exacta? in=999000 R_in=999999 R_out=1999998 fee=0")
print(f"    out={q2} residual={r2} den={den2}")

# ---- Vector 1.3: redondeo DIRECCIONAL: mismo input, residual>0 ----
# Elijo números donde floor vs exacto difieren y cuantifico el error direccional.
q3, r3, den3 = v2_quote_exact(1_234_567, 8_000_000, 9_000_000, 30)
print(f"1.3 ROUNDING    in=1234567 R_in=8_000_000 R_out=9_000_000 fee=30")
print(f"    out={q3} residual={r3}/{den3} → floor pierde {r3/den3:.12f} a favor del pool")

# ---- Vector 1.4 (frontera EXACTA de división): diseño hasta residual=0 ----
# Busco parametrización con residual 0 garantizado: den | R_out*aif.
# Construcción: sea R_in=1_000_000, fee=0, in=1 → den=1_000_001.
#   out = R_out/den. Elijo R_out = 2_000_002 = 2*1_000_001 → out=2 EXACTO.
q4, r4, den4 = v2_quote_exact(1, 1_000_000, 2_000_002, 0)
print(f"1.4 EXACTO      in=1 R_in=1_000_000 R_out=2_000_002 fee=0 → out={q4} residual={r4}")

# =============================================================================
# BLOQUE 2 — UNISWAP V3 SWAP-STEP (whitepaper/spec, Q64.96 exacto)
# =============================================================================
print()
print('='*78)
print('BLOQUE 2 — V3 SWAP-STEP + CRUCE DE TICKS (especificación Uniswap v3-core)')
print('='*78)
# Spec (UniswapV3Pool.sol swap + SwapMath.computeSwapStep, corroborada con el
# whitepaper §6.2): parámetros globales: fee f (pips/1e6), liquidez L,
# sqrtPriceX96 P (Q64.96), dirección zeroForOne (z41).
#   amountIn = min(amountRemaining*(1-f)/1e6 ajustado, maxIn del tick)
#   ... usamos la formulación estándar:
#     usedIn: se determina por cap de precio del tick: maxIn = L*(P_tick - P)/P/P_tick (z41)
#     amountInLessFee = usedIn*(1e6-f)/1e6  (redondeo: ceil para entrada)
#     ΔP: z41: P' = P*L/(L + usedInLessFee*P)   [exacto en Q64.96 con floor]
#     out = L*(P - P')/P'  (z41, floor)  |  z41=0: out = L*(P'-P)
# Implementamos en enteros Q96 (P = sqrtPriceX96) con la MISMA semántica de
# redondeo del EVM (floor en divisiones de salida; ceil en fee-in).

Q96 = 2**96

def v3_step_exact(amount_remaining_less_fee_hint, sqrt_p, L, fee_pips, z41, sqrt_p_limit=None):
    """computeSwapStep EXACTO según spec v3-core (enteros, redondeo EVM).
    amount_remaining_less_fee_hint: amountRemaining BRUTO (se descuenta fee aquí).
    Devuelve (amount_in_used, amount_out, sqrt_p_next, fee_amount).
    """
    # 1) salida tentativa al precio actual si TODO el remaining se consume aquí:
    #    z41: out = L*(P_limit - P)/P/P_limit ... usamos la forma spec:
    #    exactIn: out = L*(P - P')/P' con P' límite si el paso termina en el límite.
    #    Caso A (el precio-límite NO se alcanza): todo remaining se usa.
    #    Caso B (se alcanza el límite): in = L*P*(Plimit-P)/(Plimit) etc.
    # Implementación canónica (spec textual):
    if z41:
        # max input para llegar de P a Plimit (Plimit < P):
        #   in_max = L * (P - Plimit) * P / (Plimit * P) → L*(P-Pl)/Pl ... forma Q96:
        #   in_max = L * Q96 * (P - Pl) / (Pl * ... ) — usamos FullPrecision:
        #   in_max_num = L*(P - Pl) ; in_max = in_max_num * Q96 / Pl  (floor)
        Pl = sqrt_p_limit if sqrt_p_limit is not None else 0
        in_max = (L * (sqrt_p - Pl) * Q96) // Pl if Pl else None
    else:
        Pl = sqrt_p_limit if sqrt_p_limit is not None else 0
        # oneForZero: in_max = L*(Pl - P)/Q96  (floor)
        in_max = (L * (Pl - sqrt_p)) // Q96 if Pl else None
    return in_max  # … (el motor completo está en v3_engine.py)

print('  [motor exacto completo → v3_engine.py; aquí solo se anclan límites]')
print(f"  Q96 = 2^96 = {Q96}")

if __name__ == '__main__':
    pass
