/**
 * =============================================================================
 * OMEGA Audit Trail Types
 * =============================================================================
 *
 * Immutable record of every mutating action performed on the operational registry.
 *
 * The audit trail is append-only, partitioned by month, and replicated to
 * cold storage within 5 minutes of write.  It is the forensic backbone of
 * the R8 Fail-Honest guarantee: every decision can be reconstructed.
 *
 * Ghost Protocol: ACTIVO  |  R8 Fail-Honest: PRESERVADO
 *
 * @module audit/types
 */

// ---------------------------------------------------------------------------
// Primitive audit types
// ---------------------------------------------------------------------------

/** The action verbs that generate audit entries. */
export type AuditAction =
  | "create"
  | "update"
  | "delete"
  | "enable"
  | "disable"
  | "reload"
  | "rollback";

/** The entity types that can appear in an audit record. */
export type AuditEntityType =
  | "chain"
  | "dex"
  | "pool"
  | "token"
  | "wallet"
  | "strategy"
  | "risk_gate"
  | "relay";

/** Scope of influence of the audited action. */
export type AuditScope =
  | "global"
  | "per_chain"
  | "per_wallet"
  | "per_strategy";

/** Outcome classification of the audited action. */
export type AuditResult = "success" | "partial" | "failed";

// ---------------------------------------------------------------------------
// Core audit event
// ---------------------------------------------------------------------------

/**
 * A single immutable record in the audit trail.
 *
 * Every field is populated at write time and never mutated afterwards.
 * The idempotency_key links audit events to state-machine runs so that
 * retries do not create duplicate records.
 */
export interface AuditEvent {
  /** UUID v4 primary key of this audit record. */
  id: string;
  /** Actor that initiated the action (username, service account, or "system"). */
  actor: string;
  /** The action verb. */
  action: AuditAction;
  /** The entity type that was targeted. */
  entity_type: AuditEntityType;
  /** UUID or primary key of the targeted entity. */
  entity_id: string;
  /** Chain context, if applicable (undefined for global-scope actions). */
  chain_id?: number;
  /** Scope of influence. */
  scope: AuditScope;
  /** Entity state before the mutation (deep clone, undefined for create). */
  before?: unknown;
  /** Entity state after the mutation (deep clone, undefined for delete). */
  after?: unknown;
  /** Config hash before the mutation (undefined for create). */
  config_hash_before?: string;
  /** Config hash after the mutation (undefined for delete). */
  config_hash_after?: string;
  /** Idempotency key that ties this audit record to a state-machine run. */
  idempotency_key: string;
  /** Outcome classification. */
  result: AuditResult;
  /** Error message when result is "failed" or "partial". */
  error?: string;
  /** ISO-8601 timestamp when the event was recorded. */
  timestamp: string;
}

// ---------------------------------------------------------------------------
// Audit query filters (for forensic search UI)
// ---------------------------------------------------------------------------

/**
 * Parameters accepted by the audit search endpoint.
 *
 * All fields are optional; omitted fields mean "no filter".
 */
export interface AuditQueryFilters {
  /** Filter by actor (exact match or prefix). */
  actor?: string;
  /** Filter by action verb. */
  action?: AuditAction;
  /** Filter by entity type. */
  entity_type?: AuditEntityType;
  /** Filter by specific entity id. */
  entity_id?: string;
  /** Filter by chain context. */
  chain_id?: number;
  /** Filter by scope. */
  scope?: AuditScope;
  /** Filter by result. */
  result?: AuditResult;
  /** ISO-8601 lower bound (inclusive). */
  from?: string;
  /** ISO-8601 upper bound (inclusive). */
  to?: string;
  /** Pagination: page number (1-based). */
  page?: number;
  /** Pagination: items per page (max 100). */
  page_size?: number;
  /** Sort direction. */
  sort?: "asc" | "desc";
}

// ---------------------------------------------------------------------------
// Audit query response
// ---------------------------------------------------------------------------

/**
 * Paginated response returned by the audit search endpoint.
 */
export interface AuditQueryResponse {
  /** Matching records for the current page. */
  events: AuditEvent[];
  /** Total number of records matching the filters (ignoring pagination). */
  total: number;
  /** Current page number. */
  page: number;
  /** Items per page. */
  page_size: number;
  /** Total pages available. */
  total_pages: number;
  /** ISO-8601 timestamp when the query was executed. */
  queried_at: string;
}

// ---------------------------------------------------------------------------
// Audit statistics (dashboard tile data)
// ---------------------------------------------------------------------------

/**
 * Aggregated metrics shown on the audit dashboard.
 */
export interface AuditStats {
  /** Total audit events in the selected time window. */
  total_events: number;
  /** Breakdown by action verb. */
  by_action: Record<AuditAction, number>;
  /** Breakdown by result. */
  by_result: Record<AuditResult, number>;
  /** Breakdown by entity type. */
  by_entity_type: Record<AuditEntityType, number>;
  /** Number of distinct actors who performed actions. */
  unique_actors: number;
  /** Time window start. */
  window_start: string;
  /** Time window end. */
  window_end: string;
}

// ---------------------------------------------------------------------------
// Audit export request
// ---------------------------------------------------------------------------

/**
 * Parameters for exporting audit data to cold-storage formats (CSV, NDJSON).
 */
export interface AuditExportRequest {
  /** Output format. */
  format: "csv" | "ndjson";
  /** Filters to apply before export. */
  filters: AuditQueryFilters;
  /** S3 or filesystem destination path. */
  destination: string;
  /** ISO-8601 timestamp when the export was requested. */
  requested_at: string;
  /** Actor who requested the export. */
  requested_by: string;
}

// ---------------------------------------------------------------------------
// Audit retention configuration
// ---------------------------------------------------------------------------

/**
 * Tiered retention policy for the audit trail.
 */
export interface AuditRetentionConfig {
  /** Days to keep hot (PostgreSQL) records. */
  hot_retention_days: number;
  /** Days to keep warm (S3 / object storage) records. */
  warm_retention_days: number;
  /** Days to keep cold (glacier / tape) records (0 = infinite). */
  cold_retention_days: number;
  /** Whether to encrypt warm and cold copies. */
  encryption_enabled: boolean;
  /** Whether to compress warm and cold copies. */
  compression_enabled: boolean;
}
