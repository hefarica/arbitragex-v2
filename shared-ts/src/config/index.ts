import { readFileSync, existsSync } from "node:fs";
import { parse as parseToml } from "smol-toml";
import { z } from "zod";
import Ajv from "ajv";
import addFormats from "ajv-formats";

const ChainCfg = z.object({
  chain_id: z.number().int().positive(),
  name: z.string().min(1),
  enabled: z.boolean(),
}).strict();

const RelayCfg = z.object({
  name: z.string().min(1),
  enabled: z.boolean(),
  chains: z.array(z.number().int().positive()).min(1),
}).strict();

const ScoringCfg = z.object({
  min_accept_score: z.number().min(0).max(100),
  weight_liquidity: z.number().min(0).max(1),
  weight_depth: z.number().min(0).max(1),
  weight_safety: z.number().min(0).max(1),
  weight_slippage: z.number().min(0).max(1),
  weight_gas: z.number().min(0).max(1),
  weight_risk: z.number().min(0).max(1),
}).strict();

const TokenSafetyCfg = z.object({
  provider: z.enum(["goplus","honeypot_is","internal_only"]),
  api_call_timeout_ms: z.number().int().min(100),
  ttl_seconds_ok: z.number().int().min(60),
  ttl_seconds_bad: z.number().int().min(60),
  min_acceptable_score: z.number().int().min(0).max(100),
}).strict();

const CircuitBreakerCfg = z.object({
  name: z.string().min(1),
  threshold: z.number().int().min(1),
  window_ms: z.number().int().min(100),
  cooldown_ms: z.number().int().min(100),
}).strict();

export const AppConfigSchema = z.object({
  system: z.object({
    env: z.enum(["development","staging","production","production-like"]),
    kill_switch_enabled_default: z.boolean(),
    service_name_prefix: z.string().optional(),
  }).strict(),
  risk: z.object({
    max_revert_rate_pct: z.number().nonnegative(),
    max_execution_variance_pct: z.number().nonnegative(),
    min_token_safety_score: z.number().int().min(0).max(100),
    simulation_required_for_new_routes: z.boolean(),
    max_gas_price_gwei: z.number().nonnegative(),
    max_slippage_pct: z.number().nonnegative(),
  }).strict(),
  execution: z.object({
    private_only: z.boolean(),
    max_parallel_executions: z.number().int().nonnegative(),
    retry_limit: z.number().int().nonnegative(),
  }).strict(),
  observability: z.object({
    prometheus_enabled: z.boolean(),
    structured_logs: z.boolean(),
    log_level: z.enum(["trace","debug","info","warn","error"]).optional(),
  }).strict(),
  chains: z.array(ChainCfg).min(1),
  relays: z.array(RelayCfg).min(1),
  scoring: ScoringCfg,
  token_safety: TokenSafetyCfg,
  circuit_breakers: z.array(CircuitBreakerCfg),
}).strict();

export type AppConfig = z.infer<typeof AppConfigSchema>;

export class ConfigError extends Error {
  constructor(public code: string, message: string) {
    super(message);
    this.name = "ConfigError";
  }
}

export function loadAppConfig(opts?: { path?: string; schemaPath?: string; validateSchema?: boolean }): AppConfig {
  const path = opts?.path ?? process.env["ARBX_CONFIG_PATH"] ?? "configs/app.toml";
  if (!existsSync(path)) {
    throw new ConfigError("config_not_found", `config not found at ${path}`);
  }
  const raw = readFileSync(path, "utf-8");
  let parsed: unknown;
  try {
    parsed = parseToml(raw);
  } catch (e) {
    throw new ConfigError("config_parse", `toml parse error: ${(e as Error).message}`);
  }

  const cfg = AppConfigSchema.parse(parsed);

  if (!cfg.chains.some(c => c.enabled)) {
    throw new ConfigError("no_chains_enabled",
      "no chains enabled in configs/app.toml — at least one must be enabled");
  }

  if (opts?.validateSchema || process.env["ARBX_VALIDATE_SCHEMA"] === "1") {
    const schemaPath = opts?.schemaPath ?? "configs/schemas/app.schema.json";
    if (existsSync(schemaPath)) {
      const schema = JSON.parse(readFileSync(schemaPath, "utf-8"));
      const ajv = new Ajv({ allErrors: true, strict: false });
      addFormats(ajv);
      const validate = ajv.compile(schema);
      if (!validate(cfg)) {
        throw new ConfigError("schema_invalid",
          `schema validation failed: ${JSON.stringify(validate.errors)}`);
      }
    }
  }

  return cfg;
}

/** Require an env var at boot. Throws ConfigError with redacted message. */
export function requireEnv(name: string): string {
  const v = process.env[name];
  if (v === undefined || v === "") {
    throw new ConfigError("env_missing", `required env var missing: ${name}`);
  }
  return v;
}

/** Require one of several env vars. Useful for "either INTERNAL_GETH_HTTP or RPC_HTTP_1". */
export function requireAnyEnv(...names: string[]): string {
  for (const n of names) {
    const v = process.env[n];
    if (v !== undefined && v !== "") return v;
  }
  throw new ConfigError("env_missing", `required: one of ${names.join(", ")}`);
}

export function enabledChains(cfg: AppConfig): number[] {
  return cfg.chains.filter(c => c.enabled).map(c => c.chain_id);
}

export function relaysForChain(cfg: AppConfig, chainId: number): AppConfig["relays"] {
  return cfg.relays.filter(r => r.enabled && r.chains.includes(chainId));
}
