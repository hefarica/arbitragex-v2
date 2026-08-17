/**
 * Shared Topology Vault snapshot contract + ActiveChain mapping.
 *
 * Extracted from TopologyVaultClient.tsx (MC-CRED-2) so Server Components can
 * hydrate the RPC credential category from the server-side vault snapshot —
 * the page must not depend on a browser-local localStorage copy to decide
 * which rpc_http/rpc_ws rows exist (that made persisted validation state
 * invisible after reload when the local store was empty/wiped).
 *
 * No "use client" here on purpose: both the admin client and SSR pages import
 * from this module.
 */

import type { ActiveChain } from "@/store/useSystemStore";

export type MempoolMode = "auto" | "filtered" | "firehose";

export interface TopologyProviderSnapshot {
  name: string;
  url_masked: string;
  scheme: "http" | "https" | "ws" | "wss";
  host: string;
  provider_kind: "alchemy" | "standard";
}

export interface TopologySnapshot {
  scope: string;
  chain_id: number;
  mempool_mode: MempoolMode;
  rpc_http_1: TopologyProviderSnapshot[];
  rpc_ws_1: TopologyProviderSnapshot[];
  checksum: string;
  version_id: number;
  updated_at: string;
}

/** Envelope of GET /api/admin/topology/snapshot. */
export interface TopologySnapshotEnvelope {
  ok?: boolean;
  topology?: TopologySnapshot | null;
  source?: string;
  error?: string;
}

export function snapshotToActiveChains(
  topology: TopologySnapshot | null | undefined,
  fallbackVersion?: number,
): ActiveChain[] {
  if (!topology) return [];
  const versionId = fallbackVersion ?? topology.version_id;
  const validatedAt = topology.updated_at ?? new Date().toISOString();
  return [{
    chainId: topology.chain_id,
    name: `Chain ${topology.chain_id}`,
    rpcHttpHost: topology.rpc_http_1?.[0]?.host ?? "",
    rpcWsHost: topology.rpc_ws_1?.[0]?.host ?? "",
    versionId,
    validatedAt,
  }];
}
