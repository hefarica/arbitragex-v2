import { XRayCard } from "@/components/XRayCard";
import { StatCard } from "@/components/StatCard";
import { GateSection } from "@/components/GateSection";

const opportunities = [
  {
    pair: "WETH/USDC",
    yield: "+0.42%",
    confidence: 87,
    legs: 2,
    ago: "4s ago",
    route: "UNI-V3 → SUSHI-V2",
    fees: "pool 0.30% + gas 0.018%",
    tlsAmount: "12.4 WETH",
    simVerdict: "revm-pass · 3ms",
    safetyA: 92,
    safetyB: 88,
  },
  {
    pair: "ARB/WETH",
    yield: "+0.18%",
    confidence: 74,
    legs: 3,
    ago: "7s ago",
    route: "CAMELOT → UNI-V3",
    fees: "pool 0.25% + gas 0.021%",
    tlsAmount: "— (no TLS)",
    simVerdict: "revm-pass · 4ms",
    safetyA: 88,
    safetyB: 90,
  },
  {
    pair: "WBTC/USDC",
    yield: "+0.31%",
    confidence: 91,
    legs: 2,
    ago: "9s ago",
    route: "UNI-V3 → BAL-V2",
    fees: "pool 0.30% + gas 0.019%",
    tlsAmount: "3.1 WBTC",
    simVerdict: "revm-pass · 3ms",
    safetyA: 95,
    safetyB: 84,
  },
];

export default function HomePage() {
  return (
    <div className="space-y-12">
      {/* Hero Section */}
      <section className="max-w-[980px]">
        <div className="font-mono text-[10.5px] tracking-widest uppercase text-[var(--primary)] mb-4">
          IA OMEGA · OBSERVE → SIMULATE → EXECUTE
        </div>

        <h1 className="text-[clamp(2.4rem,4.6vw,4rem)] font-semibold leading-[1.03] tracking-[-0.04em] mb-6">
          Convergencia estocástica.
          <br />
          <span className="text-[var(--primary-2)]">Topological Yield</span> en milisegundos.
        </h1>

        <p className="text-base leading-relaxed text-[var(--muted)] max-w-[64ch]">
          El motor observa <b className="text-[var(--foreground)] font-medium">50 rutas de Liquidity Manifolds</b> en paralelo,
          resuelve <b className="text-[var(--foreground)] font-medium">Asimetría Topológica</b> bajo
          <b className="text-[var(--foreground)] font-medium">Temporal Liquidity Superposition</b>,
          y mantiene el capital expuesto en <b className="text-[var(--foreground)] font-medium">$0.00</b> hasta que cada gate
          institucional esté en verde. Doctrina OMEGA: honestidad antes que teatro.
        </p>
      </section>

      {/* Stats Grid */}
      <section className="stats-grid">
        <StatCard
          label="Topological Yield · 24h"
          value={0.42}
          subtext="proyectado · REVM verified"
          variant="success"
          decimals={2}
          suffix="%"
        />

        <StatCard
          label="Asimetrías detectadas"
          value={1284}
          subtext="stream arbx:opps:detected"
          variant="accent"
          decimals={0}
        />

        <StatCard
          label="Capital expuesto"
          value={0}
          subtext="paper-shadow · estructural"
          decimals={0}
          prefix="$"
          suffix=".00"
        />

        <StatCard
          label="Decoherencia media"
          value={0.21}
          subtext="slippage proyectado"
          decimals={2}
          suffix="%"
        />
      </section>

      {/* Opportunities Section */}
      <section>
        <div className="flex items-baseline gap-4 mb-5">
          <span className="font-mono text-[10.5px] tracking-widest uppercase text-[var(--muted)]">
            / opportunities · live
          </span>
          <h2 className="text-[22px] font-semibold tracking-[-0.02em]">
            Asimetrías Topológicas activas
          </h2>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-[18px]">
          {opportunities.map((opp) => (
            <XRayCard key={opp.pair} {...opp} />
          ))}
        </div>
      </section>

      {/* Gate Section */}
      <section>
        <div className="flex items-baseline gap-4 mb-5">
          <span className="font-mono text-[10.5px] tracking-widest uppercase text-[var(--muted)]">
            / live-readiness · doctrinal gate
          </span>
          <h2 className="text-[22px] font-semibold tracking-[-0.02em]">
            The Gate Refusal
          </h2>
        </div>

        <GateSection />
      </section>
    </div>
  );
}
