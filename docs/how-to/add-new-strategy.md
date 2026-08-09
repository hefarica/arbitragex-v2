---
title: Add a New Strategy
description: Implement a new strategy as a Rhai cartridge in the ArbitrageX v2 searcher engine.
tags: [strategy, rhai, cartridge, rust]
---

# AGREGAR NUEVA ESTRATEGIA (Mecanismo Real)

El sistema no usa traits de Rust abstractos. Las estrategias son **cartuchos Rhai** ejecutados por el motor de búsqueda (`backend/searcher-rs`). Existen 264+ cartuchos en `backend/searcher-rs/cartridges/strategies/` con la convención `mev_XX_NNN_nombre.rhai`.

## Pasos de Implementación

1. **Crear Cartucho:**
   Crea un archivo en `backend/searcher-rs/cartridges/strategies/mev_XX_NNN_nombre.rhai`.

2. **Implementar Contrato:**
   Implementa la interfaz obligatoria del loader:
   - `init_strategy(config)`
   - `evaluate_opportunity(state, context)`
   - `build_payload(opportunity_result)`

   *(Confirmar firmas exactas en `backend/searcher-rs/src/cartridge_loader.rs` al implementar.)*

3. **Carga y Validación:**
   El sistema valida la firma vía `cartridge_loader.rs` (junto a `cartridge_boot.rs`) al bootear.

4. **Verificación:**
   - Tuning vía variables de entorno `ARBX_*` / `TradingConfigState`.
   - Verificación en tiempo real vía `paper-shadow` y Redis stream `arbx:opps:detected` (sin mocks — RULE 00).

> **Nota:** No usar `test_fixture()` ni dependencias de crates externos (`crates/*`). Usar solo las APIs nativas de Rhai y el backend Rust.
