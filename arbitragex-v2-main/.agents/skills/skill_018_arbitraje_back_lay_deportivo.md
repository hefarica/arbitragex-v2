# SKILL 018 — Arbitraje back/lay deportivo legal

## 1. Propósito superior
Detectar ineficiencias matemáticas puras (Surebets / Matched Betting / Arbitraje estadístico) entre casas de apuestas tradicionales (Sportsbooks) y Exchanges de apuestas deportivas (Ej. Betfair, Matchbook, Smarkets). Operando bajo total legalidad jurisdiccional, esta skill trata los mercados deportivos como mercados financieros secundarios, aplicando ecuaciones de "Dutching" para asegurar una ganancia neta independientemente del resultado del evento.

## 2. Nivel de conocimiento requerido
Experto en Microestructura de Mercados de Apuestas, Matemática Discreta de Probabilidades y Cumplimiento Normativo (Compliance/KYC). Comprensión profunda de las comisiones de exchange (Betfair commission rate), límites de apuesta (Stake Limits / Gubi limits), y algoritmos contra la cancelación de cuentas (Anti-ban heurístics).

## 3. Capacidades principales
1. Ingesta de cuotas (Odds) decimales en tiempo real desde APIs deportivas legales o raspado estructurado (Web Scraping ético/API de terceros).
2. Cálculo de la ecuación de Arbitraje Deportivo "Surebet": Sumatoria de las inversas de las cuotas es `< 1.0`.
3. Ejecución de emparejamiento (Back en Sportsbook A, Lay en Betting Exchange B).
4. Descuento matemático exacto de la "Commission Rate" (Ej. 2% a 5% en ganancias netas en Betfair).
5. Dimensionamiento óptimo de la apuesta usando "Dutching", redondeando apuestas en el Sportsbook tradicional a números enteros para evadir algoritmos de detección de arbitrajistas.
6. Manejo asíncrono de demoras de validación ("In-play delay" de 5 a 10 segundos).
7. Balanceo de fondos: Identificar cuándo una casa tiene poco saldo pero el exchange sí, ajustando los tamaños al cuello de botella.
8. Identificación de "Palpable Errors" (Errores groseros de cuotas por parte de la casa) y descarte inmediato, ya que las casas los cancelarán (Voided bets) por Términos y Condiciones, exponiendo la pata Lay del exchange.
9. Detección automática de reglas de evento contradictorias (Ej. Reglas de abandono en Tenis difieren entre casas de apuestas).
10. Gestión dinámica de IPs residenciales o Proxies estrictamente limpios y legales por jurisdicción.

## 4. Entradas requeridas
- `sportsbook_odds`: Cuota a favor (Back).
- `exchange_odds`: Cuota en contra (Lay) y liquidez disponible.
- `commission_rate`: Porcentaje de cobro del exchange sobre las ganancias netas.
- `account_limits`: Límite máximo de apuesta permitido por el sportsbook en ese mercado.
- `market_rules`: Identificador de compatibilidad de reglas de la casa (Rule Matcher).

## 5. Salidas esperadas
- `arbitrage_opportunity`: Objeto con el ROI%, evento, cuotas y lados.
- `optimal_stakes`: Tamaños de apuesta (Ej. $100 en Back, $103.5 en Lay Liability).
- `execution_sequence`: Promesas de ejecución.
- `rejection_reason`: Motivo de descarte ("Palpable Error", "Liquidez insuficiente").

## 6. Reglas inmutables
- NUNCA operar un arbitraje si las reglas del mercado entre la Casa A y el Exchange B son divergentes (Ej. En tenis, Casa A paga si hay 1 set completado, Exchange B exige partido completo. Si un jugador se retira, pierdes en A y en B).
- Nunca enviar céntimos (ej. $104.32) a un Sportsbook "soft"; el stake debe redondearse a $105 para simular comportamiento de "apostador recreacional". El tamaño del Lay en el Exchange B absorberá el ajuste matemático.
- No ejecutar oportunidades con un ROI irreal (> 15% en pre-match), ya que el 99% son errores palpables que serán cancelados unilateralmente (Void), arruinando la cobertura.
- Cumplimiento de términos: No eludir prohibiciones regionales, usar APIs bajo T&C permitidos.

