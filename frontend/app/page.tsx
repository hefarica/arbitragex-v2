import { XRayCard } from "@/components/XRayCard";
import { StatCard } from "@/components/StatCard";
import { GateSection } from "@/components/GateSection";
// FE-0042: store-only aggregation island (client component inside this
// server page — standard Next App Router island pattern).
import { HomeStoreAggregationContainer } from "@/components/home/HomeStoreAggregation";
import { getApiBaseUrl, getReadinessDecision } from "@/lib/api-client";
import { toXRayProps } from "@/lib/home-opportunity";
import type { OpportunityRow } from "@/lib/schemas";

export const dynamic = "force-dynamic";
export const revalidate = 0;

// RULE 00 (Zero Mocks): the home page previously rendered a hardcoded array of
// 3 fake opportunities + 4 fabricated StatCards. That is a direct doctrine
// violation. This Server Component now fetches the REAL live feed from the
// edge and renders exactly what the API returns — an empty array renders an
// honest empty state, never invented data (R8 fail-honest).

interface HomeData {
  opportunities: OpportunityRow[];
  source: "server-snapshot" | "server-fetch-failed";
}

async function getHomeData(): Promise<HomeData> {
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  try {
    const res = await fetch(`${EDGE_URL}/api/opportunities/live?limit=50&order=profit_usd`, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!res.ok) {
      return { opportunities: [], source: "server-fetch-failed" };
    }
    const data = await res.json();
    const items: OpportunityRow[] = Array.isArray(data?.items)
      ? data.items
      : Array.isArray(data)
        ? data
        : [];
    return { opportunities: items, source: "server-snapshot" };
  } catch {
    return { opportunities: [], source: "server-fetch-failed" };
  }
}

export default async function HomePage() {
  const [{ opportunities, source }, decisionRes] = await Promise.all([
    getHomeData(),
    getReadinessDecision(),
  ]);
  const failed = source === "server-fetch-failed";

  // Derived honest stats — computed from the real payload, never fabricated.
  const detectedCount = opportunities.length;
  const nets = opportunities
    .map((o) => o.net_expected_profit_usd ?? o.simulated_net_profit_usd ?? null)
    .filter((v): v is number => v != null);
  const rois = opportunities.map((opp) => opp.roi_pct)
    .filter((roi): roi is number => roi != null && Number.isFinite(roi));
  const avgRoi = rois.length > 0 ? rois.reduce((sum, roi) => sum + roi, 0) / rois.length : null;
  const bestNet = nets.length > 0 ? Math.max(...nets) : null;

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
          El motor observa <b className="text-[var(--foreground)] font-medium">rutas de Liquidity Manifolds</b> en paralelo,
          resuelve <b className="text-[var(--foreground)] font-medium">Asimetría Topológica</b> bajo
          <b className="text-[var(--foreground)] font-medium"> Temporal Liquidity Superposition</b>,
          y mantiene el capital expuesto en <b className="text-[var(--foreground)] font-medium">$0.00</b> hasta que cada gate
          institucional esté en verde. Doctrina OMEGA: honestidad antes que teatro.
        </p>
      </section>

      {/* Stats Grid — values derived from the real API payload. A metric the
          API cannot supply renders "—" (R8), never an invented number. */}
      <section className="stats-grid">
        <StatCard
          label="Mejor Topological Yield · neto"
          value={bestNet != null ? bestNet.toFixed(4) : "—"}
          subtext={bestNet != null ? "neto · USD (spine/sim)" : "sin datos — feed vacío"}
          variant="success"
          animate={bestNet != null}
          prefix={bestNet != null ? "$" : ""}
        />

        <StatCard
          label="Asimetrías detectadas"
          value={detectedCount}
          subtext="stream arbx:opps:detected"
          variant="accent"
          decimals={0}
          animate={detectedCount > 0}
        />

        <StatCard
          label="Capital expuesto"
          value={0}
          subtext="paper-shadow · estructural"
          decimals={0}
          prefix="$"
          suffix=".00"
          animate={false}
        />

        <StatCard
          label="Decoherencia media (Convergence Ratio)"
          value={avgRoi != null ? avgRoi.toFixed(2) : "—"}
          subtext={avgRoi != null ? "roi_pct medio del feed" : "sin datos — feed vacío"}
          animate={avgRoi != null}
          suffix={avgRoi != null ? "%" : ""}
        />
      </section>

      {/* FE-0042 (§58 §59): Home aggregation FROM STORES ONLY — the client
          island below fetches nothing and recomputes no business logic; it
          tallies what ArbxRealtimeProvider (root layout) already hydrates.
          Missing slices (EV, risk) render declared gaps, never inventions. */}
      <section>
        <HomeStoreAggregationContainer />
      </section>

      {/* Opportunities Section */}
      <section>
        <div className="flex items-baseline gap-4 mb-5">
          <span className="font-mono text-[10.5px] tracking-widest uppercase text-[var(--muted)]">
            / opportunities · live
          </span>
          <h2 className="text-[22px] font-semibold tracking-[-0.02em]">
            Asimetrías Topológicas detectadas
          </h2>
          {/* AUDIT-2026-08-29: this feed is raw DETECTION (candidates pre-gate),
              not curated "active" arbitrage — the qualifier below says so. */}
          <span className="font-mono text-[10px] tracking-wide text-[var(--muted)]">
            candidatas pre-gate · observación
          </span>
        </div>

        {opportunities.length === 0 ? (
          <div className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--foreground)_4%,transparent)] p-8 text-center">
            <p className="font-mono text-[11px] tracking-widest uppercase text-[var(--muted)]">
              {failed
                ? "Feed no disponible — snapshot del servidor falló (R8 fail-honest)"
                : "0 asimetrías activas — searcher escaneando el mempool"}
            </p>
            <p className="mt-2 text-sm text-[var(--muted)]">
              {failed
                ? "El servidor no pudo obtener /api/opportunities/live. La UI no fabrica datos para ocultar el silencio operacional."
                : "El feed en vivo no devolvió oportunidades. Doctrina Zero-Mocks: se muestra el vacío real, no datos de demostración."}
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-[18px]">
            {opportunities.map((opp) => (
              <XRayCard key={opp.id} {...toXRayProps(opp)} />
            ))}
          </div>
        )}
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

        <GateSection decision={decisionRes.ok ? decisionRes.data : null} />
      </section>
    </div>
  );
}
