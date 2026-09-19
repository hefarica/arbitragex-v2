# DISEÑO FINAL — FEE-TIER-AWARE-QUOTING (WO-06)
> Autor: Hermes/Sancho (runs c3f28ef2 + 1ebce0bd, 2026-09-17). Verificado contra código leído.
> Cross-examiner de rama (A/B/C/D) sigue en curso del lado de Hermes; la métrica
> arbx_v3_fee_resolution_total{resolution} resuelve la rama EN PRODUCCIÓN (no bloquea).

## DEFECTO RAÍZ (verificado)
QuoterV2.quoteExactInputSingle NO recibe pool — deriva por factory.getPool(t0,t1,fee).
Fee viaja en PIPS uint24. Fee sin pool real → revert → v3_quote_unavailable.
- dex_engine.rs:484-504 construye PoolRef con fee_bps del impact set
- state_projector.rs:329: `pool.fee_bps.unwrap_or(500)` — COTIZACIÓN CIEGA al 500
- amm_math.rs:373-384: codifica fee_bps directo como uint24 (unidades OK, valor ciego)
- Catálogo autoritativo YA EXISTE: Redis `arbx:pool_index_v3:chain:symA:symB` →
  Vec<V3PoolInfo> (reserves.rs:74-100/278-288), poblado por pool_sync_worker.rs:1249,
  ya consumido por scanner.rs:1818-1824. dex_engine NO lo consulta. Ese es el hueco.

## DIFF MÍNIMO
1. Archivo NUEVO `backend/searcher-rs/src/v3_fee_catalog.rs`:
   - `V3FeeCatalog` RwLock<HashMap by_pool addr→fee_pips, by_pair (t0,t1)→BTreeSet>
   - `load_from_redis` (SCAN glob arbx:pool_index_v3:chain:* — precedente price_worker.rs:672-688)
   - `refresh()` en boot + cada ciclo pool_sync (o timer 60s)
   - `fee_for_pool`, `tiers_for_pair`, `resolve(pool, offered) -> FeeResolution`,
     `record_observed(pool, fee)` (alta pasiva post-RPC exitosa; cubre desync catálogo)
   - `enum FeeResolution { Catalog(u32), Mismatch{offered,catalog}, NotCatalogued }`
2. Intercepción ÚNICA: state_projector.rs `project_v3_quote` (:317) — embudo de TODAS
   las quotes V3 (dex_engine:502, size_optimizer:571). ELIMINAR `unwrap_or(500)`:
   - Catalog(f) → f; Mismatch → usa catalog + métrica+warn (resuelve rama A/B en prod);
     NotCatalogued → Err, SIN RPC.
   - Pre-RPC: tiers_for_pair vacío → Err(PairHasNoV3Pools) (fail-honest, cero RPC).
   - Post-quote OK → record_observed.
   - `enum ProjectV3Error { PoolNotCatalogued, PairHasNoV3Pools, ProviderUnavailable, QuoteFailed(String) }`
   - `project_v3_quote_checked(...) -> Result<V3VirtualQuote, ProjectV3Error>`
   - project_v3_quote existente = wrapper que aplana a Option (call sites siguen compilando).
   - Wiring: StateProjector::new(..., fee_catalog: Arc<V3FeeCatalog>); tests inyectan en memoria.
3. Labels honestos: rejection_reason "v3_pool_not_catalogued" y "v3_pair_no_pools"
   (NUNCA más bajo v3_quote_unavailable — reservado a fallo real de provider).
4. Métricas R8: arbx_v3_fee_catalog_pools (gauge), arbx_v3_fee_resolution_total{resolution},
   outcome "rpc_tier_revert" en arbx_v3_quote_total. Force-register en init.
Fuente de verdad: Redis pool_index_v3 (contrato wire ya consumido por scanner — UNA fuente;
PG queda como origen último vía el worker existente; NO añadir lector PG nuevo).

## TESTS (vector independiente, PIPS uint24)
T1 PIN ENCODING: fee=3000 → calldata word == 0x0bb8; fee=5 → 0x05 (ancla pips vs ABI real;
no-regresión del precedente bps-causaba-reverts).
T2 RESOLUCIÓN: catálogo {P→3000}; fee_bps:None → Catalog(3000) (antes: 500 ciego);
Some(100) → Mismatch y cotiza CON 3000.
T3 PAR SIN V3: Err(PairHasNoV3Pools), mock provider PANIQUEA si lo invocan (cero RPC).
T4 TIERS EXÓTICOS: catálogo {1,5} → mock captura EXACTO 1 y 5; tier 100 jamás pedido.
T5 POOL NO CATALOGADA: Err(PoolNotCatalogued), cero RPC.
T6 LABEL EXACTO: rejection_reason "v3_pool_not_catalogued" (no v3_quote_unavailable).
T7 VECTOR CAST ANCLADO: WETH/USDC (500+3000) calldata byte-exacto precomputado
(estilo anclaje SIM-FUND-01b).

## CRITERIOS DE ACEPTACIÓN
1. fmt + clippy -D warnings + cargo test -p searcher-rs --lib COMPLETO verde.
2. 15 min post-deploy: v3_quote_unavailable cae >90% (14.473 → <1.450).
3. fee_resolution decide rama: mismatch≫ → fee hardcode (B); not_catalogued≫ → backfill (C);
   ambos~0 y unavailable alto → size_optimizer (D).
4. accepts > 0 con mercado real en 15 min (secundario: candidatos a SizeOptimizer > 0).
5. cache_neg_hit de 74% → <10% de la demanda.
6. Fail-honest total (RULE 00/R8).

## SEGUNDA OPINIÓN RECHAZOS MENORES (consolidada)
- non_positive_profit 2.844/15min: esperable (probe 1e18 ≈ $0.30-2 brutos ≪ min_ev_usd=25).
  SALVEDAD (pregunta a del verificador): ¿el gate se aplica al probe ANTES del sizing
  variable? Si sí → bug de ORDEN DE GATES (probe = cota inferior del óptimo), revisar
  size_optimizer.rs:119/:3136.
- spot_product_le_one 2.580/15min: estructural S=γ³·∏(...)≈0.991<1 en mercado eficiente —
  el gate funciona. RIESGO LATENTE (pregunta b): si spot_product usa fee 30 (V2 bps) en
  ciclos con hops V3 (pips) → γ subestimada → FALSOS ACEPTES (dirección opuesta).
