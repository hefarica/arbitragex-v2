# =============================================================================
# HERMES-RUN8 — Motor V3 INDEPENDIENTE, ESPEC-EXACTO
# Fuente: Uniswap v3-core/main contracts/libraries/SwapMath.sol + SqrtPriceMath.sol
# (descargado 2026-09-19, texto completo preservado en spec_v3core_snapshot.txt)
# Semántica reproducida LITERALMENTE (con las mismas convenciones de redondeo):
#   - amountRemainingLessFee = mulDiv(remaining, 1e6-f, 1e6)            [FLOOR]
#   - amountIn cap (z41)  = getAmount0Delta(target, cur, L, roundUp)   [doble CEIL]
#   - amountIn cap (o4z)  = getAmount1Delta(cur, target, L, roundUp)   [CEIL]
#   - cap condition: lessFee >= amountIn  →  S_next = S_target
#   - S_next (z41)  = mulDivRoundingUp(L<<96 * S, L<<96 + amt*S)       [CEIL]
#   - S_next (o4z)  = S + floor(amt<<96 / L)                            [FLOOR]
#   - amountOut (z41) = getAmount1Delta(next, cur, L, false)           [FLOOR]
#   - amountOut (o4z) = getAmount0Delta(cur, next, L, false)           [FLOOR doble]
#   - fee: uncapped → remaining - amountIn ; capped → ceil(amtIn*f/(1e6-f))
# TickMath: floor(sqrt(1.0001^t))·2^96 exacto por bisección entera
#   (documentado: TickMath on-chain difiere ≤1 ulp en ticks extremos).
# =============================================================================
Q96 = 2 ** 96
E6 = 10 ** 6
MIN_TICK, MAX_TICK = -887272, 887272

