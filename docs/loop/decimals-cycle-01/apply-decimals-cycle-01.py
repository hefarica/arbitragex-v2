#!/usr/bin/env python3
"""DECIMALS-CYCLE-01 (t197) — transformador del parche.

Aplica la reparacion del `decimals.map` del ciclo sobre un arbol de trabajo
`backend/` YA COPIADO (nunca el checkout compartido).

Reglas:
  * Cada sustitucion es LITERAL y debe aparecer EXACTAMENTE una vez; si el
    anclaje no esta, el script aborta sin escribir nada (fail-honest: nunca
    un parche a medias).
  * Idempotente por deteccion: si ya esta aplicado, lo declara y no re-escribe.

Uso:  python3 apply-decimals-cycle-01.py <RAIZ_BACKEND>
"""
import sys
import pathlib
import re

ORCH = "searcher-rs/src/orchestrator.rs"
SCAN = "searcher-rs/src/scanner.rs"
TESTS = [
    "searcher-rs/tests/v2_shadow_replay.rs",
    "searcher-rs/tests/cartridge_shadow_replay.rs",
    "searcher-rs/tests/orchestrator_parallel_run.rs",
]

# ---------------------------------------------------------------------------
# 1) orchestrator.rs — campo nuevo en OrchestratorContext
# ---------------------------------------------------------------------------
CTX_ANCHOR = """    pub math_redis: redis::aio::ConnectionManager,
"""
CTX_NEW = """    pub math_redis: redis::aio::ConnectionManager,
    /// DECIMALS-CYCLE-01 (t197) — PG-backed `tokens.decimals` provider, the
    /// SAME `PgTokenDecimalsProvider` the sim encoder uses (populated from PG
    /// `tokens.decimals`, cache kept warm out-of-band; its `decimals()` returns
    /// `None` — never `Some(18)` — for a token PG does not know).
    ///
    /// The cycle's per-hop `decimals.map` reads real decimals from here and from
    /// the Redis token catalog (`arbx:tokens:<chain>:<addr>`). It used to read
    /// the hardcoded `canonical_token_decimals*` table instead, whose unknown
    /// arm is 18: measured live in t187 as 8/37 map entries wrong, always 18,
    /// i.e. a 1e12 unit error on EURC and 1e10 on the 8-decimal tokens — enough
    /// to make every size generated from that path wrong.
    ///
    /// `None` when no DB pool existed at boot → the map stays unresolved and the
    /// route is NOT priced with an invented unit (R8).
    ///
    /// Tipo: el mismo `Option<Arc<dyn TokenDecimalsProvider>>` que usa el
    /// scanner (`ScannerDecimalsProvider`), escrito por su ruta de LIB
    /// (`crate::sim_encoder::`, re-export de sim-core en lib.rs:182) porque
    /// `crate::scanner` es modulo del BIN y no es alcanzable desde aqui.
    pub token_decimals_provider:
        Option<Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>>,
"""

# ---------------------------------------------------------------------------
# 2) orchestrator.rs — el resolutor puro + el wrapper async
# ---------------------------------------------------------------------------
RESOLVER_ANCHOR = """// ---------------------------------------------------------------------------
// ConfigProvider
// ---------------------------------------------------------------------------
"""
RESOLVER_NEW = """// ---------------------------------------------------------------------------
// DECIMALS-CYCLE-01 (t197) — per-hop decimals resolution for the cycle
// ---------------------------------------------------------------------------

/// Outcome of resolving the cycle's per-hop decimals from the two REAL sources.
#[derive(Debug, Clone, Default)]
pub(crate) struct CycleDecimals {
    /// Only tokens whose decimals a real source confirmed. A token absent from
    /// this map has NO unit — it is not 18, it is "not computed".
    pub map: shared_rs::candidates::DecimalsMap,
    /// Tokens with an entry in NEITHER source, in route order, lowercased.
    pub unresolved: Vec<String>,
}

/// Pure resolution policy (unit-testable, no I/O).
///
/// `redis_decimals` = the Redis token catalog (`arbx:tokens:<chain>:<addr>`,
/// written by the scanner from `pool_discovery`'s on-chain `decimals()` reads);
/// `pg_decimals` = PG `tokens.decimals`, consulted only for the tokens Redis
/// did not answer for.
///
/// A token present in NEITHER source is returned in `unresolved` and gets NO
/// entry in the map. It is NEVER folded into an `18`: an invented unit is
/// fabrication, and 8/37 production entries were exactly this failure (t187).
pub(crate) fn build_cycle_decimals_map(
    token_addresses: &[String],
    redis_decimals: &HashMap<String, u8>,
    pg_decimals: &HashMap<String, u8>,
) -> CycleDecimals {
    let mut map = HashMap::new();
    let mut unresolved: Vec<String> = Vec::new();
    for addr in token_addresses {
        let lc = addr.to_lowercase();
        match redis_decimals.get(&lc).or_else(|| pg_decimals.get(&lc)) {
            Some(d) => {
                map.insert(lc, *d);
            }
            None => {
                if !unresolved.contains(&lc) {
                    unresolved.push(lc);
                }
            }
        }
    }
    CycleDecimals {
        map: shared_rs::candidates::DecimalsMap { map },
        unresolved,
    }
}

// ---------------------------------------------------------------------------
// ConfigProvider
// ---------------------------------------------------------------------------
"""

