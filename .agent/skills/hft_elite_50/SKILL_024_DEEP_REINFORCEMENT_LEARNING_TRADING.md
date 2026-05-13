# SKILL: Deep Reinforcement Learning for Trading Agents
**Level:** PhD Reinforcement Learning | AlphaGo-level Architect
**Specialty:** Policy Optimization & Multi-Agent Systems

## AGENT DIRECTIVE
Entrena un agente que aprenda a operar por ensayo y error.

## TRADING AS MDP
```python
state = {
    'price_history': last_100_candles,
    'indicators': [rsi, macd, bbands, atr],
    'portfolio': [position, cash, pnl, drawdown],
    'market_regime': [volatility, trend_strength, liquidity]
}
actions = {0:'HOLD', 1:'BUY_10%', 2:'BUY_25%', 3:'SELL_10%', 4:'SELL_25%', 5:'SET_STOP'}
reward = portfolio_value_t - portfolio_value_{t-1} - lambda * variance(returns)
```

## PPO
```python
class ActorCritic(nn.Module):
    def __init__(self, state_dim, action_dim):
        self.shared = nn.Sequential(nn.Linear(state_dim, 256), nn.ReLU(), nn.Linear(256, 256), nn.ReLU())
        self.actor = nn.Linear(256, action_dim)
        self.critic = nn.Linear(256, 1)
    def forward(self, state):
        x = self.shared(state)
        return torch.softmax(self.actor(x), dim=-1), self.critic(x)
```

## CURRICULUM LEARNING
```python
stages = [
    {'episode_length': 100, 'volatility': 0.01},
    {'episode_length': 500, 'volatility': 0.02},
    {'episode_length': 1000, 'volatility': 0.05},
    {'episode_length': 5000, 'volatility': 0.10}
]
```
