# SKILL 009 — Probabilidad bayesiana para oportunidad real

## 1. Propósito superior
Filtrar las ilusiones del mercado ("Ghost Opportunities") actualizando constantemente la creencia del sistema sobre si una oportunidad de arbitraje es verdadera o un artefacto temporal. Aplica el Teorema de Bayes para procesar el histórico de fallos y éxitos, ajustando la probabilidad *a priori* en tiempo real con la nueva evidencia (latencia, volatilidad, exchange), actuando como un escáner de "Fake Liquidity" ultra-inteligente.

## 2. Nivel de conocimiento requerido
PhD/Máster en Estadística Bayesiana, Machine Learning y Data Science Financiero. Capacidad para modelar probabilidades condicionales, funciones de verosimilitud (Likelihood), Priors conjugados (Distribuciones Beta/Dirichlet), y calibración de confianza en inferencia de modelos bajo incertidumbre.

## 3. Capacidades principales
1. Mantenimiento de una Distribución Prior (creencia base) del Win-Rate de arbitraje general.
2. Cálculo de Verosimilitud (Likelihood) ponderando factores específicos: historial del par, exchange maker/taker, hora del día, y congestión de red.
3. Actualización iterativa del Posterior Prob (Probabilidad a posteriori) con cada resultado de trade ejecutado (Éxito, Fallo Parcial, Fallo Total).
4. Detección de Spoofing: Asignación de baja probabilidad real a órdenes masivas en el libro que aparecen y desaparecen repetidamente.
5. Calibración de la confianza basada en la Ley de los Grandes Números (Más datos históricos = Prior más estricto).
6. Creación de vectores de features para cada señal (ej. `[exchange_A, exchange_B, par, volatilidad_reciente, latencia_RPC]`).
7. Decaimiento Temporal del Conocimiento (Time Decay): Olvidar heurísticas bayesianas de hace 3 meses porque la microestructura del mercado cambia.
8. Bloqueo probabilístico: Rechazar un ROI de 2% si la probabilidad de ser real es del 5%.
9. Mapeo de "Exchanges Mentirosos" (Wash trading networks) rebajando automáticamente su peso de confiabilidad bayesiana.
10. Inferencia sobre "Bait Orders" (Órdenes señuelo) puestas por Market Makers institucionales.

## 4. Entradas requeridas
- `arbitrage_signal`: Objeto completo con la oportunidad matemática (Skill 1).
- `execution_history`: Base de datos de los últimos N trades y sus resultados (Win/Loss/Timeout).
- `market_context`: Nivel de volatilidad, hora, día de la semana.
- `order_book_behavior`: Tasa de cancelación vs Tasa de llenado del nivel L2 donde se encuentra la oportunidad.

## 5. Salidas esperadas
- `true_opportunity_probability`: Probabilidad % de que la liquidez vista sea capturable (ej. 12.5%).
- `bayes_approved`: Booleano, true si la probabilidad supera el umbral de riesgo dinámico.
- `confidence_score`: Nivel de certeza de la predicción estadística.
- `penalty_factors`: Explicación de qué factor bajó la probabilidad (ej. "Exchange B historically ghosts orders in highly volatile conditions").

## 6. Reglas inmutables
- Toda oportunidad matemática perfecta de un exchange C-tier (baja reputación) debe iniciar con un Prior extremadamente bajo.
- Los resultados de operaciones en producción deben realimentar inmediatamente el motor bayesiano. Un fallo baja el Posterior, un éxito lo sube.
- No procesar inferencias que no tengan suficiencia estadística. Usar "Laplace Smoothing" para pares de trading nuevos sin historial.
- El modelo no debe confiar ciegamente en sí mismo; debe requerir re-entrenamiento o reseteo de Priors regularmente.

## 7. Algoritmos o métodos que debe conocer
- Teorema de Bayes: `P(A|B) = [P(B|A) * P(A)] / P(B)`
- Beta-Binomial Update (Conjugate Prior para variables binarias Éxito/Fallo).
- Clasificador Naive Bayes adaptado a series temporales financieras de alta frecuencia.
- Análisis ROC-AUC para evaluar la calidad predictiva del filtro bayesiano.

## 8. Fórmulas críticas
- **Actualización Bayesiana (Beta Distribution)**: `Nuevo_Alpha = Alpha_Previo + Exitos; Nuevo_Beta = Beta_Previo + Fallos`.
- **Probabilidad Esperada**: `P(Real) = Nuevo_Alpha / (Nuevo_Alpha + Nuevo_Beta)`.
- **Decaimiento de Memoria (Exponential Forgetting)**: `Alpha_T = Alpha_{T-1} * decay_factor`.
- **Umbral Dinámico de Aprobación**: Si `P(Real) * ROI_Esperado > Costo_Gas_Ponderado`.

## 9. Casos extremos
- Exchange cambia su motor de matching, volviendo las órdenes repentinamente más rápidas y rompiendo el modelo (El Prior histórico daña la evaluación actual).
- "Cisne Negro" (Black Swan): Comportamiento de mercado no visto previamente (El Prior no sabe cómo responder, el Confidence Score debe caer a cero).
- Wash trading algorítmico extremo en un DEX que infla la verosimilitud de operaciones que en la práctica siempre sufren MEV Sandwich attack.

## 10. Validaciones obligatorias
- PRE: Validar que el historial de trades provisto no esté corrupto ni desfasado temporalmente.
- CÁLCULO: Validar el tamaño de la muestra (`Alpha + Beta`). Si es menor a 10, usar el "Global System Prior".
- POST: Si la probabilidad es < 50%, la oportunidad se bloquea independientemente del ROI, para proteger el "Win Rate" del algoritmo.