def _cdiv(a, b):
    return -((-a) // b)

def _isqrt_frac(p, q):
    """floor(sqrt(p/q)) exacto (p,q enteros positivos) por bisección."""
    lo, hi = 0, 1
    while hi * hi * q <= p:
        hi <<= 1
    lo = hi >> 1
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if mid * mid * q <= p:
            lo = mid
        else:
            hi = mid - 1
    return lo

def sqrt_ratio_at_tick(t):
    """floor(2^96 · 1.0001^(t/2)) exacto."""
    if t < 0:
        return _isqrt_frac((2 ** 192) * (10000 ** (-t)), 10001 ** (-t))
    return _isqrt_frac((2 ** 192) * (10001 ** t), 10000 ** t)

# Anclas públicas (v3-core TickMath.sol constantes — verificadas contra GitHub):
assert sqrt_ratio_at_tick(0) == 2 ** 96
assert sqrt_ratio_at_tick(-887272) in (4295128738, 4295128739)  # TickMath oficial: 4295128739 (±1 ulp documentado)
assert abs(sqrt_ratio_at_tick(887272) - 1461446703485210103287273052203988822378723970342) <= 1  # MAX oficial ±1

# --- SqrtPriceMath spec-exacto ------------------------------------------------

def get_amount0_delta(sqrt_a, sqrt_b, L, round_up):
    """amount0 = L·2^96·(Sb−Sa)/(Sb·Sa); Sa=sqrt inferior. Spec: double mulDiv."""
    if sqrt_a > sqrt_b:
        sqrt_a, sqrt_b = sqrt_b, sqrt_a
    num1 = L << 96
    num2 = sqrt_b - sqrt_a
    if round_up:
        return _cdiv(_cdiv(num1 * num2, sqrt_b), sqrt_a)
    return ((num1 * num2) // sqrt_b) // sqrt_a

def get_amount1_delta(sqrt_a, sqrt_b, L, round_up):
    if sqrt_a > sqrt_b:
        sqrt_a, sqrt_b = sqrt_b, sqrt_a
    d = sqrt_b - sqrt_a
    if round_up:
        return _cdiv(L * d, Q96)
    return (L * d) // Q96

def get_next_sqrt_price_from_input(S, L, amount_in, zero_for_one):
    if zero_for_one:
        num = (L << 96) * S
        den = (L << 96) + amount_in * S
        return _cdiv(num, den)           # mulDivRoundingUp
    q = (amount_in << 96) // L           # floor
    return S + q

# --- SwapMath.computeSwapStep spec-exacto (exactIn) ---------------------------

def compute_swap_step(amount_remaining, S_cur, S_target, L, fee_pips):
    """Devuelve (amount_in, amount_out, fee_amount, S_next). exactIn > 0."""
    assert amount_remaining > 0 and L > 0
    zero_for_one = S_cur >= S_target
    less_fee = (amount_remaining * (E6 - fee_pips)) // E6          # FLOOR
    if zero_for_one:
        amount_in = get_amount0_delta(S_target, S_cur, L, True)
    else:
        amount_in = get_amount1_delta(S_cur, S_target, L, True)
    if less_fee >= amount_in:
        S_next = S_target
    else:
        S_next = get_next_sqrt_price_from_input(S_cur, L, less_fee, zero_for_one)
    reached = S_next == S_target
    if zero_for_one:
        amount_in = amount_in if reached else get_amount0_delta(S_next, S_cur, L, True)
        amount_out = get_amount1_delta(S_next, S_cur, L, False)
    else:
        amount_in = amount_in if reached else get_amount1_delta(S_cur, S_next, L, True)
        amount_out = get_amount0_delta(S_cur, S_next, L, False)
    if S_next != S_target:
        fee_amount = amount_remaining - amount_in
    else:
        fee_amount = _cdiv(amount_in * fee_pips, E6 - fee_pips)
    return amount_in, amount_out, fee_amount, S_next

# --- Loop multi-tick (Pool.swap, exactIn, sin límite externo de precio) -------

def swap_exact_in(amount_in_gross, S0, L0, fee_pips, zero_for_one, tick_nets):
    """tick_nets: {tick: liquidityNet} (net = L_below − L_above, convención v3).
    Devuelve traza completa por step + totales."""
    S_cur, L = S0, L0
    borders = sorted(tick_nets.keys(), reverse=zero_for_one)  # z41: descendente
    remaining = amount_in_gross
    total_out = total_fee = 0
    trace, crossed = [], 0
    while remaining > 0:
        S_target, nxt = None, None
        for t in borders:
            St = sqrt_ratio_at_tick(t)
            if zero_for_one and St < S_cur:
                S_target, nxt = St, t
                break
            if (not zero_for_one) and St > S_cur:
                S_target, nxt = St, t
                break
        if S_target is None:
            S_target = sqrt_ratio_at_tick(MIN_TICK if zero_for_one else MAX_TICK)
        amt_in, out, fee, S_next = compute_swap_step(remaining, S_cur, S_target, L, fee_pips)
        if amt_in == 0 and out == 0:
            trace.append({'event': 'halt', 'S': str(S_cur)})
            break
        trace.append({'S_from': str(S_cur), 'S_to': str(S_next), 'L': str(L),
                      'tick': nxt, 'net_in': str(amt_in), 'out': str(out),
                      'fee': str(fee), 'capped': S_next == S_target and nxt is not None})
        total_out += out
        total_fee += fee
        remaining -= (amt_in + fee)
        S_cur = S_next
        if S_next == S_target and nxt is not None:
            L += tick_nets[nxt] if zero_for_one else -tick_nets[nxt]
            crossed += 1
            if L <= 0:
                trace.append({'event': 'liquidity_zero', 'at': nxt})
                break
        elif S_next == S_target and nxt is None:
            break
        elif S_next != S_target:
            break  # input agotado dentro del segmento
    return {'total_out': total_out, 'S_final': S_cur, 'fee_total': total_fee,
            'ticks_crossed': crossed, 'steps': trace}
