"use client";
/**
 * AgentInsightsClient — live-polling view of the 17 ArbitrageX agent-team verdicts.
 *
 * R1 Mounted-Snapshot Pattern: receives initialData from the Server Component,
 * then polls /api/agents/status every 30s. Fail-honest: poll errors surface
 * an inline banner; the last good snapshot is preserved.
 *
 * Safety: observe-only. No capital, no execution, no broadcast.
 * Zero-Mocks: all data from /api/agents/status; no fabricated defaults.
 */
import { useEffect, useState } from "react";
import { AlertCircleIcon, BotIcon, CheckCircle2Icon, XCircleIcon, AlertTriangleIcon, ClockIcon, HelpCircleIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { LocalTime } from "@/components/LocalTime";

const POLL_INTERVAL_MS = 30_000;

// ─── Types (mirror backend wire contract) ─────────────────────────────────────
type AgentVerdict = "PASS" | "BLOCKED" | "PARTIAL" | "NO_GO" | "NOT_RUN" | "UNKNOWN";
type AgentStatus = "healthy" | "degraded" | "blocked" | "unknown";

interface AgentRow {
  id: string;
  name: string;
  category: string;
  verdict: AgentVerdict;
  status: AgentStatus;
  evidence: string[];
  last_run_at: string | null;
  source: string;
  blocks: string[];
  next_action: string | null;
  risk: string;
  operator_required: boolean;
}

interface AgentsStatusResponse {
  generated_at: string;
  source: string;
  overall_status: string;
  agents: AgentRow[];
}

// ─── Verdict badge ─────────────────────────────────────────────────────────────
function VerdictBadge({ verdict }: { verdict: AgentVerdict }) {
  const map: Record<AgentVerdict, { label: string; variant: "default" | "secondary" | "destructive" | "outline" }> = {
    PASS:    { label: "PASS",    variant: "default" },
    BLOCKED: { label: "BLOCKED", variant: "destructive" },
    PARTIAL: { label: "PARTIAL", variant: "secondary" },
    NO_GO:   { label: "NO_GO",   variant: "destructive" },
    NOT_RUN: { label: "NOT_RUN", variant: "outline" },
    UNKNOWN: { label: "UNKNOWN", variant: "outline" },
  };
  const { label, variant } = map[verdict] ?? { label: verdict, variant: "outline" };
  return <Badge variant={variant}>{label}</Badge>;
}

// ─── Status icon ───────────────────────────────────────────────────────────────
function StatusIcon({ status }: { status: AgentStatus }) {
  if (status === "healthy")  return <CheckCircle2Icon   className="h-4 w-4 text-green-500" />;
  if (status === "degraded") return <AlertTriangleIcon  className="h-4 w-4 text-yellow-500" />;
  if (status === "blocked")  return <XCircleIcon        className="h-4 w-4 text-destructive" />;
  return                            <HelpCircleIcon     className="h-4 w-4 text-muted-foreground" />;
}

// ─── Agent card ────────────────────────────────────────────────────────────────
function AgentCard({ agent }: { agent: AgentRow }) {
  return (
    <Card data-testid={`agent-card-${agent.id}`}>
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-2">
          <CardTitle className="text-sm font-semibold leading-tight">
            <span className="flex items-center gap-2">
              <StatusIcon status={agent.status} />
              {agent.name}
            </span>
          </CardTitle>
          <VerdictBadge verdict={agent.verdict} />
        </div>
        <p className="text-xs text-muted-foreground capitalize">{agent.category}</p>
      </CardHeader>
      <CardContent className="space-y-2">
        {agent.evidence.length > 0 && (
          <ul className="space-y-1">
            {agent.evidence.map((ev, i) => (
              <li key={i} className="text-xs text-muted-foreground flex items-start gap-1">
                <span className="mt-0.5 shrink-0 text-muted-foreground/50">·</span>
                {ev}
              </li>
            ))}
          </ul>
        )}
        {agent.next_action && (
          <div className="rounded bg-muted/40 px-2 py-1 text-xs">
            <span className="font-medium">Next: </span>{agent.next_action}
          </div>
        )}
        {agent.blocks.length > 0 && (
          <div className="text-xs text-destructive">
            Blocked by: {agent.blocks.join(", ")}
          </div>
        )}
        {agent.operator_required && (
          <Badge variant="outline" className="text-xs">Operator action required</Badge>
        )}
      </CardContent>
    </Card>
  );
}

// ─── Main client ───────────────────────────────────────────────────────────────
interface Props {
  initialData: unknown;
}

export function AgentInsightsClient({ initialData }: Props) {
  const [data, setData] = useState<AgentsStatusResponse | null>(
    initialData as AgentsStatusResponse | null
  );
  const [pollError, setPollError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<number>(0);

  useEffect(() => {
    setLastUpdated(Date.now());
    let alive = true;

    const poll = async () => {
      try {
        const res = await fetch("/api/agents/status", {
          cache: "no-store",
          headers: { accept: "application/json" },
        });
        if (!alive) return;
        if (!res.ok) {
          setPollError(`HTTP ${res.status}`);
          return;
        }
        const json = await res.json() as AgentsStatusResponse;
        setData(json);
        setPollError(null);
        setLastUpdated(Date.now());
      } catch (e) {
        if (alive) setPollError((e as Error).message);
      }
    };

    const timer = setInterval(() => void poll(), POLL_INTERVAL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);

  if (!data) {
    return (
      <Alert variant="destructive" data-testid="agent-insights-error">
        <AlertCircleIcon />
        <AlertTitle>Agent status unavailable</AlertTitle>
        <AlertDescription>
          The agent-teams status could not be loaded. The edge or api-server may be temporarily unavailable.
        </AlertDescription>
      </Alert>
    );
  }

  // Group agents by category
  const byCategory = data.agents.reduce<Record<string, AgentRow[]>>((acc, a) => {
    (acc[a.category] ??= []).push(a);
    return acc;
  }, {});

  const overallColor =
    data.overall_status === "healthy" ? "text-green-500" :
    data.overall_status === "blocked" ? "text-destructive" :
    "text-yellow-500";

  return (
    <div className="space-y-6" data-testid="agent-insights-panel">
      {pollError && (
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>Poll error</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{pollError}</code></AlertDescription>
        </Alert>
      )}

      {/* Summary strip */}
      <div className="flex flex-wrap items-center gap-4 rounded-lg border p-4" data-testid="agent-insights-summary">
        <div className="flex items-center gap-2">
          <BotIcon className="h-5 w-5 text-primary" />
          <span className="text-sm font-medium">Overall:</span>
          <span className={`text-sm font-semibold capitalize ${overallColor}`}>{data.overall_status}</span>
        </div>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <ClockIcon className="h-4 w-4" />
          <LocalTime iso={data.generated_at} />
        </div>
        <Badge variant="outline" className="text-xs">source: {data.source}</Badge>
        <Badge variant="outline" className="text-xs">
          {data.agents.filter(a => a.verdict === "PASS").length}/{data.agents.length} PASS
        </Badge>
      </div>

      {/* Agent cards grouped by category */}
      {Object.entries(byCategory).map(([category, agents]) => (
        <section key={category} data-testid={`agent-category-${category}`}>
          <h2 className="mb-3 text-sm font-semibold capitalize text-muted-foreground">{category}</h2>
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {agents.map(agent => (
              <AgentCard key={agent.id} agent={agent} />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
