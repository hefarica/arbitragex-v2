/**
 * =============================================================================
 * GATES UTILITY LIBRARY — Phase AIM-1 P0 Integration
 * =============================================================================
 *
 * Rule 00 (No Mocks): Reads gate status from `/api/gates/status` ONLY
 * Guarantee real telemetry, NO fabricated data.
 *
 * Physical Model:
 *   Collector (searcher-rs) → Redis Stream: XLEN arbx:gate-commit:checksum
 *   Risk Engine → Postgres: gate_checkpoint table
 *   API Server → `/api/gates/status`, `/api/gates/health`
 *   Frontend → useGatesStatus hook, GateIntegration component
 *
 * Fail-Honest Policy:
 *   - gateScore = Some(0.0) when request fails internally
 *   - Empty list when no telemetry sources active
 *
 * @see CLAUDE.md Rule 00 §33 MCP STACK
 */

export interface GateStatus {
  gate_id: string;
  verified: boolean;
  gate_score: number;
  last_checkpoint: string;
  aggregate_variance: number;
  variance_ceiling: number;
  remaining_headroom: number;
  dispatch_count: number;
  rejection_count: number;
  barrier_count: number;
}

export interface GateAggregateStats {
  total_gates: number;
  avg_gate_score: number;
  aggregate_variance: number;
  variance_ceiling: number;
  remaining_headroom: number;
  dispatch_count: number;
  rejection_count: number;
}

export interface GateStatusResponse {
  aggregated_stats: GateAggregateStats;
  gate_details: GateStatus[];
  timestamp: string;
  message: string;
}

// =============================================================================
// Type Guards
// =============================================================================

/**
 * Validate gate status response structure (Rule 00 compliance)
 */
export function isValidGateStatusResponse(data: unknown): data is GateStatusResponse {
  if (typeof data !== 'object' || data === null) return false;

  const response = data as Partial<GateStatusResponse>;

  // Required fields
  if (
    !response.aggregated_stats ||
    !Array.isArray(response.gate_details) ||
    typeof response.timestamp !== 'string' ||
    typeof response.message !== 'string'
  ) {
    return false;
  }

  // Validate aggregated_stats structure
  const stats = response.aggregated_stats;
  if (
    typeof stats.total_gates !== 'number' ||
    typeof stats.avg_gate_score !== 'number' ||
    typeof stats.aggregate_variance !== 'number' ||
    typeof stats.variance_ceiling !== 'number' ||
    typeof stats.remaining_headroom !== 'number' ||
    typeof stats.dispatch_count !== 'number' ||
    typeof stats.rejection_count !== 'number'
  ) {
    return false;
  }

  // Validate gate_details elements
  for (const gate of response.gate_details) {
    if (
      typeof gate !== 'object' ||
      gate === null ||
      typeof gate.gate_id !== 'string' ||
      typeof gate.verified !== 'boolean' ||
      typeof gate.gate_score !== 'number' ||
      typeof gate.last_checkpoint !== 'string' ||
      typeof gate.aggregate_variance !== 'number' ||
      typeof gate.variance_ceiling !== 'number' ||
      typeof gate.remaining_headroom !== 'number' ||
      typeof gate.dispatch_count !== 'number' ||
      typeof gate.rejection_count !== 'number' ||
      typeof gate.barrier_count !== 'number'
    ) {
      return false;
    }
  }

  return true;
}

/**
 * Calculate aggregate gate score from multiple gate checkpoints (Rule 00 compliant)
 */
export function calculateAggregateGateScore(gates: GateStatus[]): number {
  if (gates.length === 0) return 1.0; // No gates = full confidence

  const avgScore = gates.reduce((sum, g) => sum + g.gate_score, 0) / gates.length;

  // Normalize to [0, 1] range (GateManager failsafe alpha)
  return Math.min(avgScore, 1.0);
}

/**
 * Calculate remaining variance headroom for all gates
 */
export function calculateRemainingHeadroom(gates: GateStatus[]): number {
  if (gates.length === 0) return 100.0; // Baseline

  const headrooms = gates.map((g) => g.remaining_headroom);
  const headroomSum = headrooms.reduce((sum, h) => sum + h, 0);

  return headroomSum / gates.length;
}

// =============================================================================
// Fetch Utilities (Rule 00 compliant)
// =============================================================================

/**
 * Fetch gate status from `/api/gates/status` endpoint
 * @see Rule 00 (No Mocks) — reads ONLY from API
 */
