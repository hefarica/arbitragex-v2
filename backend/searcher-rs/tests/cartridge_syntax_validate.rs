//! TEMP validation harness — compiles every cartridge `.rhai` under
//! `cartridges/` (root + strategies/) against the workspace Rhai engine
//! (1.19, no_optimize), mirroring how `cartridge::runner` compiles them at
//! boot. Catches template-wide syntax errors (e.g. Rust `as` casts) that a
//! single first-error boot log would hide one-at-a-time.
//!
//! Run: cargo test -p searcher-rs --test cartridge_syntax_validate -- --nocapture

use std::path::{Path, PathBuf};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if ft.is_dir() {
            collect(&path, out);
        } else if ft.is_file() && path.extension().map(|e| e == "rhai").unwrap_or(false) {
            out.push(path);
        }
    }
}

#[test]
fn all_cartridge_rhai_compile() {
    // Engine like the runner (cartridge/runner.rs:139-150): default construction
    // PLUS the pinned expression-depth limits (release profile values). Without
    // these, debug-profile default limits (32/16) are stricter than the runner's
    // and would false-fail the two root cartridges that legitimately compile in
    // production (funding_rate_arbitrage, triangular_arb).
    let mut engine = rhai::Engine::new();
    engine.set_max_expr_depths(64, 32);

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("cartridges");
    let mut files = Vec::new();
    collect(&dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .rhai cartridges found under {}", dir.display());

    let mut failures: Vec<String> = Vec::new();
    for path in &files {
        let src = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        if let Err(e) = engine.compile(&src) {
            failures.push(format!("{}: {e}", path.display()));
        }
    }

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("COMPILE FAIL: {f}");
        }
        panic!("{} of {} cartridges failed to compile", failures.len(), files.len());
    }
    println!("OK: all {} cartridges compiled", files.len());
}