## 7. Algoritmos o métodos que debe conocer
- Fórmulas de Arbitraje Deportivo (Dutching bidireccional y tridireccional).
- Cálculo de "Lay Liability" (Riesgo en contra) en mercados de Exchange.
- NLP Básico / Fuzzy Matching para reconciliar nombres de equipos/jugadores entre distintas APIs (Ej. "Man Utd" vs "Manchester United").
- Algoritmos de "Ghosting" o mimetismo recreacional en patrones de apuesta.

## 8. Fórmulas críticas
- **Condición de Arbitraje (Back-Lay)**: `(1 / Cuota_Back) + ( (Cuota_Lay - 1) / (Cuota_Lay - Commission) ) < 1.0`
- **Cálculo del Lay Stake Mínimo (Asegurando beneficio equitativo)**: `Lay_Stake = (Back_Stake * Cuota_Back) / (Cuota_Lay - Commission)`
- **Responsabilidad (Liability) del Lay**: `Lay_Liability = Lay_Stake * (Cuota_Lay - 1)`
- **Beneficio Neto (Si gana Back)**: `(Back_Stake * Cuota_Back) - Back_Stake - Lay_Liability`

## 9. Casos extremos
- Un gol se anota en el milisegundo exacto entre colocar la apuesta Back y lanzar la apuesta Lay, suspendiendo el mercado (Market Suspended).
- La casa de apuestas limita la cuenta en el momento de meter la orden (Acepta $5 en lugar de $100), dejando el Lay expuesto asimétricamente.
- Cancelación post-evento: La casa decide aplicar la "Regla de Error Palpable" 12 horas después, quitándote el dinero ganado, mientras el Exchange B mantiene el dinero perdido.

## 10. Validaciones obligatorias
- PRE: Correr la normalización de cadenas (Fuzzy Matcher) para certificar al 100% que ambos endpoints hablan del mismo partido y mercado (Ej. Más de 2.5 goles totales).
- CÁLCULO: Validar la liquidez del Exchange. Si se necesita apostar $500 pero el libro solo tiene $50 disponibles, rechazar.
- POST: Confirmación de "Bet Matched" en el exchange. Si es "Unmatched" (Parcial), lanzar alarma de cobertura dinámica (Hedging).

## 11. Criterios de aprobación
- Retorno garantizado mayor al 1% neto después de comisiones de exchange.
- El evento es de una liga principal ("Tier 1"), donde los errores palpables son raros.
- Reglas deportivas 100% compatibles confirmadas por la base de datos de "Market Rules".

## 12. Criterios de rechazo
- La oportunidad requiere apostar en la "Casa de Apuestas A" más capital del que el límite dinámico (Stake Limit) permite.
- Evento in-play (en vivo) con alta volatilidad (Baloncesto finalizado, partido con penalti en revisión).

## 13. Riesgos que mitiga
- Riesgo de Unmatched Liability: Evita quedar "desnudo" en una apuesta perdedora, garantizando liquidez y atomicidad a nivel de software.
- "Gubbing" (Limitación de cuenta): Utiliza algoritmos de redondeo de stakes para prolongar la vida útil de las cuentas en casas tradicionales.

## 14. Integración con otras skills
- Requiere Normalización de Datos Multi-fuente (Skill 32) (Fuzzy Name Matching).
- Monitoreado por el Motor de Riesgo (Skill 41).

