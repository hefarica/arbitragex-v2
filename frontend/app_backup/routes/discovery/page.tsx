/**
 * /routes/discovery — Route Discovery Radar (R1 Mounted-Snapshot).
 *
 * Surfaces the DFS-bounded route-discovery graph from the searcher-rs worker.
 * Pure Server Component: fetches initial snapshot from edge at SSR time, hands
 * to RoutesDiscoveryClient for live polling every 8s.
 *
 * Fail-honest: failed server fetch yields empty snapshot — never fabricated data.
 * Safety: observe-only. No capital, no execution, no broadcast.
 */
import { RadarIcon } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { getApiBaseUrl } from "@/lib/api-client";
import { RoutesDiscoveryClient } from "@/features/routes-discovery/RoutesDiscoveryClient";

export const metadata = {
  title: "Routes Discovery — ArbitrageX",
};

export const dynamic = "force-dynamic";
export const revalidate = 0;

async function fetchJson<T>(edgeUrl: string, path: string): Promise<T | null> {
  try {
    const res = await fetch(`${edgeUrl}${path}`, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!res.ok) return null;
    return (await res.json()) as T;
  } catch {
    return null;
  }
}

export default async function RoutesDiscoveryPage() {
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();

  const [routesRaw, statusRaw] = await Promise.all([
    fetchJson<{ ok: boolean; data: { routes: unknown[] } }>(EDGE_URL, "/api/route-discovery/routes"),
    fetchJson<{ ok: boolean; mode: string; updated_at: string; data: unknown }>(EDGE_URL, "/api/route-discovery/status"),
  ]);

  const initialData = {
    routes: routesRaw?.data ?? null,
    status: statusRaw,
  };

  return (
    <>
      <PageHeader
        title="Routes Discovery"
        lede="DFS-bounded route-discovery graph from the searcher-rs worker. Observe-only — shadow mode."
        showRefresh
      />
      <RoutesDiscoveryClient initialData={initialData as Parameters<typeof RoutesDiscoveryClient>[0]["initialData"]} />
    </>
  );
}
