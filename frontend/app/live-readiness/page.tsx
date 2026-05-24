"use client";

import { useEffect, useMemo, useState } from "react";
import { RefreshCwIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { PageHeader } from "@/components/page-header";
import { getReadiness } from "@/lib/api-client";
import type {
  ReadinessGroup,
  ReadinessItem,
  ReadinessReport,
  ReadinessStatus,
} from "@/lib/schemas";
import { BlockersPanel } from "@/features/readiness/BlockersPanel";
import { GoNoGoPanel } from "@/features/readiness/GoNoGoPanel";
import { AgentTeamsPanel } from "@/features/readiness/AgentTeamsPanel";
import { ConfidenceScoringPanel } from "@/features/readiness/ConfidenceScoringPanel";
import { RiskCircuitPanel } from "@/features/risk/RiskCircuitPanel";
import { LiveReadinessStepper } from "@/components/ReadinessStepper";

// ─── Aesthetic helpers ───────────────────────────────────────────────────

const STATUS_DOT_CLASS: Record<ReadinessStatus, string> = {
  green: "bg-[oklch(75%_0.18_145)]",
  yellow: "bg-[oklch(78%_0.15_80)]",
  red: "bg-[oklch(65%_0.22_25)]",
  pending: "bg-[oklch(50%_0_0)]",
};

const STATUS_LABEL: Record<ReadinessStatus, string> = {
  green: "verified",
  yellow: "partial",
  red: "failing",
  pending: "not started",
};

const GROUP_ORDER: ReadinessGroup[] = [
  "security_compliance",
  "audit_trail",
  "risk_doctrines",
  "tokens_strategies",
  "contracts",
  "operations",
];

const GROUP_LABEL: Record<ReadinessGroup, string> = {
  security_compliance: "Security & Compliance",
  audit_trail: "Audit Trail",
  risk_doctrines: "Risk Doctrines",
  tokens_strategies: "Tokens & Strategies",
  contracts: "Smart Contracts",
  operations: "Operations",
};

// Dot-matrix progress bar (e.g. "█████░░░░░░░░░░░░"); 17 cells default.
function progressBar(green: number, total: number, width = 17): string {
  const filled = Math.round((green / total) * width);
  return "█".repeat(filled) + "░".repeat(Math.max(0, width - filled));
}

// ─── Components ──────────────────────────────────────────────────────────

function StatusDot({ status }: { status: ReadinessStatus }) {
  return (
    <span
      aria-label={STATUS_LABEL[status]}
      className={`inline-block h-2 w-2 shrink-0 rounded-full ${STATUS_DOT_CLASS[status]}`}
    />
  );
}

function ReadinessRow({ item }: { item: ReadinessItem }) {
  return (
    <div className="grid grid-cols-[auto_8rem_1fr_auto] items-start gap-x-4 border-b border-border/40 py-2.5 last:border-b-0">
      <div className="pt-1.5">
        <StatusDot status={item.status} />
      </div>
      <code className="font-mono text-xs uppercase tracking-wider text-muted-foreground tabular-nums">
        {item.id}
      </code>
      <div className="min-w-0">
        <div className="text-sm text-foreground">{item.label}</div>
        <div className="mt-0.5 text-xs text-muted-foreground/80">{item.reason}</div>
        {item.evidence && (
          <div className="mt-1 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            {item.evidence.kind} · {item.evidence.ref}
          </div>
        )}
      </div>
      <div className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/70">
        {STATUS_LABEL[item.status]}
      </div>
    </div>
  );
}

function GroupSection({
  group,
  items,
}: {
  group: ReadinessGroup;
  items: ReadinessItem[];
}) {
  if (items.length === 0) return null;
  const greens = items.filter((i) => i.status === "green").length;
  return (
    <section className="border-t border-border/40 first:border-t-0">
      <header className="flex items-baseline justify-between px-1 pb-2 pt-6">
        <h2 className="font-mono text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground">
          {GROUP_LABEL[group]}
        </h2>
        <span className="font-mono text-[10px] tabular-nums text-muted-foreground/60">
          {greens}/{items.length}
        </span>
      </header>
      <div className="px-1">
        {items.map((it) => (
          <ReadinessRow key={it.id} item={it} />
        ))}
      </div>
    </section>
  );
}

function StatusBanner({ report }: { report: ReadinessReport }) {
  const { summary, flip_blocked } = report;
  const bar = progressBar(summary.green, summary.total);
  const verified_at = new Date(report.generated_at).toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

  return (
    <div className="sticky top-0 z-10 -mx-4 mb-6 border-b border-border/60 bg-background/95 px-4 py-4 backdrop-blur supports-[backdrop-filter]:bg-background/80 sm:-mx-6 sm:px-6">
      <div className="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-2">
        <div className="flex items-baseline gap-3">
          <span className="font-mono text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
            paper_mode
          </span>
          <span
            className={`font-mono text-[11px] font-medium uppercase tracking-wider ${
              flip_blocked
                ? "text-[oklch(78%_0.15_80)]"
                : "text-[oklch(75%_0.18_145)]"
            }`}
          >
            {flip_blocked ? "flip blocked" : "ready to flip"}
          </span>
        </div>
        <div className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60 tabular-nums">
          verified {verified_at}
        </div>
      </div>

      <div className="mt-3 flex items-baseline gap-4">
        <div className="font-mono text-2xl font-semibold tabular-nums text-foreground">
          {summary.green}
          <span className="text-muted-foreground/40">/</span>
          {summary.total}
        </div>
        <code className="font-mono text-xs tracking-tighter text-muted-foreground">
          {bar}
        </code>
      </div>

      <div className="mt-2 flex flex-wrap gap-x-5 gap-y-1 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/80 tabular-nums">
        <span>
          <StatusDot status="green" />
          <span className="ml-1.5">{summary.green} verified</span>
        </span>
        {summary.yellow > 0 && (
          <span>
            <StatusDot status="yellow" />
            <span className="ml-1.5">{summary.yellow} partial</span>
          </span>
        )}
        {summary.red > 0 && (
          <span>
            <StatusDot status="red" />
            <span className="ml-1.5">{summary.red} failing</span>
          </span>
        )}
        <span>
          <StatusDot status="pending" />
          <span className="ml-1.5">{summary.pending} not started</span>
        </span>
      </div>
    </div>
  );
}

function FlipToggle({ report }: { report: ReadinessReport }) {
  const blocked = report.flip_blocked;
  const missing = report.items.filter((i) => i.status !== "green").map((i) => i.id);
  const tooltip = blocked
    ? `Disabled: ${missing.length} item(s) not green — ${missing.slice(0, 5).join(", ")}${missing.length > 5 ? `, +${missing.length - 5} more` : ""}`
    : "All items verified — confirm flip to live execution";

  return (
    <div className="mt-10 border-t border-border/40 pt-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div className="max-w-md">
          <h3 className="text-sm font-medium text-foreground">
            Activate live mode
          </h3>
          <p className="mt-1 text-xs text-muted-foreground">
            Irreversible without 2-firma override + capital cap reset. This
            button is disabled until all 17 readiness items report{" "}
            <code className="font-mono text-[11px] text-foreground/80">verified</code>
            . The honesty doctrine forbids overrides while items are red,
            yellow, or pending.
          </p>
        </div>
        <Button
          variant={blocked ? "outline" : "default"}
          disabled={blocked}
          aria-disabled={blocked}
          title={tooltip}
          className="font-mono text-xs uppercase tracking-widest disabled:opacity-30 disabled:cursor-not-allowed"
        >
          {blocked
            ? `flip blocked — ${missing.length} pending`
            : "activate live mode"}
        </Button>
      </div>
    </div>
  );
}

// ─── Page ────────────────────────────────────────────────────────────────

export default function LiveReadinessPage() {
  const [report, setReport] = useState<ReadinessReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    setLoading(true);
    const r = await getReadiness();
    setLoading(false);
    if (!r.ok) {
      setError(r.error);
      return;
    }
    setError(null);
    setReport(r.data);
  }

  useEffect(() => {
    void refresh();
    const t = setInterval(() => void refresh(), 30_000);
    return () => clearInterval(t);
  }, []);

  const grouped = useMemo(() => {
    if (!report) return null;
    const by: Partial<Record<ReadinessGroup, ReadinessItem[]>> = {};
    for (const it of report.items) {
      (by[it.group] ??= []).push(it);
    }
    return by;
  }, [report]);

  return (
    <>
      <PageHeader
        title="Live readiness"
        lede="Doctrinal gate between paper_mode and any future flip to live capital. Each item is verified dynamically by the api-server — no false greens."
        actions={
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void refresh()}
            disabled={loading}
            className="font-mono text-xs uppercase tracking-widest"
          >
            <RefreshCwIcon className={`mr-2 size-3.5 ${loading ? "animate-spin" : ""}`} />
            refresh
          </Button>
        }
      />

      <LiveReadinessStepper className="mb-6" />

      {error && (
        <div className="mb-6 rounded-md border border-[oklch(65%_0.22_25)]/40 bg-[oklch(65%_0.22_25)]/5 px-4 py-3">
          <div className="font-mono text-[10px] uppercase tracking-widest text-[oklch(65%_0.22_25)]">
            verifier error
          </div>
          <div className="mt-1 font-mono text-xs text-foreground/80">
            {error}
          </div>
          <div className="mt-2 text-xs text-muted-foreground">
            The page never synthesizes data — this is the verbatim error from
            the verifier. Fix the underlying issue and click refresh.
          </div>
        </div>
      )}

      {!report && !error && (
        <div className="space-y-4 py-12 text-center font-mono text-xs uppercase tracking-widest text-muted-foreground/60">
          loading verifiers…
        </div>
      )}

      {report && grouped && (
        <>
          <StatusBanner report={report} />
          <div className="space-y-0">
            {GROUP_ORDER.map((g) => (
              <GroupSection key={g} group={g} items={grouped[g] ?? []} />
            ))}
          </div>
          <FlipToggle report={report} />
        </>
      )}

      {/* P2 (2026-05-13): derived blockers + GO/NO-GO decision panels.
          Rendered AFTER the existing 17-item verifier list and FlipToggle,
          additively. Failures in these panels do not affect the existing UI.

          P2-continued (2026-05-13): AgentTeamsPanel — 17 agent verdicts. */}
      <div className="mt-10 space-y-6 border-t border-border/40 pt-8">
        <GoNoGoPanel />
        <BlockersPanel />
        <AgentTeamsPanel />
        <ConfidenceScoringPanel />
        <RiskCircuitPanel />
      </div>
    </>
  );
}
