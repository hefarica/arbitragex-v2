/** Thin client for the edge. Only uses what the edge actually exposes. */

export type StatusResponse = {
  ok: boolean;
  services: Record<string, { ok: boolean; status?: number; detail?: string }>;
  killswitch: {
    enabled: boolean;
    reason: string | null;
    triggered_by: string | null;
    updated_at: string;
  } | null;
  env: string;
  version: string;
  ts: string;
};

const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL || "http://localhost:8787";

export async function getStatus(): Promise<
  { ok: true; data: StatusResponse } | { ok: false; error: string }
> {
  try {
    const r = await fetch(`${EDGE_URL}/status`, {
      next: { revalidate: 0 },
      headers: { accept: "application/json" },
    });
    if (!r.ok) return { ok: false, error: `edge returned HTTP ${r.status}` };
    const data = (await r.json()) as StatusResponse;
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}