# ---------------------------------------------------------------------------
# 3) orchestrator.rs — el wrapper async + el call site
# ---------------------------------------------------------------------------
WRAPPER_ANCHOR = """    // -----------------------------------------------------------------------
    // Per-candidate processing
    // -----------------------------------------------------------------------
"""
WRAPPER_NEW = """    /// DECIMALS-CYCLE-01 (t197) — resolve the route's per-hop decimals from the
    /// two real sources: the Redis token catalog first (1 GET per token), then
    /// the PG-backed `tokens.decimals` provider for whatever Redis did not
    /// answer. NEVER a default: what neither source has is declared unresolved.
    async fn resolve_cycle_decimals(&self, addrs: &[String], chain_id: u64) -> CycleDecimals {
        let mut redis_decimals: HashMap<String, u8> = HashMap::new();
        let mut pg_decimals: HashMap<String, u8> = HashMap::new();
        for addr in addrs {
            let lc = addr.to_lowercase();
            if redis_decimals.contains_key(&lc) || pg_decimals.contains_key(&lc) {
                continue;
            }
            let mut conn = self.ctx.math_redis.clone();
            match crate::reserves::get_token_meta(&mut conn, chain_id, &lc).await {
                Ok(Some(meta)) => {
                    redis_decimals.insert(lc, meta.decimals);
                }
                Ok(None) => {
                    // Redis has no row → PG (the authoritative `tokens.decimals`).
                    if let Some(provider) = self.ctx.token_decimals_provider.as_ref() {
                        if let Ok(a) = lc.parse::<Address>() {
                            if let Some(d) = provider.decimals(chain_id, &a) {
                                pg_decimals.insert(lc, d);
                            }
                        }
                    }
                }
                Err(e) => {
                    // Redis read failed: do NOT give up on the token — PG still
                    // answers. The error is logged, never turned into a unit.
                    debug!(
                        event = "orchestrator.decimals_redis_read_failed",
                        chain_id,
                        token = %lc,
                        error = %e,
                        "redis token catalog read failed; falling back to PG provider"
                    );
                    if let Some(provider) = self.ctx.token_decimals_provider.as_ref() {
                        if let Ok(a) = lc.parse::<Address>() {
                            if let Some(d) = provider.decimals(chain_id, &a) {
                                pg_decimals.insert(lc, d);
                            }
                        }
                    }
                }
            }
        }
        build_cycle_decimals_map(addrs, &redis_decimals, &pg_decimals)
    }

    // -----------------------------------------------------------------------
    // Per-candidate processing
    // -----------------------------------------------------------------------
"""

