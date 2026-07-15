export interface PotentialGradient {
  token_in: string;
  token_out: string;
  potential_delta_usd: number;
  venue_in: string;
  venue_out: string;
}

export interface DissipationMetrics {
  gas_usd: number;
  fee_bps: number;
  latency_ms: number;
  decoherence_usd: number;
}

export type CycleStatus = "detected" | "simulated" | "paper_executed" | "rejected";

export interface PermittedCycle {
  id: string;
  chain_id: number;
  detected_at: string;
  eta: number;
  work_extracted_usd: number;
  heat_in_usd: number;
  heat_out_usd: number;
  gradient: PotentialGradient;
  dissipation: DissipationMetrics;
  status: CycleStatus;
  rejection_reason?: string;
}

export interface ThermodynamicSnapshot {
  updated_at: string;
  cycles: PermittedCycle[];
  max_gradient: number;
  avg_eta: number;
  rejection_rate: number;
}
