//! Compile-time build facts for sim-ctl's GET /capabilities report (G-SIM-1
//! FASE 1). RULE 00 Zero-Mocks: every emitted value is DERIVED from the
//! checkout or the workspace lockfile; when a fact cannot be derived, NOTHING
//! is emitted and `src/capabilities.rs` serializes an honest null via
//! `option_env!`.
//!
//! Emits (consumed in src/capabilities.rs):
//! - `ARBX_BUILD_SHA` — `git rev-parse HEAD` of this checkout. Fallback: the
//!   ambient `ARBX_BUILD_SHA` env var, so CI or a docker build without a
//!   .git dir can inject the true SHA. Neither source yields a sha-shaped
//!   value → nothing is emitted (null, never a guess).
//! - `GSIM_REVM_VERSION` — the version of the package named exactly `revm`
//!   resolved in the workspace Cargo.lock, when exactly one copy exists.
//! - `GSIM_ALLOY_PRIMITIVES_VERSION` — the version of `alloy-primitives`
//!   resolved in the workspace Cargo.lock. The lockfile legitimately carries
//!   several semver-incompatible copies at once (revm 3.5 links 0.4.x via
//!   revm-primitives; simulator-v2 pins the 0.7 line; newer alloy-* crates
//!   link 1.x), so a bare name scan cannot pick one without inventing a
//!   false singular. Disambiguation rule: report the copy the simulator
//!   crate itself (simulator-v2) links — the crate whose capabilities this
//!   endpoint reports. Unresolvable → nothing (null).

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // Rebuild when the derivation inputs change. NOTE: a new HEAD alone does
    // not retrigger the script; fresh CI checkouts build from scratch (where
    // SHA accuracy matters), and local stale-SHA binaries are cosmetic only.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../Cargo.lock");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let manifest_dir = Path::new(&manifest_dir);

    if let Some(sha) = derive_build_sha(manifest_dir) {
        // `sha` is validated hex-only, single line — it cannot smuggle a
        // newline into this directive.
        println!("cargo:rustc-env=ARBX_BUILD_SHA={sha}");
    }

    // ../Cargo.lock = the workspace root lockfile (CARGO_MANIFEST_DIR is
    // backend/sim-ctl). Unreadable lockfile (never under --locked) → emit
    // nothing; capabilities.rs reports honest nulls.
    let Ok(lock) = fs::read_to_string(manifest_dir.join("..").join("Cargo.lock")) else {
        return;
    };

    if let Some(v) = unique_locked_version(&lock, "revm") {
        println!("cargo:rustc-env=GSIM_REVM_VERSION={v}");
    }
    if let Some(v) = alloy_primitives_version(&lock) {
        println!("cargo:rustc-env=GSIM_ALLOY_PRIMITIVES_VERSION={v}");
    }
}

/// `git rev-parse HEAD`; on any failure (no git binary, no .git dir — e.g. a
/// docker COPY without the repo), the ambient `ARBX_BUILD_SHA` env var so
/// CI/docker can inject the SHA. None when neither source is sha-shaped.
fn derive_build_sha(manifest_dir: &Path) -> Option<String> {
    if let Ok(out) = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(manifest_dir)
        .output()
    {
        if out.status.success() {
            if let Some(sha) = sha_shaped(String::from_utf8_lossy(&out.stdout).trim()) {
                return Some(sha);
            }
        }
    }
    env::var("ARBX_BUILD_SHA")
        .ok()
        .and_then(|v| sha_shaped(v.trim()))
}

/// Validate + own a hex git-object sha (40-64 hex chars). The validation is
/// not decoration: a newline-bearing ARBX_BUILD_SHA must never reach a
/// `cargo:rustc-env=` directive, where it could inject extra directives.
fn sha_shaped(raw: &str) -> Option<String> {
    if (40..=64).contains(&raw.len()) && raw.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(raw.to_string())
    } else {
        None
    }
}

/// The version of package `name` resolved in the lockfile, when EXACTLY ONE
/// copy exists. Multiple copies → None (no false singular; the alloy-primitives
/// case disambiguates deliberately, see [`alloy_primitives_version`]).
fn unique_locked_version(lock: &str, name: &str) -> Option<String> {
    match locked_versions(lock, name).as_slice() {
        [only] => Some(only.clone()),
        _ => None,
    }
}

/// All `version = "..."` entries of `[[package]]` blocks whose name is
/// exactly `name` — a simple adjacent-line scan of the machine-generated
/// lockfile (format: `name = "x"` immediately followed by `version = "y"`).
fn locked_versions(lock: &str, name: &str) -> Vec<String> {
    let wanted = format!("name = \"{name}\"");
    let mut versions = Vec::new();
    let mut lines = lock.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if line == wanted {
            if let Some(v) = lines
                .next()
                .and_then(|l| l.strip_prefix("version = "))
                .map(|v| v.trim_matches('"').to_string())
            {
                versions.push(v);
            }
        }
    }
    versions
}

/// The alloy-primitives version to REPORT. Exactly one resolved copy → that
/// one; several copies (today's reality) → the copy simulator-v2 itself
/// links; none → None.
fn alloy_primitives_version(lock: &str) -> Option<String> {
    match locked_versions(lock, "alloy-primitives").as_slice() {
        [] => None,
        [only] => Some(only.clone()),
        _ => simulator_linked_version(lock, "alloy-primitives"),
    }
}

/// The version of `dep` listed inside the `[[package]] name = "simulator-v2"`
/// block's dependencies — the lockfile's disambiguation syntax `"dep x.y.z"`.
fn simulator_linked_version(lock: &str, dep: &str) -> Option<String> {
    let wanted = "name = \"simulator-v2\"";
    let prefix = format!("\"{dep} ");
    let mut in_block = false;
    for line in lock.lines() {
        let line = line.trim();
        if line.starts_with("[[package]]") {
            if in_block {
                // Left simulator-v2's block without a matching dep entry.
                return None;
            }
        } else if line == wanted {
            in_block = true;
        } else if in_block {
            if let Some(rest) = line.strip_prefix(&prefix) {
                let v = rest.trim_end_matches(',').trim_end_matches('"');
                if v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}
