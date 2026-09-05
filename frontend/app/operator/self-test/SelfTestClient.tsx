"use client";

/**
 * SelfTestClient — Operator Self-Test Center (R1 Mounted-Snapshot client).
 *
 * Receives the server-fetched initial snapshot as a prop (page.tsx is the pure
 * Server Component shell) and re-fetches on operator demand. Everything
 * non-deterministic (refetch timestamps, client clock) lives in event handlers
 * / useEffect only; suppressHydrationWarning is applied ONLY to individual
 * time <span>s, never to containers.
 *
 * HONESTY (RULE 00): renders exactly what the API returns. Empty/failed
 * snapshot → honest empty/error states. The credentials table is PRESENCE-ONLY
 * by design — values never leave the server.
 */

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { ArrowUpRightIcon, RefreshCwIcon, ShieldCheckIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PageHeader } from "@/components/page-header";
import { getApiBaseUrl } from "@/lib/api-client";

// ─── Wire contracts (mirror backend/api-server routes/operator-selftest.ts) ──

export type SelfTestStatus =
  | "VERIFIED"
  | "MISSING"
  | "PARTIAL"
  | "FAILING"
  | "HONEST_EMPTY"
  | "DISABLED_BY_POLICY"
  | "BLOCKED_REQUIRES_OPERATOR_SECRET"
  | "BLOCKED_REQUIRES_OPERATOR_CONFIG"
  | "BLOCKED_REQUIRES_MIGRATION"
  | "BLOCKED_REQUIRES_TIME_WINDOW";

export type CredentialStatus = SelfTestStatus | "DORMANT_SEALED" | "ABSENT_BY_POLICY";

export type SelfTestBlock = {
  block: string;
  status: SelfTestStatus;
  evidence: string;
  last_checked_at: string;
  link: string;
  latency_ms?: number;
  operator_action?: string;
  days_remaining?: number;
};

export type SelfTestResponse = {
  mode: string;
  live_enabled: boolean;
  capital_exposed: number;
  broadcast: boolean;
  generated_at: string;
  total_blocks: number;
  verified_count: number;
  blocks: SelfTestBlock[];
};

export type CredentialEntry = {
  name: string;
  present: boolean;
  scope: "server" | "public_build_env";
  exposed_to_frontend: boolean;
  status: CredentialStatus;
  note?: string;
};

export type CredentialsStatusResponse = {
  mode: string;
  live_enabled: boolean;
  capital_exposed: number;
  broadcast: boolean;
  policy: string;
  secrets: CredentialEntry[];
  generated_at: string;
};

export type SelfTestSnapshot = {
  selftest: SelfTestResponse | null;
  credentials: CredentialsStatusResponse | null;
  fetchedAt: string | null;
  source: string;
};

// ─── Presentation maps ───────────────────────────────────────────────────────

type BadgeVariant = "success" | "warning" | "destructive" | "info" | "secondary" | "outline";

const STATUS_VARIANT: Record<string, BadgeVariant> = {
  VERIFIED: "success",
  HONEST_EMPTY: "warning",
  PARTIAL: "warning",
  FAILING: "destructive",
  MISSING: "destructive",
  BLOCKED_REQUIRES_OPERATOR_SECRET: "info",
  BLOCKED_REQUIRES_OPERATOR_CONFIG: "info",
  BLOCKED_REQUIRES_MIGRATION: "info",
  BLOCKED_REQUIRES_TIME_WINDOW: "info",
  DISABLED_BY_POLICY: "secondary",
  DORMANT_SEALED: "secondary",
  ABSENT_BY_POLICY: "secondary",
};

const BLOCK_LABEL: Record<string, string> = {
  credentials: "Credentials",
  connections: "Connections (Postgres · Redis)",
  wallet: "Wallet safety posture",
  settings: "Trading settings",
  data_feeds: "Data feeds",
  strategies: "Strategies (forge registry)",
  simulation: "Simulation runtime",
  signals: "Signals (route outcomes)",
  paper: "Paper window (≥7d)",
  live_readiness: "Live readiness gate",
};

