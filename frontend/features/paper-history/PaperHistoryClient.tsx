"use client";
/**
 * PaperHistoryClient — live-polling view of paper_trade_runs drift-analysis data.
 *
 * R1 Mounted-Snapshot Pattern: receives initialData from the Server Component,
 * then polls /api/paper/history + /api/paper/history/summary every 15s.
 * Fail-honest: poll errors surface an inline banner; last good snapshot preserved.
 *
 * Safety: observe-only. No capital, no execution, no broadcast.
 * Ghost Protocol: capital_exposure_usd = 0.0000000000 (never displayed here).
 * Zero-Mocks: all data from /api/paper/history; no fabricated defaults.
 */
import { useEffect, useState } from "react";
import { AlertCircleIcon, FlaskConicalIcon, TrendingUpIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { fmtMoney, fmtTime } from "@/lib/formatters";

const POLL_INTERVAL_MS = 15_000;

// ─── Types ────────────────────────────────────────────────────────────────────
interface PaperTradeRow {
  id: string;
  opportunity_id: string | null;
  route_hash: string | null;
  sim_expected_profit_usd: string | null;
  sim_gas_cost_usd: string | null;
  sim_net_profit_usd: string | null;
  strategy: string | null;
  chain_id: number | null;
  created_at: string;
}

interface PaperHistoryResponse {
  ok: boolean;
  source: string;
  count: number;
  limit: number;
  offset: number;
  data: PaperTradeRow[];
}

interface SummaryTotals {
  total: number;
  profitable: number;
  avg_expected_profit_usd: string | null;
  avg_net_profit_usd: string | null;
  total_gas_cost_usd: string | null;
  strategies: number;
  chains: number;
}

interface PaperSummaryResponse {
  ok: boolean;
  source: string;
  window_hours: number;
  data: { totals: SummaryTotals };
}

interface PaperSnapshot {
  history: PaperHistoryResponse | null;
  summary: PaperSummaryResponse | null;
}

// ─── Helpers ──────────────────────────────────────────────────────────────────
function fmtProfit(val: string | null): string {
  if (val === null || val === undefined) return "—";
  const n = parseFloat(val);
  if (!Number.isFinite(n)) return "—";
  return fmtMoney(n);
}

// ─── Summary strip ────────────────────────────────────────────────────────────
function SummaryStrip({ summary }: { summary: PaperSummaryResponse }) {
  const t = summary.data.totals;
  const hitRate = t.total > 0 ? ((Number(t.profitable) / Number(t.total)) * 100).toFixed(1) : "—";
  return (
    <div className="flex flex-wrap gap-4 rounded-lg border p-4" data-testid="paper-history-summary">
      <div className="flex items-center gap-2">
        <FlaskConicalIcon className="h-4 w-4 text-primary" />
        <span className="text-sm font-medium">Runs:</span>
        <span className="text-sm font-semibold">{Number(t.total).toLocaleString()}</span>
      </div>
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <TrendingUpIcon className="h-4 w-4" />
        <span>Hit rate: <strong>{hitRate}%</strong></span>
      </div>
      <div className="text-sm text-muted-foreground">
        Avg net: <strong>{fmtProfit(t.avg_net_profit_usd)}</strong>
      </div>
      <div className="text-sm text-muted-foreground">
        Gas total: <strong>{fmtProfit(t.total_gas_cost_usd)}</strong>
      </div>
      <Badge variant="outline" className="text-xs">window: {summary.window_hours}h</Badge>
      <Badge variant="outline" className="text-xs">source: {summary.source}</Badge>
    </div>
  );
}

// ─── Row ──────────────────────────────────────────────────────────────────────
function PaperTradeRowCard({ row }: { row: PaperTradeRow }) {
  const netProfit = row.sim_net_profit_usd ? parseFloat(row.sim_net_profit_usd) : null;
  const isProfit = netProfit !== null && netProfit > 0;
  return (
    <tr className="border-b text-sm last:border-0">
      <td className="py-2 pr-3 font-mono text-xs text-muted-foreground">
        {row.created_at ? fmtTime(row.created_at) : "—"}
      </td>
      <td className="py-2 pr-3">
        <Badge variant="outline" className="text-xs capitalize">{row.strategy ?? "—"}</Badge>
      </td>
      <td className="py-2 pr-3 text-xs text-muted-foreground">
        {row.route_hash ? row.route_hash.slice(0, 10) + "…" : "—"}
      </td>
      <td className="py-2 pr-3 text-right font-mono text-xs">
        {fmtProfit(row.sim_expected_profit_usd)}
      </td>
      <td className="py-2 pr-3 text-right font-mono text-xs text-muted-foreground">
        {fmtProfit(row.sim_gas_cost_usd)}
      </td>
      <td className={`py-2 text-right font-mono text-xs font-semibold ${isProfit ? "text-green-500" : "text-destructive"}`}>
        {fmtProfit(row.sim_net_profit_usd)}
      </td>
    </tr>
  );
}

// ─── Main client ───────────────────────────────────────────────────────────────
interface Props {
  initialData: PaperSnapshot;
}

export function PaperHistoryClient({ initialData }: Props) {
  const [data, setData] = useState<PaperSnapshot>(initialData);
  const [pollError, setPollError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;

    const poll = async () => {
      try {
        const [histRes, sumRes] = await Promise.all([
          fetch("/api/paper/history?limit=50", { cache: "no-store", headers: { accept: "application/json" } }),
          fetch("/api/paper/history/summary?hours=24", { cache: "no-store", headers: { accept: "application/json" } }),
        ]);
        if (!alive) return;
        const history = histRes.ok ? await histRes.json() : null;
        const summary = sumRes.ok ? await sumRes.json() : null;
        setData({ history, summary });
        setPollError(null);
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

  const rows = data.history?.data ?? [];

  return (
    <div className="space-y-6" data-testid="paper-history-panel">
      {pollError && (
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>Poll error</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{pollError}</code></AlertDescription>
        </Alert>
      )}

      {data.summary && <SummaryStrip summary={data.summary} />}

      {rows.length === 0 ? (
        <div className="rounded-lg border p-8 text-center text-sm text-muted-foreground" data-testid="paper-history-empty">
          No paper trade runs recorded yet. Set <code className="font-mono text-xs">ARBX_PAPER_ARCHIVER_MODE=on</code> to enable the archiver.
        </div>
      ) : (
        <div className="overflow-x-auto rounded-lg border" data-testid="paper-history-table">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b bg-muted/40 text-xs text-muted-foreground">
                <th className="py-2 pr-3 text-left font-medium">Time</th>
                <th className="py-2 pr-3 text-left font-medium">Strategy</th>
                <th className="py-2 pr-3 text-left font-medium">Route</th>
                <th className="py-2 pr-3 text-right font-medium">Sim Profit</th>
                <th className="py-2 pr-3 text-right font-medium">Gas Cost</th>
                <th className="py-2 text-right font-medium">Net Profit</th>
              </tr>
            </thead>
            <tbody>
              {rows.map(row => (
                <PaperTradeRowCard key={row.id} row={row} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
