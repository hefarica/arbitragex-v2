//! Filesystem-based cartridge loader for development and bootstrap.
//!
//! In production, cartridges are injected via Redis PubSub (see `subscriber.rs`).
//! During development and initial boot, this loader scans the `cartridges/`
//! directory and loads all `.rhai` files found there.
//!
//! ## Boot Sequence
//!
//! 1. Scan `CARTRIDGE_DIR` for `.rhai` files.
//! 2. For each file, compute SHA-256 content hash.
//! 3. Call `CartridgeRunner::load_cartridge(id, source, hash)`.
//! 4. Log success/failure for each cartridge.
//! 5. Return summary of loaded cartridges.
//!
//! ## Chain Filtering
//!
//! After loading, cartridges are filtered by `target_chains` in their metadata.
//! A cartridge with `target_chains: []` (empty) is active on ALL chains.
//! A cartridge with `target_chains: [1, 42161]` is only active on those chains.

use crate::cartridge::runner::CartridgeRunner;
use crate::cartridge::types::CartridgeMetadata;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Default directory for cartridge files (relative to binary working dir).
pub const CARTRIDGE_DIR: &str = "cartridges";

/// Result of loading a single cartridge from filesystem.
#[derive(Debug)]
pub struct LoadResult {
    pub file_name: String,
    pub cartridge_id: String,
    pub success: bool,
    pub metadata: Option<CartridgeMetadata>,
    pub error: Option<String>,
}

/// Scans the cartridges directory and loads all `.rhai` files.
///
/// Returns a vector of `LoadResult` for operator visibility.
pub async fn load_cartridges_from_dir(
    runner: &Arc<CartridgeRunner>,
    dir: &Path,
    chain_id: u64,
) -> Vec<LoadResult> {
    let mut results = Vec::new();

    if !dir.exists() {
        warn!(
            event = "cartridge_loader.dir_not_found",
            path = %dir.display(),
            "cartridges directory not found — no cartridges loaded at boot"
        );
        return results;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            error!(
                event = "cartridge_loader.read_dir_failed",
                path = %dir.display(),
                error = %e,
            );
            return results;
        }
    };

    let mut rhai_files: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "rhai").unwrap_or(false) {
            rhai_files.push(path);
        }
    }

    // Sort for deterministic load order
    rhai_files.sort();

    info!(
        event = "cartridge_loader.scan_complete",
        dir = %dir.display(),
        count = rhai_files.len(),
        "found .rhai cartridge files"
    );

    for path in rhai_files {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_owned());

        // Derive cartridge ID from filename (without extension)
        let cartridge_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                results.push(LoadResult {
                    file_name: file_name.clone(),
                    cartridge_id: cartridge_id.clone(),
                    success: false,
                    metadata: None,
                    error: Some(format!("read error: {e}")),
                });
                continue;
            }
        };

        // Compute content hash
        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        // Skip if already loaded with same hash (dedup)
        if runner.has_content_hash(&content_hash).await {
            info!(
                event = "cartridge_loader.dedup_skip",
                file = %file_name,
                hash = %content_hash,
            );
            continue;
        }

        // Load the cartridge
        match runner
            .load_cartridge(&cartridge_id, &source, &content_hash)
            .await
        {
            Ok(metadata) => {
                // Check chain compatibility
                if !metadata.target_chains.is_empty() && !metadata.target_chains.contains(&chain_id)
                {
                    info!(
                        event = "cartridge_loader.chain_mismatch",
                        file = %file_name,
                        cartridge_chains = ?metadata.target_chains,
                        host_chain = chain_id,
                        "cartridge not targeting this chain — loaded but paused"
                    );
                    let _ = runner.pause_cartridge(&cartridge_id).await;
                }

                results.push(LoadResult {
                    file_name,
                    cartridge_id,
                    success: true,
                    metadata: Some(metadata),
                    error: None,
                });
            }
            Err(e) => {
                error!(
                    event = "cartridge_loader.load_failed",
                    file = %file_name,
                    error = %e,
                );
                results.push(LoadResult {
                    file_name,
                    cartridge_id,
                    success: false,
                    metadata: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let success_count = results.iter().filter(|r| r.success).count();
    let fail_count = results.iter().filter(|r| !r.success).count();

    info!(
        event = "cartridge_loader.complete",
        total = results.len(),
        success = success_count,
        failed = fail_count,
        chain_id = chain_id,
        "cartridge boot load complete"
    );

    results
}
