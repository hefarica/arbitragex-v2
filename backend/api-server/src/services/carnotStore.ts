import type { PermittedCycle, ThermodynamicSnapshot } from "@arbx/shared";

const MAX_CYCLES = 200;

export class CarnotStore {
  private cycles: PermittedCycle[] = [];
  onAdd?: (cycle: PermittedCycle) => void;

  add(cycle: PermittedCycle): void {
    this.cycles.push(cycle);
    if (this.cycles.length > MAX_CYCLES) {
      this.cycles.shift();
    }
    this.onAdd?.(cycle);
  }

  snapshot(): ThermodynamicSnapshot {
    const gradients = this.cycles.map((c) => c.gradient.potential_delta_usd);
    const etas = this.cycles.map((c) => c.eta);
    return {
      updated_at: new Date().toISOString(),
      cycles: [...this.cycles].reverse(),
      max_gradient: gradients.length ? Math.max(...gradients) : 0,
      avg_eta: etas.length ? etas.reduce((a, b) => a + b, 0) / etas.length : 0,
      rejection_rate: 0,
    };
  }

  recent(limit: number): PermittedCycle[] {
    return this.cycles.slice(-limit).reverse();
  }
}
