//! REDEMPTION-PRODUCER-01 fase 2 (2026-10-03): lector on-chain de estado de
//! contratos de redemption (ERC-4626 vaults y equivalentes). Lee maxRedeem /
//! previewRedeem / convertToAssets vía `eth_call` crudo sobre el pool HTTP
//! con failover existente — mismo patrón que `price_worker::eth_call_latest_answer`.
//!
//! Las direcciones de los baskets vienen del OPERADOR vía
//! `ARBX_BASKET_CONTRACTS` (env var, formato `addr1,addr2,...`) — jamás
//! hardcodeadas (RULE 00 / arbx-no-hardcode-doctrine). Sin env var → sin
//! datos → los verificadores de redemption siguen en FAIL honesto (R8).
//!
//! BASKET-OWNER-01 (2026-10-04): `maxRedeem` es SIEMPRE relativo a una CUENTA.
//! La versión anterior consultaba `maxRedeem(address(0))`, que en la
//! implementación ERC-4626 estándar devuelve 0 (la dirección cero no posee
//! shares): `redemption_within_limits` comparaba el importe de la ruta contra
//! ese 0 y fallaba con `redemption_amount_exceeds_onchain_maxRedeem` aunque el
//! redemption fuera realmente posible — un diagnóstico FALSO. Quien redime es
//! el EXECUTOR del operador, así que su dirección llega por
//! `ARBX_BASKET_OWNER` (misma validación que los baskets) y es REQUISITO para
//! leer. Sin owner — ausente o malformado — NO se consulta el RPC y el mapa
//! queda VACÍO: "sin estado" es la respuesta honesta mientras el operador no
//! termine de configurar, y el verificador lo reporta como tal (R8); jamás se
//! fabrica un límite.
//!
//! OBSERVER-ONLY: eth_call estático, sin firma ni broadcast.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use tracing::{debug, warn};

/// Selector de función (4 bytes keccak) para llamadas ERC-4626.
/// maxRedeem(address) = 0xc63d32b8
/// totalAssets() = 0x01e1d114
const SEL_MAX_REDEEM: &str = "c63d32b8";
const SEL_TOTAL_ASSETS: &str = "01e1d114";

/// Env var del operador con las direcciones de los baskets (`addr1,addr2,...`).
const BASKET_CONTRACTS_ENV: &str = "ARBX_BASKET_CONTRACTS";
/// Env var del operador con la dirección que EJECUTA el redemption (executor /
/// contrato de arbitraje). Sin ella no hay lectura posible: el `maxRedeem` de
/// `address(0)` es 0 en cualquier ERC-4626 estándar.
const BASKET_OWNER_ENV: &str = "ARBX_BASKET_OWNER";

/// Razón exacta (log del llamador y tests) cuando la env var del owner no está
/// definida.
const OWNER_MISSING_REASON: &str = "ARBX_BASKET_OWNER_not_set";
/// Razón exacta cuando la env var del owner está definida pero no es una
/// dirección canónica (42 caracteres con prefijo `0x`).
const OWNER_MALFORMED_REASON: &str = "ARBX_BASKET_OWNER_malformed_expected_42_char_0x_address";

/// Normaliza y valida una dirección EVM tal como la escribe el operador:
/// `trim` + minúsculas y, sólo si queda con la forma canónica (42 caracteres
/// con prefijo `0x`), se acepta. Es la MISMA regla para los baskets y para el
/// owner — una sola fuente de verdad, de modo que un "casi dirección" jamás
/// llega al RPC.
fn normalize_address(raw: &str) -> Option<String> {
    let addr = raw.trim().to_ascii_lowercase();
    (addr.len() == 42 && addr.starts_with("0x")).then_some(addr)
}

/// Parsea la env var `ARBX_BASKET_CONTRACTS` (direcciones separadas por coma).
/// Sin var o malformada → mapa vacío (los verificadores fallan honesto).
pub fn baskets_from_env() -> Vec<String> {
    std::env::var(BASKET_CONTRACTS_ENV)
        .unwrap_or_default()
        .split(',')
        .filter_map(normalize_address)
        .collect()
}

/// Núcleo PURO de la resolución del owner: `None` = env var ausente. Ausente o
/// malformada → `Err(razón exacta)`, nunca un owner inventado.
fn owner_from_raw(raw: Option<String>) -> Result<String, &'static str> {
    match raw {
        None => Err(OWNER_MISSING_REASON),
        Some(value) => normalize_address(&value).ok_or(OWNER_MALFORMED_REASON),
    }
}

