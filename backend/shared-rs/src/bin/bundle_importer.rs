//! bundle_importer — the single importer binary both trigger paths invoke.
//!
//! Ruta 1 (SSH): the operator scp's the .enc, then SSH-runs this binary.
//! Ruta 2 (HTTP): the api-server endpoint writes the .enc, then exec's this binary.
//!
//! One importer, two triggers. Decrypt + validate (shared-rs::config_bundle) +
//! idempotent apply (.env fragment merge + chains_runtime / rpc_endpoints / factories
//! upserts). NEVER touches paper_mode (filtered at 3 layers before we get here, and
//! re-asserted by load_bundle).
//!
//! Exit codes: 0 = OK; 1 = bad args; 2 = decrypt/schema/NEVER_SHIP rejection; 3 = apply error.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use shared_rs::config_bundle::{self, BundleError, ConfigBundle};

const ENV_PATH_DEFAULT: &str = "/opt/arbitragex-v2/.env";
const ENC_PATH_DEFAULT: &str = "/opt/arbitragex-v2/config/arbx_config_bundle.json.enc";
const PRIV_KEY_DEFAULT: &str = "/opt/arbitragex-v2/config/arbx_bundle_private.pem";
const SCHEMA_PATH_DEFAULT: &str =
    "/opt/arbitragex-v2/scripts/arbx-env-deploy/bundle_schema.json";

struct Args {
    enc: PathBuf,
    private_key: PathBuf,
    schema: PathBuf,
    env_path: PathBuf,
    dry_run: bool,
    database_url: Option<String>,
}

fn parse_args() -> Result<Args, ExitCode> {
    let mut enc = PathBuf::from(ENC_PATH_DEFAULT);
    let mut private_key = PathBuf::from(PRIV_KEY_DEFAULT);
    let mut schema = PathBuf::from(SCHEMA_PATH_DEFAULT);
    let mut env_path = PathBuf::from(ENV_PATH_DEFAULT);
    let mut dry_run = false;
    let mut database_url = std::env::var("DATABASE_URL").ok();

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--enc" => {
                i += 1;
                enc = PathBuf::from(&args[i]);
            }
            "--private-key" => {
                i += 1;
                private_key = PathBuf::from(&args[i]);
            }
            "--schema" => {
                i += 1;
                schema = PathBuf::from(&args[i]);
            }
            "--env-path" => {
                i += 1;
                env_path = PathBuf::from(&args[i]);
            }
            "--database-url" => {
                i += 1;
                database_url = Some(args[i].clone());
            }
            "--dry-run" => dry_run = true,
            "--apply" => dry_run = false,
            "-h" | "--help" => {
                eprintln!(
                    "bundle_importer --enc <path> --private-key <pem> --schema <json>\n  \
                     [--env-path <path>] [--database-url <url>|DATABASE_URL env]\n  \
                     [--dry-run|--apply]"
                );
                return Err(ExitCode::from(1));
            }
            other => {
                eprintln!("unknown arg: {other}");
                return Err(ExitCode::from(1));
            }
        }
        i += 1;
    }
    Ok(Args { enc, private_key, schema, env_path, dry_run, database_url })
}

#[derive(serde::Serialize)]
struct Report {
    schema_version: String,
    generated_at: String,
    env_vars: usize,
    chains: usize,
    factories: usize,
    api_keys: usize,
    contract_addresses: usize,
    mode: &'static str,
    apply: Option<ApplyReport>,
}

