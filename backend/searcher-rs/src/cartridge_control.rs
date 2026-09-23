//! CARTRIDGE-CONTROL — acople/desacople (enable/disable) por cartucho.
//!
//! Mecanismo de control operativo para CUALQUIER cartucho cargado (los 264
//! numerados + los 7 raíz + futuros v4): el operador decide qué estrategias
//! participan del runtime sin recompilar ni reiniciar.
//!
//! ## Contrato
//!
//! * **Estado deseado** — hash Redis `arbx:cartridges:control:<chain>` con
//!   campo `cartridge_id` → `"enabled" | "disabled"`. Es la única fuente de
//!   verdad que el searcher lee. Un cartucho SIN entrada en el hash usa su
//!   estado por defecto de carga (Active) — la ausencia nunca desacopla.
//! * **Comandos en caliente** — canal PubSub `arbx:cartridges:control:commands`
//!   con JSON `{cartridge_id, desired, actor, reason, chain_id}`; el loop
//!   aplica `runner.pause_cartridge`/`resume_carbitrage` y re-publica el
//!   registro (la UI ve el cambio en el próximo GET /api/cartridges).
//! * **Auditoría** — la api-server persiste cada comando en PG
//!   (`cartridge_control`, migración 125) ANTES de escribir Redis; el searcher
//!   nunca escribe PG en este path (observabilidad pura).
//! * **Fail-closed** — un comando con cartridge_id vacío, desired inválido o
//!   chain_id distinto se ignora con warn; nunca pausa "todo".
//!
//! ## Fronteras respetadas
//!
//! - No toca firmadores, modos de trading ni broadcast (§32/§33/§34): desacoplar
//!   un cartucho solo deja de EVALUARLO; el terminus de capital no cambia.
//! - respeta la doctrina dispatch/status: pausar es un estado de runtime
//!   adicional (CartridgeState::Paused ya existía y el evaluador lo respeta
//!   con `NotActive`).

use crate::cartridge::runner::CartridgeRunner;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Hash de estado deseado por cadena: campo `cartridge_id` → `enabled|disabled`.
pub fn control_hash_key(chain_id: u64) -> String {
    format!("arbx:cartridges:control:{chain_id}")
}

/// Canal PubSub de comandos de control (todas las cadenas; el mensaje lleva
/// chain_id y el loop filtra por la suya).
pub const CONTROL_CHANNEL: &str = "arbx:cartridges:control:commands";

/// Estado deseado de un cartucho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesiredState {
    Enabled,
    Disabled,
}

impl DesiredState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    /// `"enabled"`/`"disabled"` (case-insensitive). Cualquier otra cosa es
    /// `None` — el comando se descarta (fail-closed).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "enabled" | "enable" | "on" | "true" => Some(Self::Enabled),
            "disabled" | "disable" | "off" | "false" | "paused" => Some(Self::Disabled),
            _ => None,
        }
    }
}

/// Comando de control tal como viaja por el canal PubSub (lo publica la
/// api-server tras persistirlo en PG).
#[derive(Debug, Clone)]
pub struct ControlCommand {
    pub chain_id: u64,
    pub cartridge_id: String,
    pub desired: DesiredState,
    pub actor: String,
    pub reason: String,
}

impl ControlCommand {
    /// Parsea el payload JSON del canal. `None` = comando inválido (se ignora).
    pub fn parse(payload: &str) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(payload).ok()?;
        let chain_id = v.get("chain_id")?.as_u64()?;
        let cartridge_id = v.get("cartridge_id")?.as_str()?.trim().to_string();
        if cartridge_id.is_empty() {
            return None;
        }
        let desired = DesiredState::parse(v.get("desired")?.as_str()?)?;
        let actor = v
            .get("actor")
            .and_then(|a| a.as_str())
            .unwrap_or("unknown")
            .to_string();
        let reason = v
            .get("reason")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();
        Some(Self {
            chain_id,
            cartridge_id,
            desired,
            actor,
            reason,
        })
    }
}

/// Aplica el estado deseado de UN cartucho al runner. Devuelve `true` si el
/// estado del runtime cambió. Un cartucho no cargado NO es error: el estado
/// deseado queda registrado y se aplicará cuando cargue (o jamás — honesto).
pub async fn apply_desired(
    runner: &Arc<CartridgeRunner>,
    cartridge_id: &str,
    desired: DesiredState,
) -> bool {
    let result = match desired {
        DesiredState::Disabled => runner.pause_cartridge(cartridge_id).await,
        DesiredState::Enabled => runner.resume_cartridge(cartridge_id).await,
    };
    result.is_ok()
}