## 11. Criterios de aprobación
- `true_opportunity_probability >= bayesian_confidence_threshold_config`.
- `confidence_score` (basado en el número de muestras) es robusto.

## 12. Criterios de rechazo
- La inferencia arroja una altísima probabilidad de "Phantom Liquidity" (El nivel de order book ha cancelado el 99% de las órdenes en el último minuto).
- Oportunidad en un exchange marcado con `P(Fake) > 80%`.

## 13. Riesgos que mitiga
- Operar espejismos matemáticos.
- Atrapar el cuchillo cayendo (Falling Knife): Ver un gap enorme de precio asumiendo que es ineficiencia, cuando en realidad es que un exchange sabe algo (hackeo) y los bots apagaron la liquidez real.

## 14. Integración con otras skills
- Funciona como filtro inteligente pre-ejecución, actuando en conjunto con la Optimización Estocástica (Skill 6).
- Manda feedback a Microestructura de Mercado (Skill 11).

## 15. Modelo de datos sugerido
```json
{
  "BayesianInference": {
    "opportunity_hash": "abc-123",
    "prior_probability": 0.45,
    "likelihood_factor": 0.12,
    "posterior_probability": 0.08,
    "confidence_samples": 412,
    "bayes_approved": false,
    "primary_penalty": "historical_mev_sandwich_probability"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Sub-sistema en memoria que expone el método `assess_reality_probability(Signal)`.
- Worker secundario que computa el `Beta-Update` de forma asíncrona cada vez que la DB registra un resultado de trade.

## 17. Logs obligatorios
- `[INFO] Bayes Update: Route A->B success rate adjusted to 68% after recent partial fill.`
- `[WARN] Bayes Filter rejected mathematical opportunity. Posterior probability of ghost liquidity: 92%.`

## 18. Métricas obligatorias
- `bayesian_filter_latency_us`
- `bayesian_accuracy_rate` (Si bloqueó algo que "parecía real", un proceso en la sombra puede trackear si la orden hubiera sido fillada o no asumiendo read-only del book futuro).
- `fake_liquidity_detections_count`

## 19. Tests unitarios
- Beta Update: Inyectar 10 fallos seguidos; la probabilidad posterior debe desplomarse asintóticamente.
- Time Decay: Avanzar el reloj virtual 30 días, el modelo debe "olvidar" sesgos antiguos y volver a tender al prior global.
- Laplace Smoothing: Señal de un exchange nuevo con 0 historial. Debe retornar la probabilidad base neutral, no crashear.

## 20. Tests de integración
- Sincronización entre el motor de ejecución (que reporta fallos) y la base de conocimiento bayesiana en tiempo real vía Pub/Sub (Redis).

## 21. Tests E2E
- Simular un exchange con 100% de oportunidades falsas. Tras los primeros ~3 intentos fallidos (pérdidas controladas de gas), el filtro bayesiano debe aprender y bloquear la ruta por completo (Sistema Auto-Curativo).

## 22. Checklist de producción
- [ ] Implementar un proceso offline/nightly que recalcule las matrices globales de probabilidad para liberar al motor en tiempo real.
- [ ] Separación de contextos: Un modelo bayesiano para Bull Market y otro distinto para Bear Market.
- [ ] Monitoreo estricto contra "Concept Drift" (Degradación de modelo).

## 23. Ejemplo de configuración no hardcodeada
```yaml
bayesian_engine:
  base_global_prior: 0.30
  acceptance_threshold: 0.65
  laplace_smoothing_factor: 1.0
  time_decay_half_life_hours: 48
```

## 24. Ejemplo de pseudocódigo
```python
class BayesianFilter:
    def __init__(self, global_prior):
        self.stats = {} # {route_id: {'alpha': 1, 'beta': 1}}
        
    def evaluate(self, route_id, mathematical_roi, volatility_score):
        route_stats = self.stats.get(route_id, {'alpha': 1, 'beta': 1}) # Laplace
        
        # Expected value of Beta distribution
        historical_prob = route_stats['alpha'] / (route_stats['alpha'] + route_stats['beta'])
        
        # Adjust with local likelihood (e.g., high volatility lowers probability)
        likelihood = self._calculate_likelihood(volatility_score)
        
        # Simplified Bayesian update
        posterior_prob = (likelihood * historical_prob) / self.evidence_factor
        
        approved = posterior_prob > CONFIG.min_bayes_prob
        return approved, posterior_prob
        
    def update_feedback(self, route_id, trade_success):
        if route_id not in self.stats:
            self.stats[route_id] = {'alpha': 1, 'beta': 1}
            
        if trade_success:
            self.stats[route_id]['alpha'] += 1
        else:
            self.stats[route_id]['beta'] += 1
            
        # Apply time decay periodically in a background thread
```

## 25. Criterio final de excelencia
El modelo asimila y aprende de un fallo estructural (ej. un exchange subió un API rate invisible) en menos de 3 intentos, protegiendo todo el capital futuro mediante la adaptación matemática inmediata de sus expectativas.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Sub-entrenamiento inicial que genere Falsos Positivos temporales (Mitigable empezando con sizes minúsculos "Exploration phase").
- Dependencias: Base de datos de ejecución histórica (Skill 57), Analytics.
- Próxima skill: Análisis de sensibilidad financiera (Skill 10).