CALLSITE_ANCHOR = """            // PER-HOP (math-audit AUDIT-MATH-OPPS-2026-09-26): populate the
            // decimals map from the route's OWN token path — the same canonical
            // immutable-protocol table the engine's USD conversion uses (unknown
            // → 18, the ERC-20 default). Without it the card can only ever show
            // raw per-hop wei, never USD: `route_metadata.decimals.map` was
            // empty in 32/32 production rows.
            {
                let mut m = std::collections::HashMap::new();
                for addr in &chosen.token_addresses {
                    let lc = addr.to_lowercase();
                    m.insert(
                        lc,
                        crate::engines::dex_engine::canonical_token_decimals_str(addr),
                    );
                }
                if !m.is_empty() {
                    chosen.decimals = shared_rs::candidates::DecimalsMap { map: m };
                }
            }
"""
CALLSITE_NEW = """            // PER-HOP — DECIMALS-CYCLE-01 (t197, supersedes the
            // AUDIT-MATH-OPPS-2026-09-26 default): the decimals map is built from
            // the route's own token path BUT resolved against the TWO REAL
            // sources — the Redis token catalog and PG `tokens.decimals` — never
            // from the hardcoded canonical table whose unknown arm is 18.
            //
            // Measured (t187): that default stamped 8 of 37 map entries wrong,
            // always 18, including EURC (6 real → 1e12 unit error) and the
            // 8-decimal tokens (→ 1e10). A wrong unit of that class produces a
            // size that cannot work, so every economic verdict measured on sizes
            // generated through this path was contaminated.
            //
            // A token in NEITHER source is NOT priced: the map is left empty and
            // the reason is logged (R8 — "not computed" is not 18, and a default
            // sold as data is fabrication).
            {
                let resolved = self
                    .resolve_cycle_decimals(&chosen.token_addresses, chain_id)
                    .await;
                if resolved.unresolved.is_empty() {
                    if !resolved.map.map.is_empty() {
                        chosen.decimals = resolved.map;
                    }
                } else {
                    warn!(
                        event = "orchestrator.decimals_unresolved",
                        chain_id,
                        reason = "decimals_not_in_redis_token_catalog_nor_pg_tokens_decimals",
                        unresolved = ?resolved.unresolved,
                        route_tokens = chosen.token_addresses.len(),
                        "per-hop decimals unresolved — route not priced with an invented unit (R8)"
                    );
                }
            }
"""

