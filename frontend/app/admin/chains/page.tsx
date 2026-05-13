/**
 * B1.b — /admin/chains Server Component.
 *
 * Mounted Snapshot Pattern (R1): the RSC attempts to fetch the chains list
 * from the edge proxy at SSR time so the first render is hydrated with real
 * data when available. The admin endpoint is V-AT-1 gated, so the SSR call
 * lacks a session cookie and will receive 401; the client then re-fetches
 * after mount with credentials: "include" (the operator's httpOnly cookie
 * automatically attaches in the browser).
 *
 * The page is `force-dynamic` because chains_runtime is mutable.
 */
import { PageHeader } from "@/components/page-header";
import { getApiBaseUrl } from "@/lib/api-client";
import { AdminChainsListSchema } from "@/lib/schemas";

import { ChainsAdminClient, type ChainsAdminSnapshot } from "./ChainsAdminClient";

export const metadata = { title: "Chains Admin | ArbitrageX" };
export const dynamic = "force-dynamic";
export const revalidate = 0;

async function getInitialSnapshot(): Promise<ChainsAdminSnapshot> {
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  try {
    const res = await fetch(`${EDGE_URL}/api/admin/chains`, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (res.status === 401 || res.status === 403) {
      return { rows: [], source: "server-auth-required", error: `HTTP ${res.status} — client will retry with admin cookie` };
    }
    if (res.status === 404) {
      return { rows: [], source: "endpoint-not-implemented", error: "admin chains endpoint not mounted on edge" };
    }
    if (!res.ok) {
      const body = await res.text().catch(() => "");
      return { rows: [], source: "server-fetch-failed", error: `HTTP ${res.status}: ${body.slice(0, 200)}` };
    }
    const raw = await res.json();
    const parsed = AdminChainsListSchema.safeParse(raw);
    if (!parsed.success) {
      return { rows: [], source: "server-fetch-failed", error: `edge response shape invalid` };
    }
    return { rows: parsed.data.items, source: "server-snapshot" };
  } catch (e) {
    return { rows: [], source: "server-fetch-failed", error: (e as Error).message };
  }
}

export default async function ChainsAdminPage() {
  const initialSnapshot = await getInitialSnapshot();
  return (
    <>
      <PageHeader
        title="Chains Admin"
        lede="Operator console for chains_runtime — register, edit, probe, and soft-disable chains. All mutations audited, hot-reload via Redis pub/sub."
        showRefresh
      />
      <ChainsAdminClient initialSnapshot={initialSnapshot} />
    </>
  );
}
