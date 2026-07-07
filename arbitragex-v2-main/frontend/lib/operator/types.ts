/**
 * Tipos del Operator Parametrization Sovereignty (C9.4).
 *
 * Mirror Law Extendida — frontend EXTIENDE pero NO modifica
 * layout.tsx, tailwind.config.ts, globals.css, components/ui/*.
 */

export type OperatorRole = 'observer' | 'steward' | 'sovereign';

export interface OperatorIdentity {
  operator_id: string;
  display_name: string;
  signing_pubkey: string;
  role: OperatorRole;
  cap_usd_ceiling: string;      // NUMERIC serializado
  allowed_chains: number[];
  allowed_registries: string[];
  ui_preferences: Record<string, unknown>;
  feature_overrides: Record<string, boolean>;
  config_hash: string;
  enabled: boolean;
}

export interface OperatorGates {
  can_mutate_registries: boolean;
  can_escalate_capital: boolean;
  can_promote_mainnet: boolean;
  can_modify_feature_manifest: boolean;
  can_view_audit: boolean;
  can_view_drift: boolean;
  can_trigger_hot_reload: boolean;
}

export interface FeatureManifestEntry {
  feature_key: string;
  ui_path: string;
  backend_route: string;
  requires_layers: string[];
  enabled: boolean;
  description: string;
  effective_enabled: boolean;
}

export interface OperatorMeResponse {
  operator: OperatorIdentity;
  gates: OperatorGates;
  feature_manifest: FeatureManifestEntry[];
  coherence_rule: string;
}

export const REGISTRY_KEYS = [
  'chain',
  'rpc',
  'dex',
  'token',
  'pool',
  'wallet',
  'strategy',
  'contract',
  'risk_gate',
  'capital_gate',
  'relay',
  'agent',
] as const;

export type RegistryKey = (typeof REGISTRY_KEYS)[number];