## 15. Modelo de datos sugerido
```json
{
  "SportsArbitrage": {
    "match_id": "premier_league_ars_mci_1204",
    "market": "Match_Odds_Draw",
    "soft_bookmaker": "Bet365",
    "exchange": "Betfair",
    "back_odds": 3.50,
    "lay_odds": 3.40,
    "exchange_commission_pct": 2.0,
    "back_stake_rounded": 100.0,
    "lay_stake": 103.55,
    "lay_liability": 248.52,
    "guaranteed_profit_usd": 1.48
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Consumidores Webhooks de feeds deportivos (Betradar/Sportradar) combinados con conexiones JSON-RPC/API a Betfair/Smarkets.

## 17. Logs obligatorios
- `[INFO] Arbitrage Detected: Arsenal vs City (Draw). Back 3.50, Lay 3.40. ROI: 1.48%.`
- `[WARN] Fuzzy match low confidence ("Man United" vs "ManUtd U21"). Rejecting to prevent mismatch disaster.`
- `[CRITICAL] Market suspended during Lay placement. Firing emergency hedge script on Smarkets.`

## 18. Métricas obligatorias
- `average_roi_per_arb` (Suele estar entre 1% y 4%).
- `soft_bookmaker_account_health_score` (Medidor de cuánto falta para ser baneado o limitado).
- `execution_latency_ms`.

## 19. Tests unitarios
- Matcher de nombres: Testear que "Rafael Nadal" hace match con "Nadal, R.", pero falla contra "R. Nadal-Vives".
- Math Solver: Probar el redondeo al entero más cercano en la casa, forzando que la pata Lay asuma los decimales y el profit siga siendo positivo en todos los escenarios.
- Palpable Error Trap: Inyectar cuota 8.00 cuando el mercado está en 2.00, el validador debe bloquearlo como error evidente.

## 20. Tests de integración
- Conectar con API Sandbox de Betfair. Ejecutar orden Lay, comprobar el estado `Matched` y la extracción de la Responsabilidad (`Liability`) exacta en cuenta.

## 21. Tests E2E
- El bot navega o consulta API de "Casa Falsa", localiza cuota, verifica Betfair Testnet, calcula stakes con redondeo, ejecuta ambas patas, confirma el PnL y guarda en BD.

## 22. Checklist de producción
- [ ] Base de datos estática de equivalencia de Reglas (Retiro en Tenis, Prórroga en Baloncesto, Gol de Oro).
- [ ] IP Rotator configurado a nivel regional (Cada cuenta deportiva amarrada permanentemente a una IP fija para no saltar alarmas antifraude).
- [ ] Secuenciación: SIEMPRE ejecutar primero la pata "Soft Bookmaker" y SIEMPRE ejecutar la pata Exchange en segundo lugar (El exchange no te limitará, la casa sí).

## 23. Ejemplo de configuración no hardcodeada
```yaml
sports_arb:
  min_roi_net_pct: 1.0
  max_roi_net_pct: 12.0 # Higher is considered Palpable Error trap
  round_soft_bookmaker_stakes: true
  allowed_sports: ["Football_Tier1", "Tennis_ATP", "NBA"]
```

## 24. Ejemplo de pseudocódigo
```python
def calculate_sports_arb(back_odds, lay_odds, commission, back_stake):
    # Rule of thumb for Back/Lay arb
    if (1.0 / back_odds) + ((lay_odds - 1) / (lay_odds - (commission / 100.0))) >= 1.0:
        return ArbResult(viable=False)
        
    # Round the back stake to look like a normal human (e.g., $100 instead of $101.32)
    human_back_stake = round(back_stake)
    
    # Calculate required Lay to balance the profit perfectly
    lay_stake = (human_back_stake * back_odds) / (lay_odds - (commission / 100.0))
    lay_liability = lay_stake * (lay_odds - 1.0)
    
    # Verify profit in both outcomes
    profit_if_back_wins = (human_back_stake * back_odds) - human_back_stake - lay_liability
    profit_if_lay_wins = (lay_stake * (1 - (commission / 100.0))) - human_back_stake
    
    # Take the minimum as guaranteed profit
    min_profit = min(profit_if_back_wins, profit_if_lay_wins)
    
    if min_profit > MIN_PROFIT_THRESHOLD:
        return ArbResult(viable=True, back=human_back_stake, lay=lay_stake, profit=min_profit)
    return ArbResult(viable=False)
```

## 25. Criterio final de excelencia
El sistema exprime márgenes constantes sin arriesgar capital en resultados deportivos, logrando evadir la limitación de cuentas (Gubbing) por meses al simular perfiles de apuestas recreacionales, y evitando categóricamente el 100% de las trampas por errores técnicos de las casas de apuestas.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Cancelación de cuenta unilateral por parte de las casas de apuestas (Riesgo regulatorio humano que el software no puede anular totalmente, solo mitigar).
- Dependencias: Data Normalization (Nombres de equipos), Gestión Proxies/IP.
- Próxima skill: Arbitraje cross-chain controlado (Skill 19).
