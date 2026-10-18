# REA — greyblake/ta-rs

> Ficha REA (Reconocimiento Estructural de Arquitectura) · fase P1 · read-only.
> Fuente: greyblake/ta-rs · licencia **MIT** · 870★ · último push 2024-07-12T15:51:05Z · archived=False
> Destino declarado en el plan: crates de `arbitragex-v2` (backend) + `contracts/` + `frontend/`.
> Regla de fusión: **ADITIVA**. Se extrae el PATRÓN, nunca se copia-pega código.

## 1. Propósito / madurez
MADURO — 870★, MIT, Rust, push 2024-07. Fuente 16 incluye además `twopirllc/pandas-ta`, que HOY responde 404 (repo retirado; continuación mantenida: `xgboosted/pandas-ta-classic`, MIT).
- Technical analysis library for Rust language

## 2. Arquitectura
Colección de indicadores técnicos puros en Rust (SMA, EMA, RSI, MACD, BB, ATR...).

## 3. Algoritmos
Indicadores incrementales/streaming y por lote.

## 4. Esquemas
`Next<T>`, series; sin IO.

## 5. Resiliencia
Sin IO; determinista.

## 6. Tests
Unit + comparación numérica.

## 7. Licencia
`MIT`. Permisiva: se puede citar/adaptar con atribución, pero la doctrina del proyecto manda PATRÓN, no copia.

## 8. Qué se extrae / qué se descarta
- **Se extrae:** PATRÓN de indicadores incrementales con estado (`Next`) + paridad numérica contra implementación de referencia (pandas-ta) como test.
- **Se descarta:** Reimplementar todo; se adopta sólo lo que el math-engine necesita.

## 9. Destino exacto
math-engine (indicadores), token-enricher (features).

## 10. ADR
ADR-018 (candidato): indicadores con estado incremental y test de paridad contra referencia.

## 11. Gates
G-IND-01 (propuesto): cada indicador tiene test de paridad numérica vs referencia (tolerancia declarada).

## 12. Riesgo + revert
- **Riesgo:** Divergencia numérica silenciosa. Revert: indicador tras flag, con test de paridad obligatorio.