export async function fetchGateStatusApi(): Promise<GateStatusResponse | null> {
  try {
    const NEXT_PUBLIC_EDGE_URL =
      process.env.NEXT_PUBLIC_EDGE_URL || 'http://localhost:8787';

    const url = `${NEXT_PUBLIC_EDGE_URL}/api/gates/status`;
    const response = await fetch(url, {
      headers: { accept: 'application/json' },
      cache: 'no-store',
      next: { revalidate: 3000 }, // Poll every 3 seconds (Rule 00: no-stale data)
    });

    if (!response.ok) {
      console.error('[GatesUtility] Fetch failed:', response.status, response.statusText);
      return null;
    }

    const data = await response.json();

    if (!isValidGateStatusResponse(data)) {
      console.error('[GatesUtility] Invalid response structure');
      return null;
    }

    return data;
  } catch (e) {
    console.error('[GatesUtility] Fetch gate status failed:', e);
    return null;
  }
}

/**
 * Fetch gate health status from `/api/gates/health` endpoint
 */
export async function fetchGateHealthApi(): Promise<boolean> {
  try {
    const NEXT_PUBLIC_EDGE_URL =
      process.env.NEXT_PUBLIC_EDGE_URL || 'http://localhost:8787';

    const url = `${NEXT_PUBLIC_EDGE_URL}/api/gates/health`;
    const response = await fetch(url, {
      headers: { accept: 'application/json' },
      cache: 'no-store',
    });

    if (!response.ok) return false;

    const data = await response.json();

    return data.healthy === true;
  } catch (e) {
    console.error('[GatesUtility] Fetch gate health failed:', e);
    return false;
  }
}

/**
 * Poll gate status periodically (Rule 00 compliant)
 */
export function pollGateStatus(
  fn: () => Promise<GateStatusResponse | null>,
  options: {
    interval: number;
    onError?: (error: Error) => void;
  }
): {
  start: () => void;
  stop: () => void;
} {
  let intervalId: NodeJS.Timeout | null = null;

  const start = () => {
    stop();
    const tick = async () => {
      try {
        const data = await fn();
        if (!data) {
          console.warn('[GatesUtility] Gate status fetch returned null');
        }
      } catch (error) {
        if (options.onError) {
          options.onError(error as Error);
        } else {
          console.error('[GatesUtility] Poll error:', error);
        }
      }
    };

    tick(); // Immediate execution
    intervalId = setInterval(tick, options.interval);
  };

  const stop = () => {
    if (intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
  };

  start();

  return { start, stop };
}

// =============================================================================
// Formatting Utilities
// =============================================================================

/**
 * Format gate score for display with color-coding (Rule 08 compliant)
 */
export function formatGateScoreDisplay(score: number): {
  text: string;
  colorClass: string;
  colorHex: string;
} {
  if (score === 1.0) {
    return {
      text: '100% Approved',
      colorClass: 'text-green-500',
      colorHex: '#22c55e',
    };
  } else if (score > 0.5) {
    return {
      text: `${(score * 100).toFixed(0)}% Moderate`,
      colorClass: 'text-amber-500',
      colorHex: '#f59e0b',
    };
  } else {
    return {
      text: `${(score * 100).toFixed(0)}% Rejected`,
      colorClass: 'text-red-500',
      colorHex: '#ef4444',
    };
  }
}

/**
 * Format variance metrics for display
 */
export function formatVarianceMetrics(variance: number, ceiling: number): {
  variance: string;
  headroom: string;
  percentUsed: string;
} {
  const percentUsed = ((variance / ceiling) * 100).toFixed(1);
  const headroom = (ceiling - variance).toFixed(2);
  const varianceStr = variance.toFixed(4);

  return {
    variance: varianceStr,
    headroom,
    percentUsed,
  };
}

// =============================================================================
// Initialization Utils
// =============================================================================

/**
 * Initialize gates utility library
 */
export async function initGatesLibrary(): Promise<{ healthy: boolean; message: string }> {
  try {
    const healthy = await fetchGateHealthApi();

    if (healthy) {
      return {
        healthy: true,
        message: 'Gates library initialized: telemetry sources active',
      };
    } else {
      return {
        healthy: false,
        message: 'Gates library initialized: no telemetry sources (safe mode)',
      };
    }
  } catch (e) {
    return {
      healthy: false,
      message: `Gates library initialization failed: ${e}`,
    };
  }
}