#[derive(serde::Serialize, Default)]
struct ApplyReport {
    env_inserted: usize,
    env_updated: usize,
    env_unchanged: usize,
    chains_upserted: usize,
    rpc_endpoints_upserted: usize,
    factories_upserted: usize,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(code) => return code,
    };

    let enc = match fs::read(&args.enc) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("FATAL: cannot read .enc ({}): {}", args.enc.display(), e);
            return ExitCode::from(1);
        }
    };
    let pem = match fs::read_to_string(&args.private_key) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FATAL: cannot read private key ({}): {}", args.private_key.display(), e);
            return ExitCode::from(1);
        }
    };
    let schema_json = match fs::read_to_string(&args.schema) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FATAL: cannot read schema ({}): {}", args.schema.display(), e);
            return ExitCode::from(1);
        }
    };

    let bundle = match config_bundle::load_bundle(&enc, &pem, &schema_json) {
        Ok(b) => b,
        Err(e) => {
            // Distinguish tamper/crypto failures (exit 2) from apply errors (exit 3).
            eprintln!("REJECTED: {e}");
            return match e {
                BundleError::AesDecrypt(_) | BundleError::RsaUnwrap(_) => ExitCode::from(2),
                BundleError::SchemaValidation(_) | BundleError::NeverShipLeak(_)
                | BundleError::BadSchemaVersion(_) | BundleError::BadJson(_) => ExitCode::from(2),
                _ => ExitCode::from(2),
            };
        }
    };

    let factories_total: usize = bundle.chains.iter().map(|c| c.factories.len()).sum();
    eprintln!(
        "bundle OK: {} chains, {} env_vars, {} factories, {} api_keys, {} contract_addrs",
        bundle.chains.len(),
        bundle.env_vars.len(),
        factories_total,
        bundle.api_keys.len(),
        bundle.contract_addresses.len(),
    );

    let mode = if args.dry_run { "dry-run" } else { "apply" };

    let apply = if args.dry_run {
        None
    } else {
        match apply_bundle(&bundle, &args).await {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("APPLY FAILED: {e}");
                return ExitCode::from(3);
            }
        }
    };

    let report = Report {
        schema_version: bundle.schema_version.clone(),
        generated_at: bundle.generated_at.clone(),
        env_vars: bundle.env_vars.len(),
        chains: bundle.chains.len(),
        factories: factories_total,
        api_keys: bundle.api_keys.len(),
        contract_addresses: bundle.contract_addresses.len(),
        mode,
        apply,
    };
    // The JSON report on stdout (the endpoint parses this; SSH users read it directly).
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_else(|e| format!("{{\"report_error\":\"{e}\"}}")));
    ExitCode::SUCCESS
}

/// Idempotent apply: .env fragment merge + DB upserts. paper_mode is NEVER in the bundle
/// (filtered at 3 layers), so writing env_vars cannot clobber it — but we belt-and-braces
/// skip any NEVER_SHIP key here too (defense-in-depth layer 4).
async fn apply_bundle(bundle: &ConfigBundle, args: &Args) -> Result<ApplyReport, String> {
    let mut report = ApplyReport::default();

    // 1. .env merge — read existing, upsert bundle env_vars + api_keys + contract_addrs.
    let mut merged: BTreeMap<String, String> = parse_env_file(&args.env_path)?;
    let mut all_vars: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in bundle.env_vars.iter().chain(bundle.api_keys.iter()) {
        if config_bundle::NEVER_SHIP.iter().any(|n| n.eq_ignore_ascii_case(k)) {
            eprintln!("WARN: NEVER_SHIP key '{}' present post-load_bundle — skipping (layer 4)", k);
            continue;
        }
        all_vars.insert(k.clone(), v.clone());
    }
    for (k, v) in bundle.contract_addresses.iter() {
        all_vars.insert(k.clone(), v.clone());
    }
    for (k, v) in all_vars.iter() {
        match merged.get(k) {
            Some(existing) if existing == v => report.env_unchanged += 1,
            Some(_) => {
                report.env_updated += 1;
                merged.insert(k.clone(), v.clone());
            }
            None => {
                report.env_inserted += 1;
                merged.insert(k.clone(), v.clone());
            }
        }
    }
    write_env_file(&args.env_path, &merged)?;

    // 2. DB upserts (chains + rpc_endpoints + factories). Skipped if no DATABASE_URL.
    if let Some(url) = &args.database_url {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(url)
            .await
            .map_err(|e| format!("db connect: {e}"))?;
        apply_db(&pool, bundle, &mut report).await?;
    } else {
        eprintln!("NOTE: DATABASE_URL not set — skipped DB upserts (.env merged only).");
    }

    Ok(report)
}