function statusVariant(status: string): BadgeVariant {
  return STATUS_VARIANT[status] ?? "outline";
}

// ─── Components ──────────────────────────────────────────────────────────────

function SafetyStrip() {
  return (
    <div
      data-testid="selftest-safety-strip"
      className="flex items-center gap-2 rounded-md border border-success/30 bg-success/5 px-4 py-2.5"
    >
      <ShieldCheckIcon className="size-4 shrink-0 text-success" aria-hidden />
      <span className="font-mono text-xs uppercase tracking-wider text-foreground/90">
        Live OFF · Capital 0 · Broadcast OFF — operator self-test surface (shadow/paper)
      </span>
    </div>
  );
}

function BlockRow({ block }: { block: SelfTestBlock }) {
  const isBlocked = block.status.startsWith("BLOCKED_");
  return (
    <div
      data-testid={`selftest-block-${block.block}`}
      data-status={block.status}
      className="grid grid-cols-[minmax(9rem,14rem)_1fr_auto] items-start gap-x-4 border-b border-border/40 py-3 last:border-b-0 max-sm:grid-cols-1 max-sm:gap-y-2"
    >
      <div className="min-w-0">
        <div className="text-sm font-medium text-foreground">
          {BLOCK_LABEL[block.block] ?? block.block}
        </div>
        <div className="mt-1">
          <Badge variant={statusVariant(block.status)} className="font-mono text-[10px] tracking-wider">
            {block.status}
          </Badge>
        </div>
      </div>

      <div className="min-w-0">
        <div className="text-xs text-muted-foreground">{block.evidence}</div>
        {isBlocked && block.operator_action && (
          <div className="mt-1 text-xs text-info">
            <span className="font-mono text-[10px] uppercase tracking-wider">operator action · </span>
            {block.operator_action}
          </div>
        )}
        {block.days_remaining !== undefined && (
          <div className="mt-0.5 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/80">
            days_remaining: {block.days_remaining}
          </div>
        )}
        <div className="mt-1 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          {block.latency_ms !== undefined && <span>{block.latency_ms}ms · </span>}
          checked{" "}
          <span suppressHydrationWarning>{block.last_checked_at}</span>
        </div>
      </div>

      <div className="pt-0.5">
        <Button asChild variant="outline" size="sm" className="font-mono text-[10px] uppercase tracking-widest">
          <Link href={block.link}>
            Open
            <ArrowUpRightIcon className="ml-1 size-3" aria-hidden />
          </Link>
        </Button>
      </div>
    </div>
  );
}