/// Owner que EJECUTA el redemption, desde `ARBX_BASKET_OWNER` (el executor /
/// contrato de arbitraje del operador; jamás hardcodeado).
///
/// Es REQUISITO para que un `maxRedeem` signifique algo: comparar el importe
/// de la ruta contra el `maxRedeem` de `address(0)` produce un FAIL FALSO
/// ("el importe excede el límite"). Ausente o malformada → `Err(razón)` y el
/// llamador NO lee (R8).
pub fn basket_owner_from_env() -> Result<String, &'static str> {
    match std::env::var(BASKET_OWNER_ENV) {
        Ok(value) => owner_from_raw(Some(value)),
        Err(std::env::VarError::NotPresent) => owner_from_raw(None),
        // Presente pero no decodificable como texto: no es una dirección.
        Err(std::env::VarError::NotUnicode(_)) => Err(OWNER_MALFORMED_REASON),
    }
}

/// BASKET-OWNER-01 — plan de lectura resuelto: la ÚNICA puerta por la que un
/// llamador llega al RPC de baskets. Materializa la regla fail-honest en un
/// solo sitio, de modo que el camino del intent y el wrapper de conveniencia
/// no puedan divergir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasketReadPlan {
    /// Sin baskets que leer (env var vacía o sin direcciones válidas, o ningún
    /// basket relevante a la ruta): CERO RPC.
    NoContracts,
    /// Hay contratos configurados pero NO un owner utilizable: CERO RPC y mapa
    /// vacío (R8). La razón exacta viaja aquí para el log del llamador.
    NoOwner {
        contracts: usize,
        reason: &'static str,
    },
    /// Lectura posible: estos baskets, con ESTE owner canónico.
    Ready { baskets: Vec<String>, owner: String },
}

/// Resuelve el plan desde los baskets ya seleccionados y el resultado de
/// `basket_owner_from_env()`. PURO (sin entorno ni red) para poder probar las
/// tres ramas fail-honest directamente.
///
/// Orden deliberado: sin contratos se responde `NoContracts` ANTES de mirar el
/// owner — quien no configuró baskets no recibe un aviso de configuración que
/// no le hace falta — y con contratos pero sin owner jamás se devuelve `Ready`.
pub fn plan_read(relevant: Vec<String>, owner: Result<String, &'static str>) -> BasketReadPlan {
    if relevant.is_empty() {
        return BasketReadPlan::NoContracts;
    }
    match owner {
        Ok(owner) => BasketReadPlan::Ready {
            baskets: relevant,
            owner,
        },
        Err(reason) => BasketReadPlan::NoOwner {
            contracts: relevant.len(),
            reason,
        },
    }
}

/// Lee el estado de redemption de un contrato ERC-4626 via eth_call crudo.
/// `owner_addr` es la cuenta cuyo `maxRedeem` se consulta (el executor del
/// operador: ver `basket_owner_from_env`); con cualquier otra cosa devuelve
/// None en vez de construir calldata basura. Devuelve None si el RPC falla (R8).
pub async fn read_redemption_state(
    rpc_url: &str,
    basket_addr: &str,
    owner_addr: &str,
) -> Option<Value> {
    let owner_addr = normalize_address(owner_addr)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .ok()?;

    // maxRedeem(owner) — el máximo que ESA dirección puede redimir.
    let max_redeem_data = format!(
        "{}{}{}",
        SEL_MAX_REDEEM,
        "000000000000000000000000",
        &owner_addr[2..]
    );
    let max_redeem = eth_call_static(&client, rpc_url, basket_addr, &max_redeem_data).await?;

    // totalAssets() — el AUM del vault (sanity check de liquidez).
    let total_assets = eth_call_static(&client, rpc_url, basket_addr, SEL_TOTAL_ASSETS).await;

    Some(json!({
        "basket_address": basket_addr,
        "max_redeem_raw": format!("0x{}", max_redeem),
        "total_assets_raw": total_assets
            .as_ref()
            .map(|t| format!("0x{}", t))
            .unwrap_or_default(),
        "read_at_ms": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis() as u64,
    }))
}

