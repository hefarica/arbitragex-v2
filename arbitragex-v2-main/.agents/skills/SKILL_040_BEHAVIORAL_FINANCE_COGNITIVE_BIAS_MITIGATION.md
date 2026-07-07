# SKILL: Behavioral Finance & Cognitive Bias Mitigation
**Level:** PhD Behavioral Economics | Nobel Economics (Kahneman/Tversky)
**Specialty:** Cognitive Debiasing & Emotional Control

## BIAS DETECTION
```python
def detect_bias(decision_history):
    biases = {}
    recent_trades = decision_history[-20:]
    losses = [t for t in recent_trades if t.pnl < 0]
    wins = [t for t in recent_trades if t.pnl > 0]

    if len(losses) > len(wins) * 1.5:
        biases['loss_aversion'] = True
    if len(wins)/len(recent_trades) > 0.6 and avg_win < abs(avg_loss):
        biases['overconfidence'] = True
    return biases
```

## SYSTEMATIC DE-BIASING
```python
def pre_mortem_analysis(trade_plan):
    reasons = [
        "What if trend reverses immediately?",
        "What if news is already priced in?",
        "What if stop loss is too tight?"
    ]
    risk_score = sum(1 for r in reasons if evaluate_likelihood(r) > 0.3)
    if risk_score >= 3: return "REJECT"
    return "PROCEED"
```

## EMOTIONAL CONTROL
```
- No trading if HR > 100 bpm
- No trading first 30 min after waking
- Mandatory 5-min break after 3 consecutive losses
- Monthly review con coach/psychologist
```

## CONTRARIAN SIGNALS
```
Fear & Greed 0-20: Extreme Fear → BUY
Fear & Greed 80-100: Extreme Greed → SELL
Funding highly positive: Crowded long → Bearish
Funding highly negative: Crowded short → Bullish
```