/// Boot-time: lee TODO el hash de estado deseado y lo aplica. Devuelve el
/// número de cartuchos cuyo estado de runtime cambió. Errores de Redis son
/// no-fatales (warn) — sin hash alcanzable, todos los cartuchos quedan en su
/// estado de carga por defecto (ningún desacople silencioso masivo).
pub async fn apply_desired_states(
    runner: &Arc<CartridgeRunner>,
    redis: &mut redis::aio::ConnectionManager,
    chain_id: u64,
) -> usize {
    let states: std::collections::HashMap<String, String> =
        match redis::AsyncCommands::hgetall(&mut *redis, control_hash_key(chain_id)).await {
            Ok(map) => map,
            Err(e) => {
                warn!(
                    event = "cartridge_control.boot_read_failed",
                    chain_id,
                    error = %e,
                    "control hash unreadable — keeping load-default states (no silent mass-pause)"
                );
                return 0;
            }
        };
    let mut applied = 0usize;
    for (cartridge_id, raw) in states {
        let Some(desired) = DesiredState::parse(&raw) else {
            warn!(
                event = "cartridge_control.invalid_desired_state",
                chain_id,
                cartridge_id = %cartridge_id,
                raw = %raw,
                "ignoring invalid desired state (fail-closed)"
            );
            continue;
        };
        if apply_desired(runner, &cartridge_id, desired).await {
            applied += 1;
            info!(
                event = "cartridge_control.boot_applied",
                chain_id,
                cartridge_id = %cartridge_id,
                desired = desired.as_str(),
                "desired state applied at boot"
            );
        }
    }
    applied
}

/// Loop de comandos en caliente: suscribe el canal, aplica cada comando de
/// ESTA cadena y re-publica el registro para que la UI refleje el estado.
/// Fire-and-forget (el caller la spawnea); termina en cancel.
pub async fn control_loop(
    runner: Arc<CartridgeRunner>,
    redis_url: String,
    chain_id: u64,
    cancel: CancellationToken,
) {
    let client = match redis::Client::open(redis_url.as_str()) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                event = "cartridge_control.loop_client_failed",
                chain_id,
                error = %e,
                "control loop not started (hot toggles unavailable until restart)"
            );
            return;
        }
    };
    let mut pubsub = match client.get_async_pubsub().await {
        Ok(p) => p,
        Err(e) => {
            warn!(
                event = "cartridge_control.loop_pubsub_failed",
                chain_id,
                error = %e,
                "control loop not started"
            );
            return;
        }
    };
    if let Err(e) = pubsub.subscribe(CONTROL_CHANNEL).await {
        warn!(
            event = "cartridge_control.loop_subscribe_failed",
            chain_id,
            error = %e
        );
        return;
    }
    info!(
        event = "cartridge_control.loop_started",
        chain_id,
        channel = CONTROL_CHANNEL,
        "hot couple/decouple loop listening"
    );
    let mut stream = pubsub.on_message();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            msg = stream.next() => {
                let Some(msg) = msg else { break };
                let payload = match msg.get_payload::<String>() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let Some(cmd) = ControlCommand::parse(&payload) else {
                    warn!(
                        event = "cartridge_control.invalid_command",
                        chain_id,
                        "discarding malformed control command (fail-closed)"
                    );
                    continue;
                };
                if cmd.chain_id != chain_id {
                    continue; // comando de otra cadena — no es error
                }
                let changed = apply_desired(&runner, &cmd.cartridge_id, cmd.desired).await;
                info!(
                    event = if changed { "cartridge_control.applied" } else { "cartridge_control.noop" },
                    chain_id,
                    cartridge_id = %cmd.cartridge_id,
                    desired = cmd.desired.as_str(),
                    actor = %cmd.actor,
                    reason = %cmd.reason,
                    "control command processed"
                );
                // Re-publicar el registro para que GET /api/cartridges refleje
                // el cambio de estado sin esperar el refresh de 240s.
                if changed {
                    let mut conn = runner.redis_connection().await;
                    crate::cartridge_boot::publish_cartridge_registry(
                        &mut conn,
                        &runner,
                        chain_id,
                    )
                    .await;
                }
            }
        }
    }
    info!(event = "cartridge_control.loop_stopped", chain_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_key_format() {
        assert_eq!(control_hash_key(1), "arbx:cartridges:control:1");
    }

    #[test]
    fn desired_state_parse_roundtrip() {
        assert_eq!(DesiredState::parse("enabled"), Some(DesiredState::Enabled));
        assert_eq!(DesiredState::parse("DISABLED"), Some(DesiredState::Disabled));
        assert_eq!(DesiredState::parse("off"), Some(DesiredState::Disabled));
        assert_eq!(DesiredState::parse(""), None);
        assert_eq!(DesiredState::parse("maybe"), None);
        assert_eq!(DesiredState::Disabled.as_str(), "disabled");
    }

    #[test]
    fn command_parse_valid_and_invalid() {
        let ok = ControlCommand::parse(
            r#"{"chain_id":1,"cartridge_id":"mev_01_001_dex_dex_arbitrage","desired":"disabled","actor":"operator","reason":"decouple test"}"#,
        )
        .expect("valid command");
        assert_eq!(ok.chain_id, 1);
        assert_eq!(ok.cartridge_id, "mev_01_001_dex_dex_arbitrage");
        assert_eq!(ok.desired, DesiredState::Disabled);
        assert_eq!(ok.actor, "operator");

        // vacío / desired inválido / sin chain → None (fail-closed)
        assert!(ControlCommand::parse(r#"{"chain_id":1,"cartridge_id":"","desired":"disabled"}"#).is_none());
        assert!(ControlCommand::parse(r#"{"chain_id":1,"cartridge_id":"x","desired":"zap"}"#).is_none());
        assert!(ControlCommand::parse(r#"{"cartridge_id":"x","desired":"disabled"}"#).is_none());
        assert!(ControlCommand::parse("not json").is_none());
    }
}
