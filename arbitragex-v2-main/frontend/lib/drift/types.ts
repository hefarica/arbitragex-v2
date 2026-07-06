/**
 * =============================================================================
 * OMEGA Drift Detection Types
 * =============================================================================
 *
 * Types for cross-source configuration consistency monitoring.
 *
 * The drift engine periodically snapshots the registry from every data source
 * (PostgreSQL, Redis, runtime memory, frontend cache, TOML files) and compares
 * config hashes.  Any mismatch triggers a DriftReport with reconciliation
 * recommendations.
 *
 * Ghost Protocol: ACTIVO  |  R8 Fail-Honest: PRESERVADO
 *
 * @module drift/types
 */

// ---------------------------------------------------------------------------
// Primitive drift-specific types
// ---------------------------------------------------------------------------

/** Every source that can hold a copy of the operational registry. */
export type DriftSource =
  | "postgresql"
  | "redis"
  | "runtime"
  | "frontend"
  | "toml";

/** Overall health status returned by a drift analysis pass. */
export type DriftStatus =
  | "consistent"
  | "partial"
  | "drift_detected"
  | "blocked";

/** Recommended remediation action for a single drifted entity. */
export type DriftAction =
  | "reconcile"   // Auto-merge: pick the newer hash and propagate
  | "reload"      // Discard local copy and reload from canonical source
  | "investigate"; // Human review required before any change

// ---------------------------------------------------------------------------
// Per-source snapshot
// ---------------------------------------------------------------------------

/**
 * A point-in-time capture of the registry as seen by a single data source.
 *
 * The `entities` map counts how many rows of each entity type exist in that
 * source.  Mismatched counts are an early signal of incomplete replication.
 */
export interface DriftSnapshot {
  /** Which data source produced this snapshot. */
  source: DriftSource;
  /** ISO-8601 timestamp when the snapshot was taken. */
  timestamp: string;
  /** SHA-256 hash of the canonical JSON representation of the full registry. */
  config_hash: string;
  /** Total number of entities across all types in this source. */
  entities_count: number;
  /** Breakdown per entity type: { chains: 6, dexes: 12, pools: 340, ... }. */
  entities: Record<string, number>;
}

// ---------------------------------------------------------------------------
// Single drift difference
// ---------------------------------------------------------------------------

/**
 * One specific entity that diverges between two data sources.
 *
 * The action field tells the guardian what to do about the mismatch.
 */
export interface DriftDifference {
  /** Canonical entity-type name (e.g. "token", "strategy"). */
  entity_type: string;
  /** UUID or primary-key identifier of the diverging entity. */
  entity_id: string;
  /** Human-readable label of the first source. */
  source_a: string;
  /** Human-readable label of the second source. */
  source_b: string;
  /** Config hash from source_a. */
  hash_a: string;
  /** Config hash from source_b. */
  hash_b: string;
  /** Recommended remediation. */
  action: DriftAction;
}

// ---------------------------------------------------------------------------
// Aggregated drift report
// ---------------------------------------------------------------------------

/**
 * Result of a full cross-source drift-analysis pass.
 *
 * Produced by the DriftEngine after comparing all registered snapshots.
 * The frontend dashboard renders this structure in real time.
 */
export interface DriftReport {
  /** Overall health verdict. */
  status: DriftStatus;
  /** All snapshots that participated in this analysis. */
  snapshots: DriftSnapshot[];
  /** Every individual divergence found (empty when consistent). */
  differences: DriftDifference[];
  /** Human-readable summary with next steps. */
  recommendation: string;
  // REPAIR c815ef9: campos emitidos por backend que la UI ya consumía;
  // se declaran como requeridos (DriftPanel asume su presencia).
  /** ISO-8601 timestamp of when this report was generated. */
  checked_at: string;
  /** Total entities compared during this pass. */
  total_entities_checked: number;
  /** Total individual differences across all entities. */
  total_differences: number;
  /** Labels of layers that responded successfully (e.g. ["postgres","redis"]). */
  layers_reachable: string[];
  /** Labels of layers that did not respond / are unhealthy. */
  layers_unreachable: string[];
}

// ---------------------------------------------------------------------------
// Drift engine configuration
// ---------------------------------------------------------------------------

/**
 * Tunable parameters that control how aggressively the drift engine reacts.
 */
export interface DriftEngineConfig {
  /** How often (ms) to run a full cross-source comparison. */
  interval_ms: number;
  /** Maximum age (ms) a snapshot may have before it is considered stale. */
  max_snapshot_age_ms: number;
  /** If true, automatically execute "reconcile" actions without human approval. */
  auto_reconcile: boolean;
  /** If true, automatically execute "reload" actions without human approval. */
  auto_reload: boolean;
  /** Sources that require manual approval for any remediation. */
  protected_sources: DriftSource[];
}

// ---------------------------------------------------------------------------
// Drift alert (for event-bus / WebSocket emission)
// ---------------------------------------------------------------------------

/**
 * A lightweight alert payload suitable for pushing over WebSocket
 * to connected frontend dashboards.
 */
export interface DriftAlert {
  alert_id: string;
  severity: "info" | "warning" | "critical";
  status: DriftStatus;
  source_a: DriftSource;
  source_b: DriftSource;
  differences_count: number;
  message: string;
  triggered_at: string;
}
