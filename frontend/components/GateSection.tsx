"use client";

interface GateCheck {
  name: string;
  status: "green" | "red";
}

interface GateSectionProps {
  title?: string;
  description?: string;
  verdict?: string;
  checks?: GateCheck[];
  lockMessage?: string;
}

const defaultChecks: GateCheck[] = [
  { name: "Flash-loan discipline", status: "green" },
  { name: "Simulation mandatory", status: "green" },
  { name: "Net-profit gate", status: "green" },
  { name: "RPC failover", status: "green" },
  { name: "Token-safety screen", status: "green" },
  { name: "KMS/HSM signer", status: "red" },
  { name: "Risk limits", status: "green" },
  { name: "MEV ethics", status: "green" },
  { name: "Pre-execute checklist", status: "green" },
  { name: "Crucible 72h/95%", status: "red" },
  { name: "Pre-edit audit", status: "green" },
  { name: "Contract atomicity", status: "red" },
];

export function GateSection({
  title = "Habilitación live · bloqueada por doctrina",
  description = "El flip a live requiere 12 gates doctrinales en verde + confirmación manual del operador. Mientras tanto, capital expuesto permanece estructuralmente en $0.00. La restricción al launch es lo que convierte escépticos en creyentes.",
  verdict = "VEREDICTO · NO-GO",
  checks = defaultChecks,
  lockMessage = "Activate live mode · LOCKED (3 gates red)",
}: GateSectionProps) {
  const redCount = checks.filter((c) => c.status === "red").length;

  return (
    <section className="gate-section">
      <div className="relative z-10">
        <h3 className="font-semibold text-xl mb-2 tracking-tight">{title}</h3>
        <p className="text-[var(--muted)] text-sm max-w-[52ch]">{description}</p>
      </div>

      <div className="relative z-10 font-mono">
        <div className="gate-verdict">{verdict}</div>

        <div className="gate-checks">
          {checks.map((check) => (
            <div key={check.name} className={`gate-check ${check.status}`}>
              <span className="led"></span>
              <span>{check.name}</span>
            </div>
          ))}
        </div>

        <div className="mt-4 font-mono text-[11px] tracking-widest uppercase px-4 py-3 rounded-lg bg-[color-mix(in_oklab,var(--foreground)_8%,transparent)] text-[var(--muted)] border border-[var(--border)] text-center cursor-not-allowed">
          {lockMessage.replace(/\d+ gates red/, `${redCount} gates red`)}
        </div>
      </div>
    </section>
  );
}
