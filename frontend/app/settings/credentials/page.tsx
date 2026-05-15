/**
 * /settings/credentials — operator's single surface for every external
 * credential the platform needs to function.
 *
 * R1 Mounted Snapshot: this page is a Server Component that fetches the
 * masked credential list once at SSR time and hands it to the Client
 * component for interactive editing.
 *
 * OMEGA-8 / M5 Capa 4 Fase 3 (P0-SC-1 / P1-SC-2):
 *   - Edge URL resolved from `INTERNAL_EDGE_URL` (Docker DNS) at SSR, never
 *     `NEXT_PUBLIC_EDGE_URL` (which would leak the browser-facing URL into
 *     a server-side fetch and break R2 in production).
 *   - Response is parsed with `CredentialsResponseSchema`. The page renders
 *     a discriminated union so a caída is NOT silently converted to `items: []`.
 */

import type { Metadata } from "next";
import { CredentialsClient, type CredentialsSnapshot } from "./CredentialsClient";
import { getValidated } from "@/lib/frontier";
import { CredentialsResponseSchema } from "@/lib/schemas";
import { getApiBaseUrl } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export const metadata: Metadata = {
  title: "Credentials — QuantumX",
  description: "Operator credentials store — RPC, CEX, MEV, prices. Live status per credential.",
};

async function fetchInitial(): Promise<CredentialsSnapshot> {
  const result = await getValidated("/api/credentials", CredentialsResponseSchema);
  const now = new Date().toISOString();
  switch (result.kind) {
    case "ok":
      return {
        items: result.data.items as CredentialsSnapshot["items"],
        ts: result.data.ts,
        error: null,
      };
    case "auth_required":
      return { items: [], ts: now, error: `AUTH_REQUIRED (HTTP ${result.status})` };
    case "unavailable":
      return { items: [], ts: now, error: `UNAVAILABLE: ${result.detail}` };
    case "endpoint_not_implemented":
      return { items: [], ts: now, error: `ENDPOINT_NOT_IMPLEMENTED: ${result.detail}` };
    case "invalid_response":
      return { items: [], ts: now, error: `INVALID_RESPONSE: ${result.detail}` };
  }
}

export default async function CredentialsPage() {
  const initial = await fetchInitial();
  // The client component still needs an edge URL for mutations — pass the
  // browser-facing URL so mutations from the operator's tab reach the edge
  // via the proper public hostname (not the Docker DNS internal name).
  const browserEdgeUrl = process.env["NEXT_PUBLIC_EDGE_URL"] ?? getApiBaseUrl();
  return <CredentialsClient initialSnapshot={initial} edgeUrl={browserEdgeUrl} />;
}