function CredentialsTable({ credentials }: { credentials: CredentialsStatusResponse }) {
  return (
    <Card data-testid="selftest-credentials-card">
      <CardHeader>
        <CardTitle className="text-sm">Credential matrix (presence only)</CardTitle>
        <p className="text-xs text-muted-foreground">
          presence only — values never leave the server. The api-server reports a boolean per
          variable; no value, mask, or fingerprint is ever emitted.
        </p>
      </CardHeader>
      <CardContent>
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead>
              <tr className="border-b border-border/60 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
                <th className="py-2 pr-4">name</th>
                <th className="py-2 pr-4">present</th>
                <th className="py-2 pr-4">scope</th>
                <th className="py-2">status</th>
              </tr>
            </thead>
            <tbody>
              {credentials.secrets.map((s) => (
                <tr
                  key={s.name}
                  data-testid={`selftest-credential-${s.name}`}
                  data-status={s.status}
                  className="border-b border-border/30 last:border-b-0"
                >
                  <td className="py-1.5 pr-4 font-mono text-[11px] text-foreground/90">{s.name}</td>
                  <td className="py-1.5 pr-4 font-mono" aria-label={s.present ? "present" : "absent"}>
                    {s.present ? "✓" : "✗"}
                  </td>
                  <td className="py-1.5 pr-4 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
                    {s.scope}
                  </td>
                  <td className="py-1.5">
                    <Badge variant={statusVariant(s.status)} className="font-mono text-[10px] tracking-wider">
                      {s.status}
                    </Badge>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  );
}

// ─── Client root ─────────────────────────────────────────────────────────────

async function fetchJson<T>(path: string): Promise<T | null> {
  try {
    const r = await fetch(`${getApiBaseUrl()}${path}`, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!r.ok) return null;
    return (await r.json()) as T;
  } catch {
    return null;
  }
}

export default function SelfTestClient({ initialSnapshot }: { initialSnapshot: SelfTestSnapshot }) {
  const [snapshot, setSnapshot] = useState<SelfTestSnapshot>(initialSnapshot);
  const [loading, setLoading] = useState(false);
  const [mounted, setMounted] = useState(false);

  // Non-deterministic work ONLY after mount (R1).
  useEffect(() => {
    setMounted(true);
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    const [selftest, credentials] = await Promise.all([
      fetchJson<SelfTestResponse>("/api/operator/selftest"),
      fetchJson<CredentialsStatusResponse>("/api/operator/credentials/status"),
    ]);
    setSnapshot({
      selftest,
      credentials,
      fetchedAt: new Date().toISOString(),
      source: "client-refresh",
    });
    setLoading(false);
  }, []);

  const { selftest, credentials } = snapshot;

  return (
    <>
      <PageHeader
        title="Operator self-test center"
        lede="One aggregated, evidence-based view of the 10 readiness blocks — credentials, connections, wallet posture, settings, feeds, strategies, simulation, signals, paper window, and the live-readiness gate. Every status is computed server-side from real probes; nothing is fabricated."
        actions={
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void refresh()}
            disabled={loading || !mounted}
            className="font-mono text-xs uppercase tracking-widest"
            data-testid="selftest-refresh"
          >
            <RefreshCwIcon className={`mr-2 size-3.5 ${loading ? "animate-spin" : ""}`} aria-hidden />
            refresh
          </Button>
        }
      />

      <div className="space-y-6">
        <SafetyStrip />

        {!selftest && (
          <Card data-testid="selftest-unavailable">
            <CardContent className="py-8 text-center">
              <div className="font-mono text-xs uppercase tracking-widest text-destructive">
                self-test unavailable
              </div>
              <p className="mt-2 text-xs text-muted-foreground">
                The edge could not return /api/operator/selftest ({snapshot.source}). This page never
                synthesizes data — fix connectivity and refresh.
              </p>
            </CardContent>
          </Card>
        )}

        {selftest && (
          <Card data-testid="selftest-checklist-card">
            <CardHeader>
              <div className="flex flex-wrap items-baseline justify-between gap-2">
                <CardTitle className="text-sm">
                  Self-test checklist — {selftest.verified_count}/{selftest.total_blocks} verified
                </CardTitle>
                <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/70">
                  generated <span suppressHydrationWarning>{selftest.generated_at}</span>
                </span>
              </div>
            </CardHeader>
            <CardContent>
              <div>
                {selftest.blocks.map((b) => (
                  <BlockRow key={b.block} block={b} />
                ))}
              </div>
            </CardContent>
          </Card>
        )}

        {!credentials && (
          <Card data-testid="selftest-credentials-unavailable">
            <CardContent className="py-8 text-center">
              <div className="font-mono text-xs uppercase tracking-widest text-destructive">
                credential matrix unavailable
              </div>
              <p className="mt-2 text-xs text-muted-foreground">
                The edge could not return /api/operator/credentials/status. Presence data is
                server-evaluated only — nothing is rendered without it.
              </p>
            </CardContent>
          </Card>
        )}

        {credentials && <CredentialsTable credentials={credentials} />}
      </div>
    </>
  );
}
