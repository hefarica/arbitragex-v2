# SKILL: Bayesian Inference for Price Prediction
**Level:** PhD Bayesian Statistics | Probabilistic Machine Learning
**Specialty:** Posterior Distribution Modeling & Uncertainty Quantification

## AGENT DIRECTIVE
Nunca predigas un punto. Predice una **distribución**.

## BAYESIAN UPDATE (Conjugate Prior)
```python
# Normal-Inverse-Gamma
κ_n = κ_0 + n
μ_n = (κ_0*μ_0 + n*x̄) / κ_n
α_n = α_0 + n/2
β_n = β_0 + 0.5*Σ(x_i - x̄)² + (κ_0*n*(x̄ - μ_0)²)/(2*κ_n)
```

## GAUSSIAN PROCESS
```python
kernel = ConstantKernel(1.0) * RBF(1.0) + WhiteKernel(1e-5)
gp = GaussianProcessRegressor(kernel=kernel, n_restarts_optimizer=10)
gp.fit(X, y)
y_pred, sigma = gp.predict(X_new, return_std=True)
# Trade solo si |y_pred| > 2*sigma
```

## UNCERTAINTY QUANTIFICATION
```
Epistemic: "No sé porque no tengo datos" → Reduce con más datos
Aleatoric: "No sé porque el mercado es aleatorio" → Irreducible
Decision: Si epistemic > aleatoric → No operes
```