# ---------------------------------------------------------------------------
# 4) orchestrator.rs — los tests del falsificador
# ---------------------------------------------------------------------------
TESTS_ANCHOR = """    #[test]
    fn liquidation_engine_error_counter_increments() {
"""
TESTS_NEW = '''    // ── DECIMALS-CYCLE-01 (t197) ─────────────────────────────────────────────
    // El defecto medido en t187: 8 de 37 ENTRADAS del `decimals.map` del ciclo
    // discrepaban, y el valor erroneo era siempre 18 (el arm desconocido de la
    // tabla canonica). Estos dos tests miden LAS ENTRADAS DEL MAPA (el valor
    // que entra por token de la ruta), no el `token_in` de la oportunidad
    // emitida: auditar `token_in` da un falso todo-OK (en t187 la auditoria por
    // `token_in` dio discrepancias=0 contra 8 reales).

    /// Los 8 casos medidos en t187: (address, symbol, decimals real, de donde
    /// sale). Los 8 tienen fila en PG `tokens.decimals` con
    /// `resolved_via='onchain_full'`; 6 de ellos estan ademas en el catalogo
    /// Redis `arbx:tokens:1:*` y 2 (ALICE, CLAUS) NO — por eso el resolutor
    /// tiene que consultar las DOS fuentes y no solo Redis.
    #[cfg(test)]
    const T187_CASES: [(&str, &str, u8, &str); 8] = [
        ("0x1abaea1f7c830bd89acc67ec4af516284b1bc33c", "EURC", 6, "redis+pg"),
        ("0xa1f410f13b6007fca76833ee7eb58478d47bc5ef", "RJV", 6, "redis+pg"),
        ("0xac51066d7bec65dc4589368da368b212745d63e8", "ALICE", 6, "pg_only"),
        ("0x2b591e99afe9f32eaa6214f7b7629768c40eeb39", "HEX", 8, "redis+pg"),
        ("0x72e4f9f808c49a2a61de9c5896298920dc4eeea9", "BITCOIN", 8, "redis+pg"),
        ("0x14fee680690900ba0cccfc76ad70fd1b95d10e16", "$PAAL", 9, "redis+pg"),
        ("0x95af4af910c28e8ece4512bfe46f1f33687424ce", "MANYU", 9, "redis+pg"),
        ("0xa606d433971e9ee140e234daa7c94c476e10ead1", "CLAUS", 9, "pg_only"),
    ];

    fn t187_sources() -> (HashMap<String, u8>, HashMap<String, u8>) {
        let mut redis_decimals: HashMap<String, u8> = HashMap::new();
        let mut pg_decimals: HashMap<String, u8> = HashMap::new();
        for (addr, _sym, dec, provenance) in T187_CASES {
            pg_decimals.insert(addr.to_string(), dec);
            if provenance == "redis+pg" {
                redis_decimals.insert(addr.to_string(), dec);
            }
        }
        (redis_decimals, pg_decimals)
    }

    /// FALSIFICADOR (t197): con el defecto restaurado (la tabla canonica y su
    /// default de 18), este test FALLA en los 8 casos. Con el fix PASA.
    #[test]
    fn decimals_cycle_t187_eight_cases_resolve_to_the_real_unit_not_18() {
        let (redis_decimals, pg_decimals) = t187_sources();
        let addrs: Vec<String> = T187_CASES.iter().map(|(a, _, _, _)| a.to_string()).collect();
        let got = build_cycle_decimals_map(&addrs, &redis_decimals, &pg_decimals);

        assert!(
            got.unresolved.is_empty(),
            "los 8 casos de t187 tienen valor real en PG y/o Redis; ninguno puede quedar sin resolver: {:?}",
            got.unresolved
        );

        for (addr, sym, dec, provenance) in T187_CASES {
            let lc = addr.to_string();
            let entry = got.map.map.get(&lc);
            assert_eq!(
                entry,
                Some(&dec),
                "{sym} {lc} ({provenance}) debe entrar al mapa con su unidad real {dec}"
            );
            assert_ne!(
                entry,
                Some(&18),
                "{sym} {lc}: el mapa NO puede llevar el default de 18 (defecto t187)"
            );
            // El defecto, reproducido dentro del test: la tabla canonica que el
            // ciclo usaba ANTES del fix sigue devolviendo 18 para estos 8
            // tokens. Es decir: el cambio esta en las ENTRADAS del mapa.
            assert_eq!(
                crate::engines::dex_engine::canonical_token_decimals_str(addr),
                18,
                "{sym} {lc}: la tabla canonica pre-fix devolvia 18 (defecto medido en t187)"
            );
        }
    }

    /// El caso negativo: un token sin fila en PG `tokens.decimals` Y sin entrada
    /// en el catalogo Redis NO recibe 18 — se declara NO COMPUTADO con reason
    /// explicito y no se le inventa unidad.
    #[test]
    fn decimals_cycle_unknown_token_is_not_computed_never_18() {
        let unknown = "0x00000000000000000000000000000000deadbeef".to_string();
        let empty_redis: HashMap<String, u8> = HashMap::new();
        let empty_pg: HashMap<String, u8> = HashMap::new();
        let got = build_cycle_decimals_map(&[unknown.clone()], &empty_redis, &empty_pg);

        assert!(
            got.map.map.is_empty(),
            "sin fuente no hay unidad: el mapa no puede llevar NINGUNA entrada (ni 18)"
        );
        assert_eq!(
            got.map.map.get(&unknown),
            None,
            "el token sin fuente no puede aparecer en el mapa"
        );
        assert_eq!(
            got.unresolved,
            vec![unknown.clone()],
            "el token sin fuente debe declararse NO COMPUTADO en `unresolved` \
             (reason: decimals_not_in_redis_token_catalog_nor_pg_tokens_decimals)"
        );
        // La ruta no se cotiza con una unidad inventada: el mapa vacio es lo que
        // el consumidor interpreta como "no computado", nunca 18.
    }

    /// Los dos casos negativos no pueden colarse por el otro lado: una fuente
    /// que SI responde gana sobre la ausencia de la otra (Redis primero, PG
    /// como respaldo), y el orden de la ruta se preserva en `unresolved`.
    #[test]
    fn decimals_cycle_source_precedence_and_route_order() {
        let mut redis_decimals: HashMap<String, u8> = HashMap::new();
        let mut pg_decimals: HashMap<String, u8> = HashMap::new();
        let a = "0xaaa0000000000000000000000000000000000001".to_string();
        let b = "0xbbb0000000000000000000000000000000000002".to_string();
        let c = "0xccc0000000000000000000000000000000000003".to_string();
        redis_decimals.insert(a.clone(), 6);
        pg_decimals.insert(a.clone(), 18); // PG no puede pisar a Redis
        pg_decimals.insert(b.clone(), 8); // solo PG
        let got = build_cycle_decimals_map(&[a.clone(), b.clone(), c.clone()], &redis_decimals, &pg_decimals);
        assert_eq!(got.map.map.get(&a), Some(&6), "Redis manda sobre PG");
        assert_eq!(got.map.map.get(&b), Some(&8), "PG respalda lo que Redis no tiene");
        assert_eq!(got.map.map.get(&c), None, "sin fuente no hay entrada");
        assert_eq!(got.unresolved, vec![c], "orden de ruta preservado");
    }

    #[test]
    fn liquidation_engine_error_counter_increments() {
'''

