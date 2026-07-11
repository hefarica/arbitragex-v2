/**
 * /paper/history — Paper Trade Runs Drift-Analysis Surface (R1 Mounted-Snapshot).
 *
 * Surfaces the paper_trade_runs table written by the PaperTradeArchiver.
 * Pure Server Component: fetches initial snapshot at SSR time, passes to
 * PaperHistoryClient for live polling every 15s.
 *
 * Fail-honest: failed server fetch yields empty snapshot — never fabricated data.
 * Safety: observe-only. No capital, no execution, no broadcast.
 * Ghost Protocol: capital_exposure_usd = 0.0000000000 (never displayed here).
 */
import { FlaskConicalIcon } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { getApiBaseUrl } from "@/lib/api-client";
import { PaperHistoryClient } from "@/features/paper-history/PaperHistoryClient";

export const metadata = {
  title: "Paper History — ArbitrageX",
};

export const dynamic = "force-dynamic";
export const revalidate = 0;

async function fetchJson<T>(edgeUrl: string, path: string): Promise<{ data: T | null; error: string | null }> {
  try {
    const res = await fetch(`${edgeUrl}${path}`, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!res.ok) return { data: null, error: `HTTP ${res.status}` };
    return { data: (await res.json()) as T, error: null };
  } catch (e) {
    return { data: null, error: (e as Error).message };
  }
}

export default async function PaperHistoryPage() {
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();

  const [history, summary] = await Promise.all([
    fetchJson(EDGE_URL, "/api/paper/history?limit=50"),
    fetchJson(EDGE_URL, "/api/paper/history/summary?hours=24"),
  ]);

  // Fail-honest: thread an SSR fetch-failure sentinel so the client shows a
  // degraded banner on first paint instead of the healthy "no runs yet" empty.
  const initialError = history.error
    ? `history ${history.error}`
    : summary.error
    ? `summary ${summary.error}`
    : null;

  const initialData = { history: history.data, summary: summary.data, initialError };

  return (
    <>
      <PageHeader
        title="Paper Trade History"
        lede="Drift-analysis surface over paper_trade_runs — sim predictions from the shadow archiver. Observe-only."
        showRefresh
      />
      <PaperHistoryClient initialData={initialData as Parameters<typeof PaperHistoryClient>[0]["initialData"]} />
    </>
  );
}