fn parse_env_file(path: &PathBuf) -> Result<BTreeMap<String, String>, String> {
    let mut map = BTreeMap::new();
    if !path.exists() {
        return Ok(map);
    }
    let content = fs::read_to_string(path).map_err(|e| format!("read .env: {e}"))?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    Ok(map)
}

fn write_env_file(path: &PathBuf, map: &BTreeMap<String, String>) -> Result<(), String> {
    // Backup the existing .env (non-destructive). Mirrors RunFullSyncCycle's .bak_* convention.
    if path.exists() {
        let bak = path.with_extension(format!("env.bak_{}", chrono::Utc::now().timestamp()));
        fs::rename(path, &bak).map_err(|e| format!("backup .env: {e}"))?;
    }
    let mut out = String::new();
    out.push_str(&format!(
        "# arbx .env - bundle_importer apply {}\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    ));
    for (k, v) in map {
        out.push_str(&format!("{k}={v}\n"));
    }
    let mut f = fs::File::create(path).map_err(|e| format!("create .env: {e}"))?;
    f.write_all(out.as_bytes()).map_err(|e| format!("write .env: {e}"))?;
    Ok(())
}

async fn apply_db(
    pool: &sqlx::PgPool,
    bundle: &ConfigBundle,
    report: &mut ApplyReport,
) -> Result<(), String> {
    for chain in &bundle.chains {
        // chains (INTEGER) vs chains_runtime/rpc_endpoints (BIGINT) — bind the right width.
        let cid_i32: i32 = chain.chain_id as i32;
        let cid_i64: i64 = chain.chain_id;

        // 1. chains catalog upsert - FK target for rpc_endpoints + factories. MUST come first
        //    (rpc_endpoints.chain_id REFERENCES chains.chain_id; without this row it FK-fails).
        //    Schema per migration 021: (chain_id INT UNIQUE, name, native_currency, explorer_url, is_active).
        sqlx::query(
            "INSERT INTO chains (chain_id, name, native_currency, explorer_url, is_active)
             VALUES ($1, $2, $3, $4, true)
             ON CONFLICT (chain_id) DO UPDATE SET
               name = EXCLUDED.name, native_currency = EXCLUDED.native_currency,
               explorer_url = EXCLUDED.explorer_url, is_active = true, updated_at = NOW()",
        )
        .bind(cid_i32)
        .bind(&chain.name)
        .bind(&chain.native_currency)
        .bind(&chain.explorer_url)
        .execute(pool)
        .await
        .map_err(|e| format!("chains catalog upsert chain {}: {e}", chain.chain_id))?;

        // 2. chains_runtime upsert (runtime cache; searcher-rs reads this). Schema per migration 061:
        //    (chain_id BIGINT UNIQUE, name, rpc_http_url, rpc_ws_url, native_currency, enabled).
        let http = first_provider(&chain.rpc_http);
        let ws = first_provider(&chain.rpc_ws);
        sqlx::query(
            "INSERT INTO chains_runtime
               (chain_id, name, rpc_http_url, rpc_ws_url, native_currency, enabled)
             VALUES ($1, $2, $3, $4, $5, true)
             ON CONFLICT (chain_id) DO UPDATE
               SET name = EXCLUDED.name, rpc_http_url = EXCLUDED.rpc_http_url,
                   rpc_ws_url = EXCLUDED.rpc_ws_url, native_currency = EXCLUDED.native_currency,
                   updated_at = NOW()",
        )
        .bind(cid_i64)
        .bind(&chain.name)
        .bind(http)
        .bind(ws)
        .bind(&chain.native_currency)
        .execute(pool)
        .await
        .map_err(|e| format!("chains_runtime upsert chain {}: {e}", chain.chain_id))?;
        report.chains_upserted += 1;

        // 3. rpc_endpoints upsert - one row per provider URL. Schema per migration 066:
        //    (chain_id BIGINT, url TEXT, transport CHECK IN ('http','https','ws','wss','ipc'), ...)
        //    UNIQUE (chain_id, url). transport derived from the URL scheme.
        for (_, url) in iter_csv(&chain.rpc_http) {
            let transport = transport_from_url(&url).unwrap_or("http");
            upsert_rpc_endpoint(pool, cid_i64, &url, transport).await?;
            report.rpc_endpoints_upserted += 1;
        }
        for (_, url) in iter_csv(&chain.rpc_ws) {
            let transport = transport_from_url(&url).unwrap_or("ws");
            upsert_rpc_endpoint(pool, cid_i64, &url, transport).await?;
            report.rpc_endpoints_upserted += 1;
        }

        // 4. factories upsert - FK via dexes.name subquery (UUID-independent, mirrors
        //    gen_chain_env.py). SELECT-from-dexes returns no rows if the dex is absent ->
        //    INSERT inserts nothing (silent skip), not an FK violation.
        for f in &chain.factories {
            sqlx::query(
                "INSERT INTO factories (dex_id, chain_id, address)
                 SELECT id, $2, $3 FROM dexes WHERE name = $1
                 ON CONFLICT (chain_id, address) DO NOTHING",
            )
            .bind(&f.dex_name)
            .bind(cid_i32)
            .bind(&f.address)
            .execute(pool)
            .await
            .map_err(|e| format!("factories upsert {} on chain {}: {e}", f.dex_name, chain.chain_id))?;
            report.factories_upserted += 1;
        }
    }
    Ok(())
}

async fn upsert_rpc_endpoint(
    pool: &sqlx::PgPool,
    chain_id: i64,
    url: &str,
    transport: &str,
) -> Result<(), String> {
    // rpc_endpoints (migration 066): chain_id, url, transport — NO provider/kind/enabled columns.
    sqlx::query(
        "INSERT INTO rpc_endpoints (chain_id, url, transport)
         VALUES ($1, $2, $3)
         ON CONFLICT (chain_id, url) DO UPDATE SET transport = EXCLUDED.transport",
    )
    .bind(chain_id)
    .bind(url)
    .bind(transport)
    .execute(pool)
    .await
    .map_err(|e| format!("rpc_endpoints upsert {url} on chain {chain_id}: {e}"))?;
    Ok(())
}

/// Derive the transport tag from the URL scheme. Matches the CHECK constraint on
/// rpc_endpoints.transport ('http','https','ws','wss','ipc') and the RpcSyncPanel client logic.
fn transport_from_url(url: &str) -> Option<&'static str> {
    let s = url.trim().to_ascii_lowercase();
    if s.starts_with("wss://") {
        Some("wss")
    } else if s.starts_with("ws://") {
        Some("ws")
    } else if s.starts_with("https://") {
        Some("https")
    } else if s.starts_with("http://") {
        Some("http")
    } else {
        None
    }
}

/// Extract the first provider's URL from a "prov=url,prov=url" CSV (chains_runtime.rpc_http_url
/// is a single-URL column per the admin-chains schema; the full multi-provider CSV lives in rpc_endpoints).
fn first_provider(csv: &str) -> Option<String> {
    iter_csv(csv).next().map(|(_, url)| url)
}

/// Iterate a "prov=url,prov=url" CSV → (provider, url).
fn iter_csv(csv: &str) -> impl Iterator<Item = (String, String)> + '_ {
    csv.split(',').filter_map(|pair| {
        let pair = pair.trim();
        let (p, u) = pair.split_once('=')?;
        Some((p.trim().to_string(), u.trim().to_string()))
    })
}