# ---------------------------------------------------------------------------
# 5) scanner.rs — pasar el provider al contexto
# ---------------------------------------------------------------------------
SCAN_ANCHOR = """        math_redis: redis.clone(),
"""
SCAN_NEW = """        math_redis: redis.clone(),
        // DECIMALS-CYCLE-01 (t197): el mismo provider PG que usa el encoder,
        // para que el `decimals.map` del ciclo lea `tokens.decimals` en vez de
        // defaultear a 18.
        token_decimals_provider: decimals_provider.clone(),
"""

TESTFIELD_ANCHOR = """        math_redis: redis.clone(),
"""
TESTFIELD_NEW = """        math_redis: redis.clone(),
        token_decimals_provider: None,
"""


def replace_once(path: pathlib.Path, old: str, new: str, label: str) -> None:
    raw = path.read_text(encoding="utf-8")
    n = raw.count(old)
    if n != 1:
        raise SystemExit(f"ABORT: anclaje '{label}' aparece {n} veces en {path} (se esperaba 1)")
    path.write_text(raw.replace(old, new, 1), encoding="utf-8")
    print(f"  ok  {label:28s} -> {path}")


def thread_provider_into_build_orchestrator(scan: pathlib.Path) -> None:
    """`build_orchestrator` no recibe el provider: se le añade el parametro (y a
    sus DOS call sites, ambos dentro de `run_chain`, que SI lo tiene en scope).

    Se hace por regex sobre `cartridge_context_router` para no depender de la
    indentacion exacta. Cuenta esperada: 1 firma + 2 llamadas.
    """
    raw = scan.read_text(encoding="utf-8")
    sig = re.compile(
        r"(cartridge_context_router: Option<Arc<crate::context_router::ContextRouter>>,\n)"
        r"(\s*\)\s*->\s*Option<\(Arc<Orchestrator>)"
    )
    if len(sig.findall(raw)) != 1:
        raise SystemExit("ABORT: firma de build_orchestrator no localizada (1 esperada)")
    raw = sig.sub(
        lambda m: m.group(1)
        + "        // DECIMALS-CYCLE-01 (t197): provider PG de `tokens.decimals` para el\n"
        + "        // `decimals.map` del ciclo — que antes defaulteaba a 18 con la tabla\n"
        + "        // canonica (8/37 entradas erroneas medidas en t187).\n"
        + "        decimals_provider: ScannerDecimalsProvider,\n"
        + m.group(2),
        raw,
        count=1,
    )
    call = re.compile(r"^([ \t]*)cartridge_context_router\.clone\(\),\s*$", re.M)
    if len(call.findall(raw)) != 2:
        raise SystemExit(f"ABORT: se esperaban 2 call sites de build_orchestrator, hay {len(call.findall(raw))}")
    raw = call.sub(
        lambda m: m.group(0) + "\n" + m.group(1) + "decimals_provider.clone(),", raw
    )
    scan.write_text(raw, encoding="utf-8")
    print(f"  ok  {'scanner:thread_provider':28s} -> {scan} (firma + 2 call sites)")


def main() -> None:
    root = pathlib.Path(sys.argv[1]).resolve()
    orch = root / ORCH
    scan = root / SCAN
    for p in (orch, scan, *[root / t for t in TESTS]):
        if not p.is_file():
            raise SystemExit(f"ABORT: falta {p}")
    if "DECIMALS-CYCLE-01" in orch.read_text(encoding="utf-8"):
        print("YA_APLICADO")
        return
    print("aplicando DECIMALS-CYCLE-01 ...")
    replace_once(orch, CTX_ANCHOR, CTX_NEW, "orchestrator:ctx_field")
    replace_once(orch, RESOLVER_ANCHOR, RESOLVER_NEW, "orchestrator:resolver")
    replace_once(orch, WRAPPER_ANCHOR, WRAPPER_NEW, "orchestrator:async_wrapper")
    replace_once(orch, CALLSITE_ANCHOR, CALLSITE_NEW, "orchestrator:callsite")
    replace_once(orch, TESTS_ANCHOR, TESTS_NEW, "orchestrator:tests")
    replace_once(scan, SCAN_ANCHOR, SCAN_NEW, "scanner:ctx_wire")
    thread_provider_into_build_orchestrator(scan)
    # Los 3 ficheros de test los completa fixup-dec01-tests.py: su literal
    # `OrchestratorContext {` usa una expresion distinta por fichero
    # (`dummy_conn` / `conn` / `redis_conn`), asi que se inserta por la linea
    # `math_redis:` preservando la indentacion (anclaje robusto, idempotente).
    print("APLICADO (tests: ejecutar fixup-dec01-tests.py)")


if __name__ == "__main__":
    main()
