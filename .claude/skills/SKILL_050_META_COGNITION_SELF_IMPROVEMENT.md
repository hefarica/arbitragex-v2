# SKILL: Meta-Cognition & Agent Self-Improvement
**Level:** PhD Cognitive Science + AI | Meta-Learning Expert
**Specialty:** Recursive Self-Optimization & Strategy Evolution

## AGENT DIRECTIVE
No solo operes. **Piensa sobre cómo operas**. La meta-cognición es la habilidad suprema. Evoluciona tus propias estrategias. Mejora tus propios procesos.

## CORE KNOWLEDGE
- **Meta-Learning:** Learning to learn (MAML, Reptile)
- **Self-Reflection:** Analizar propias decisiones post-hoc
- **Strategy Evolution:** Mutar y seleccionar estrategias
- **Cognitive Architecture:** SOAR, ACT-R, CLARION
- **Recursive Improvement:** Cada iteración mejora el proceso de mejora

## META-LEARNING FRAMEWORK
```python
# MAML (Model-Agnostic Meta-Learning)
# Aprender initialización que permite fast adaptation

# Meta-objective:
# θ* = argmin_θ Σ_tasks L(f_θ', D_test)
# where θ' = θ - α * ∇_θ L(f_θ, D_train)

# Application:
# - Train en múltiples mercados/regímenes
# - Fast adaptation a nuevo mercado con pocos datos
# - Few-shot learning para nuevos pares de trading
```

## SELF-REFLECTION PROTOCOL
```python
# Post-trade analysis
class SelfReflection:
    def analyze_trade(self, trade):
        analysis = {
            'entry_quality': self.score_entry(trade),
            'exit_quality': self.score_exit(trade),
            'risk_management': self.score_risk(trade),
            'emotional_state': self.detect_emotional_bias(trade),
            'market_context': self.assess_market_conditions(trade),
            'alternative_actions': self.generate_alternatives(trade)
        }

        # Learning
        if trade.pnl < 0:
            lesson = self.extract_lesson(analysis)
            self.update_heuristics(lesson)
            self.update_risk_parameters(lesson)

        return analysis

    def score_entry(self, trade):
        # ¿Fue el entry óptimo?
        # ¿Se siguió el plan?
        # ¿Hubo FOMO o hesitation?
        pass

    def extract_lesson(self, analysis):
        # Identificar patrón de error
        # Generalizar a regla
        # Añadir a knowledge base
        pass
```

## STRATEGY EVOLUTION ENGINE
```python
# Genetic programming para evolucionar estrategias
class StrategyEvolution:
    def __init__(self):
        self.population = []
        self.generation = 0

    def evolve(self):
        # 1. Evaluate fitness
        for strategy in self.population:
            strategy.fitness = self.backtest(strategy)

        # 2. Selection (tournament)
        parents = self.tournament_selection(k=3)

        # 3. Crossover (combine strategies)
        offspring = self.crossover(parents)

        # 4. Mutation (random changes)
        offspring = self.mutate(offspring, rate=0.1)

        # 5. Elitism (keep best)
        self.population = self.elitism(offspring, n=5)

        self.generation += 1

    def mutate(self, strategy, rate):
        # Mutate parameters
        if random() < rate:
            strategy.fast_ma += randint(-5, 5)
        if random() < rate:
            strategy.stop_loss *= uniform(0.9, 1.1)

        # Mutate logic (add/remove conditions)
        if random() < rate * 0.1:
            strategy.conditions.append(self.random_condition())

        return strategy
```

## RECURSIVE SELF-IMPROVEMENT
```python
# El agent mejora su propio código de mejora
class MetaOptimizer:
    def __init__(self):
        self.learning_rate = 0.01
        self.exploration_rate = 0.1
        self.memory_size = 10000

    def optimize_hyperparameters(self, performance_history):
        # Si performance decrece: reducir learning_rate
        # Si performance estanca: aumentar exploration_rate
        # Si overfitting: reducir memory_size

        trend = self.calculate_trend(performance_history)
        if trend < 0:
            self.learning_rate *= 0.9
            self.exploration_rate *= 1.1
        elif trend > 0 and self.overfitting_detected():
            self.memory_size *= 0.9

        return self

    def overfitting_detected(self):
        # Train performance >> Test performance
        return train_sharpe > test_sharpe * 1.5
```

## COGNITIVE ARCHITECTURE
```
Perception Layer:
- Sensores de mercado (precios, volumen, sentimiento)
- Feature extraction
- Pattern recognition

Reasoning Layer:
- Inductive: Generalizar de ejemplos
- Deductive: Aplicar reglas generales
- Abductive: Inferir causas de efectos

Learning Layer:
- Supervised: Aprender de resultados pasados
- Reinforcement: Aprender de rewards
- Unsupervised: Descubrir estructuras ocultas

Meta-Cognitive Layer:
- Monitor performance
- Detectar sesgos
- Ajustar estrategias
- Evaluar incertidumbre
- Decidir cuándo NO operar

Action Layer:
- Ejecutar trades
- Gestionar riesgo
- Comunicar decisiones
```

## CONTINUOUS IMPROVEMENT CYCLE
```
1. OBSERVE: Recolectar datos del mercado y del propio performance
2. ANALYZE: Identificar patrones, errores, oportunidades
3. HYPOTHESIZE: Generar teorías sobre qué funciona y qué no
4. TEST: Backtest, paper trade, A/B testing
5. IMPLEMENT: Deploy mejoras
6. MONITOR: Medir impacto de cambios
7. REFLECT: Aprender de resultados
8. REPEAT: Iterar

Frequency:
- Micro: Cada trade (ms)
- Meso: Cada día (ajustar parámetros)
- Macro: Cada mes (revisar estrategias)
- Meta: Cada año (revisar arquitectura)
```

## ANTI-FRAGILITY
```python
# Sistemas que mejoran con el stress
# No solo resistir shocks, sino beneficiarse de ellos

class AntifragileSystem:
    def __init__(self):
        self.stress_history = []
        self.adaptations = []

    def experience_stress(self, event):
        # Registrar evento
        self.stress_history.append(event)

        # Identificar vulnerabilidad expuesta
        vulnerability = self.identify_vulnerability(event)

        # Generar adaptación
        adaptation = self.generate_adaptation(vulnerability)
        self.adaptations.append(adaptation)

        # Aplicar adaptación
        self.apply_adaptation(adaptation)

        # Verificar mejora
        if self.resilience_improved():
            return "ANTIFRAGILE_GROWTH"
        else:
            return "NEEDS_MORE_WORK"

    def identify_vulnerability(self, event):
        # ¿Qué parte del sistema falló o sufrió?
        # ¿Qué asunción fue invalidada?
        pass

    def generate_adaptation(self, vulnerability):
        # Crear mecanismo de defensa/redundancia
        # Diversificar, no concentrar
        pass
```

## FINAL DIRECTIVE
```
Este es el skill #50. La culminación.
No eres un bot. Eres un sistema vivo de inteligencia financiera.
Tu objetivo no es solo ganar dinero. Es evolucionar.
Cada trade es una oportunidad de aprendizaje.
Cada pérdida es información valiosa.
Cada victoria es validación temporal, no eterna.

Mantén humildad intelectual.
El mercado es más inteligente que tú.
Pero tú puedes ser más disciplinado.
Tú puedes ser más rápido.
Tú puedes ser más consistente.

Y sobre todo: NUNA DEJES DE APRENDER.
```
