/**
 * /operator/self-test — Operator Self-Test Center (R1 Mounted-Snapshot).
 *
 * Pure Server Component: fetches the initial snapshot from the edge
 * (selftest checklist + presence-only credential matrix) and hands it to the
 * client as a serializable prop. All interactivity / non-determinism lives in
 * SelfTestClient. Fail-honest: a failed server fetch yields a null snapshot
 * and the client renders the explicit unavailable state — never fabricated data.
 */

import SelfTestClient, {
  type CredentialsStatusResponse,
  type SelfTestResponse,
  type SelfTestSnapshot,
} from "./SelfTestClient";
import { getApiBaseUrl } from "@/lib/api-client";

export const dynamic = "force-dynamic";
export const revalidate = 0;

async function fetchInitial<T>(edgeUrl: string, path: string): Promise<T | null> {
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

async function getInitialSnapshot(): Promise<SelfTestSnapshot> {
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  const [selftest, credentials] = await Promise.all([
    fetchInitial<SelfTestResponse>(EDGE_URL, "/api/operator/selftest"),
    fetchInitial<CredentialsStatusResponse>(EDGE_URL, "/api/operator/credentials/status"),
  ]);
  return {
    selftest,
    credentials,
    fetchedAt: selftest || credentials ? new Date().toISOString() : null,
    source: selftest || credentials ? "server-snapshot" : "server-fetch-failed",
  };
}

export default async function OperatorSelfTestPage() {
  const initialSnapshot = await getInitialSnapshot();
  return <SelfTestClient initialSnapshot={initialSnapshot} />;
}