/// eth_call estático crudo (mismo patrón que price_worker). Devuelve los
/// datos de respuesta (hex) sin el prefijo 0x.
async fn eth_call_static(
    client: &reqwest::Client,
    rpc_url: &str,
    to: &str,
    data: &str,
) -> Option<String> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_call",
        "params": [
            { "to": to, "data": format!("0x{data}") },
            "latest"
        ]
    });
    let resp: Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    resp.get("result")?
        .as_str()
        .map(|s| s.trim_start_matches("0x").to_string())
        .filter(|s| s.len() >= 2)
}

/// Lee el estado de redemption de los baskets INDICADOS — subconjunto
/// RELEVANTE al intent que arma el llamador (BASKET-WORKER-01) — con failover
/// entre endpoints: el primer RPC que responde por basket gana. Sin baskets o
/// sin endpoints → mapa vacío (R8), sin tocar la red.
///
/// BASKET-OWNER-01: `owner` es la cuenta que redime (executor del operador).
/// Si no es una dirección canónica devuelve mapa vacío SIN tocar la red:
/// consultar un owner inventado daría un `maxRedeem` que no es el de nadie.
pub async fn read_baskets(
    baskets: &[String],
    rpc_urls: &[String],
    owner: &str,
) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    if baskets.is_empty() || rpc_urls.is_empty() {
        return out;
    }
    let Some(owner) = normalize_address(owner) else {
        debug!(
            event = "basket_reader.owner_invalid",
            baskets = baskets.len(),
            "owner no canónico; sin lectura on-chain, redemption_state vacío (R8 fail-honest)"
        );
        return out;
    };
    for basket in baskets {
        for rpc in rpc_urls {
            if let Some(state) = read_redemption_state(rpc, basket, &owner).await {
                out.insert(basket.clone(), state);
                break; // primer RPC que responde gana (failover)
            }
        }
    }
    out
}

