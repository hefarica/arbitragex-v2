//! StrategyRegistry — Loader dinámico de cartuchos estratégicos
//!
//! Carga 264 cartuchos Rhai desde el filesystem:
//! backend/searcher-rs/cartridges/strategies/mev_XX_NNN_name.rhai
//!
//! Doctrina de Aislamiento Topológico:
//! - Sin código rígido para estrategias individuales
//! - Cada estrategia es un archivo .rhai editable manualmente
//! - El frontend ensambla y configura los cartuchos

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Metadatos de un cartucho estratégico (extraído de init_strategy())
#[derive(Debug, Clone)]
pub struct CartridgeMeta {
    pub mev_id: String,
    pub name: String,
    pub family: String,
    pub math_domain: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub category: String,
    pub target_chains: Vec<u64>,
    pub min_eval_interval_ms: u64,
    pub atomic_possible: bool,
    pub nonatomic_possible: bool,
    pub min_legs: u32,
    pub max_legs: u32,
    pub applicable_operators: Vec<u8>,
    pub source_path: PathBuf,
}

/// Despachador central de estrategias MEV (loader dinámico)
pub struct StrategyRegistry {
    cartridges: HashMap<String, CartridgeMeta>,
    cartridge_path: PathBuf,
}

impl StrategyRegistry {
    pub fn new() -> Self {
        let path = PathBuf::from("../searcher-rs/cartridges/strategies");
        let mut reg = Self {
            cartridges: HashMap::with_capacity(264),
            cartridge_path: path.clone(),
        };
        reg.discover_cartridges(&path);
        reg
    }

    fn discover_cartridges(&mut self, path: &Path) {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let fp = entry.path();
                if fp.extension().map(|e| e == "rhai").unwrap_or(false) {
                    if let Some(meta) = self.parse_cartridge_meta(&fp) {
                        self.cartridges.insert(meta.mev_id.clone(), meta);
                    }
                }
            }
        }
    }

    fn parse_cartridge_meta(&self, path: &Path) -> Option<CartridgeMeta> {
        let content = std::fs::read_to_string(path).ok()?;
        let mev_id = Self::extract_field(&content, "mev_id:")?;
        let name = Self::extract_field(&content, "name:")?;
        let family = Self::extract_field(&content, "family:").unwrap_or_default();
        let math_domain = Self::extract_field(&content, "math_domain:").unwrap_or_default();
        let version = Self::extract_field(&content, "version:").unwrap_or_else(|| "1.0.0".into());
        let author = Self::extract_field(&content, "author:").unwrap_or_else(|| "omega".into());
        let description = Self::extract_field(&content, "description:").unwrap_or_default();
        let category = Self::extract_field(&content, "category:").unwrap_or_else(|| "auto".into());
        let atomic = Self::extract_bool(&content, "atomic_possible:").unwrap_or(true);
        let nonatomic = Self::extract_bool(&content, "nonatomic_possible:").unwrap_or(true);
        let min_legs = Self::extract_u32(&content, "min_legs:").unwrap_or(2);
        let max_legs = Self::extract_u32(&content, "max_legs:").unwrap_or(8);
        let interval = Self::extract_u64(&content, "min_eval_interval_ms:").unwrap_or(100);
        let ops = Self::extract_operators(&content).unwrap_or_default();

        Some(CartridgeMeta {
            mev_id, name, family, math_domain, version, author,
            description, category, target_chains: vec![],
            min_eval_interval_ms: interval,
            atomic_possible: atomic, nonatomic_possible: nonatomic,
            min_legs, max_legs,
            applicable_operators: ops,
            source_path: path.to_path_buf(),
        })
    }

    fn extract_field(content: &str, key: &str) -> Option<String> {
        content.lines()
            .find(|l| l.contains(key))
            .and_then(|l| {
                let start = l.find('"')? + 1;
                let end = l.rfind('"')?;
                Some(l[start..end].to_string())
            })
    }

    fn extract_bool(content: &str, key: &str) -> Option<bool> {
        content.lines()
            .find(|l| l.contains(key))
            .map(|l| l.contains("true"))
    }

    fn extract_u32(content: &str, key: &str) -> Option<u32> {
        content.lines()
            .find(|l| l.contains(key))
            .and_then(|l| {
                let num: String = l.chars().skip_while(|c| !c.is_ascii_digit())
                    .take_while(|c| c.is_ascii_digit()).collect();
                num.parse().ok()
            })
    }

    fn extract_u64(content: &str, key: &str) -> Option<u64> {
        Self::extract_u32(content, key).map(|v| v as u64)
    }

    fn extract_operators(content: &str) -> Option<Vec<u8>> {
        content.lines()
            .find(|l| l.contains("applicable_operators:"))
            .map(|l| {
                l.split(':').nth(1).unwrap_or("")
                    .trim()
                    .trim_matches(|c| c == '[' || c == ']' || c == ',')
                    .split(',')
                    .filter_map(|s| s.trim().parse::<u8>().ok())
                    .collect()
            })
    }

    pub fn get(&self, mev_id: &str) -> Option<&CartridgeMeta> {
        self.cartridges.get(mev_id)
    }

    pub fn all(&self) -> Vec<&CartridgeMeta> {
        let mut v: Vec<_> = self.cartridges.values().collect();
        v.sort_by(|a, b| a.mev_id.cmp(&b.mev_id));
        v
    }

    pub fn len(&self) -> usize { self.cartridges.len() }
    pub fn is_empty(&self) -> bool { self.cartridges.is_empty() }
}

impl Default for StrategyRegistry {
    fn default() -> Self { Self::new() }
}
