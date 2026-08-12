import type { ReadinessDecisionResponse } from "@/lib/schemas";

// Server Component (no "use client"). Rebinds the doctrinal gate visual to the
// real /api/readiness/decision verdict instead of a fabricated 12-gate array
// (RULE 00 — Zero Mocks). On fetch failure the parent passes decision=null and
// we render an R8 fail-honest block; no invented gates, no invented counts.

interface GateSectionProps {
  title?: string;
  description?: string;
  decision: ReadinessDecisionResponse | null; // null = fetch failed
}

const MAX_LED_ROWS = 6;

export function GateSection({
  title = "Habilitación live · bloqueada por doctrina",
  description = "El flip a live requiere los gates doctrinales en verde + confirmación manual del operador. Mientras tanto, capital expuesto permanece estructuralmente en $0.00. La restricción al launch es lo que convierte escépticos en creyentes.",
  decision,
}: GateSectionProps) {
  return (
    <section className="gate-section">
      <div className="relative z-10">
        <h3 className="font-semibold text-xl mb-2 tracking-tight">{title}</h3>
        <p className="text-[var(--muted)] text-sm max-w-[52ch]">{description}</p>
      </div>

      {decision == null ? (
        // R8 fail-honest: backend unavailable. No verdict, no LED wall, no LOCK pill.
        <div className="relative z-10 font-mono">
          <div className="gate-verdict">backend no disponible</div>
          <div className="gate-checks">
            <div className="gate-check red">
              <span className="led"></span>
              <span>
                Backend no disponible — no se pudo obtener /api/readiness/decision.
                R8 fail-honest: no se fabrican gates.
              </span>
            </div>
          </div>
        </div>
      ) : (
        <div className="relative z-10 font-mono">
          <div className="gate-verdict">
            {decision.verdict === "NO_GO" ? "VEREDICTO · NO-GO" : "VEREDICTO · GO"}
          </div>

          <div className="gate-checks">
            {decision.reasons.length === 0 ? (
              <div className="gate-check green">
                <span className="led"></span>
                <span>All gates clear</span>
              </div>
            ) : (
              <>
                {decision.reasons.slice(0, MAX_LED_ROWS).map((reason) => (
                  <div key={reason} className="gate-check red">
                    <span className="led"></span>
                    <span>{reason}</span>
                  </div>
                ))}
                {decision.reasons.length > MAX_LED_ROWS && (
                  <div className="gate-check red">
                    <span className="led"></span>
                    <span>
                      +{decision.reasons.length - MAX_LED_ROWS} more — ver /readiness
                    </span>
                  </div>
                )}
              </>
            )}
          </div>

          <div className="mt-4 font-mono text-[11px] tracking-widest uppercase px-4 py-3 rounded-lg bg-[color-mix(in_oklab,var(--foreground)_8%,transparent)] text-[var(--muted)] border border-[var(--border)] text-center cursor-not-allowed">
            {decision.verdict === "NO_GO"
              ? `Activate live mode · LOCKED (${decision.reasons.length} gates red)`
              : "Ready for operator confirmation"}
          </div>

          <div className="mt-3 font-mono text-[11px] tracking-widest uppercase text-[var(--muted)] text-center">
            {decision.phase} · capital ${decision.capital_exposure_usd.toFixed(2)} · paper{" "}
            {decision.paper_mode ? "ON" : "OFF"}
          </div>

          <div className="mt-3 text-center">
            <a
              href="/readiness"
              className="font-mono text-[11px] tracking-widest uppercase text-[var(--primary)] hover:underline"
            >
              Full breakdown →
            </a>
          </div>
        </div>
      )}
    </section>
  );
}