/// Lee todos los baskets configurados y devuelve el mapa para el bundle.
/// Sin baskets configurados → mapa vacío (honesto, no error).
///
/// BASKET-OWNER-01: con `ARBX_BASKET_CONTRACTS` configurado pero sin
/// `ARBX_BASKET_OWNER` (ausente o malformado) NO se consulta ningún RPC y el
/// mapa queda VACÍO: el `maxRedeem` de `address(0)` devolvería 0 y el
/// verificador lo leería como "el importe excede el límite" — un diagnóstico
/// FALSO. Se registra a `warn!` la razón EXACTA (es un mensaje de configuración
/// del operador, no de un ítem del hot path: R9 LOGFLOOD-01).
///
/// La ruta del intent NO usa esta variante (leería baskets ajenos a la ruta):
/// usa `read_baskets` con el subconjunto filtrado por relevancia.
pub async fn read_all_baskets(rpc_urls: &[String]) -> BTreeMap<String, Value> {
    match plan_read(baskets_from_env(), basket_owner_from_env()) {
        BasketReadPlan::Ready { baskets, owner } => read_baskets(&baskets, rpc_urls, &owner).await,
        BasketReadPlan::NoContracts => BTreeMap::new(),
        BasketReadPlan::NoOwner { contracts, reason } => {
            warn!(
                event = "basket_reader.owner_unset",
                contracts,
                reason,
                "ARBX_BASKET_CONTRACTS configurado sin ARBX_BASKET_OWNER utilizable; \
                 sin lectura on-chain, redemption_state vacío (R8 fail-honest)"
            );
            BTreeMap::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dirección canónica sintética (cero hardcodeo de direcciones reales:
    /// sólo la FORMA importa para validar).
    fn canonical(hex: char) -> String {
        format!("0x{}", hex.to_string().repeat(40))
    }

    // ── normalización: una sola regla para baskets y owner ────────────────

    #[test]
    fn normalize_address_requires_42_chars_with_0x_prefix() {
        assert_eq!(normalize_address(&canonical('a')), Some(canonical('a')));
        // 41 caracteres (una letra de menos) → inválida.
        assert_eq!(normalize_address(&format!("0x{}", "a".repeat(39))), None);
        // 43 caracteres (una de más) → inválida.
        assert_eq!(normalize_address(&format!("0x{}", "a".repeat(41))), None);
        // Sin prefijo 0x → inválida aunque tenga 42 caracteres.
        assert_eq!(normalize_address(&"a".repeat(42)), None);
        assert_eq!(normalize_address(""), None);
        assert_eq!(normalize_address("0x"), None);
    }

    #[test]
    fn normalize_address_trims_and_lowercases() {
        let mixed = format!("0x{}", "AbCdEf".repeat(6) + "AbCd");
        assert_eq!(mixed.len() - 2, 40);
        let expected = mixed.trim().to_ascii_lowercase();
        assert_eq!(
            normalize_address(&mixed).as_deref(),
            Some(expected.as_str())
        );
        assert_eq!(
            normalize_address(&format!("  {mixed}  ")).as_deref(),
            Some(expected.as_str())
        );
    }

    // ── rama 1: sin contratos ─────────────────────────────────────────────

    #[test]
    fn no_contracts_is_no_read_even_without_owner() {
        assert_eq!(
            plan_read(Vec::new(), Err(OWNER_MISSING_REASON)),
            BasketReadPlan::NoContracts
        );
        assert_eq!(
            plan_read(Vec::new(), Ok(canonical('b'))),
            BasketReadPlan::NoContracts
        );
    }

    // ── rama 2: contratos configurados, owner AUSENTE ─────────────────────

    #[test]
    fn contracts_without_owner_plan_no_read_with_exact_missing_reason() {
        let plan = plan_read(vec![canonical('c')], owner_from_raw(None));
        assert_eq!(
            plan,
            BasketReadPlan::NoOwner {
                contracts: 1,
                reason: "ARBX_BASKET_OWNER_not_set",
            }
        );
    }

    // ── rama 3: contratos configurados, owner MALFORMADO ──────────────────

    #[test]
    fn contracts_with_malformed_owner_plan_no_read_with_exact_reason() {
        for bad in [
            String::new(),                    // definida pero vacía
            "0x".to_string(),                 // sólo el prefijo
            "0xdead".to_string(),             // demasiado corta
            "a".repeat(42),                   // sin prefijo 0x
            canonical('d')[..41].to_string(), // 41 caracteres (pegado truncado)
        ] {
            let reason = owner_from_raw(Some(bad.clone())).unwrap_err();
            let plan = plan_read(vec![canonical('e'), canonical('f')], Err(reason));
            assert_eq!(
                plan,
                BasketReadPlan::NoOwner {
                    contracts: 2,
                    reason: "ARBX_BASKET_OWNER_malformed_expected_42_char_0x_address",
                },
                "owner {bad:?} debe quedar en NoOwner"
            );
        }
    }

    #[test]
    fn malformed_owner_is_never_mistaken_for_missing() {
        assert_ne!(
            owner_from_raw(Some("0xdead".to_string())).unwrap_err(),
            OWNER_MISSING_REASON
        );
    }

    // ── camino listo: owner canónico ──────────────────────────────────────

    #[test]
    fn contracts_with_canonical_owner_plan_reads_with_that_owner() {
        let baskets = vec![canonical('1'), canonical('2')];
        let plan = plan_read(baskets.clone(), owner_from_raw(Some(canonical('A'))));
        assert_eq!(
            plan,
            BasketReadPlan::Ready {
                baskets,
                // El owner se normaliza a minúsculas: la calldata nunca lleva
                // mayúsculas del operador.
                owner: canonical('a'),
            }
        );
    }

    #[test]
    fn ready_plan_always_carries_a_canonical_owner() {
        // Con owner crudo válido el plan SIEMPRE es Ready y su owner es
        // canónico (nunca el zero address: el defecto BASKET-OWNER-01).
        let expected = canonical('9');
        let owner = owner_from_raw(Some(format!("  {expected}  "))).expect("owner válido");
        match plan_read(vec![canonical('3')], Ok(owner)) {
            BasketReadPlan::Ready { owner, .. } => {
                assert_eq!(owner, expected);
                assert_eq!(normalize_address(&owner).as_deref(), Some(owner.as_str()));
                assert_ne!(owner, "0x0000000000000000000000000000000000000000");
            }
            other => panic!("esperaba Ready, llegó {other:?}"),
        }
    }

    #[test]
    fn owner_from_raw_accepts_only_the_canonical_form() {
        assert!(owner_from_raw(Some(canonical('a'))).is_ok());
        assert!(owner_from_raw(Some(format!(" {} ", canonical('b')))).is_ok());
        assert!(owner_from_raw(None).is_err());
        assert!(owner_from_raw(Some(String::new())).is_err());
        assert!(owner_from_raw(Some(
            "0x000000000000000000000000000000000000000".to_string()
        ))
        .is_err());
    }
}
