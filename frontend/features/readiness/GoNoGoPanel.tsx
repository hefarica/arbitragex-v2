"use client";

/**
 * GoNoGoPanel — go/no-go verdict surface (P2).
 *
 * Consumes /api/readiness/decision (proxied to /api/v1/readiness/decision).
 *
 * Renders a compact "verdict tile" in the page flow, AND opens a Dialog modal
 * (P0 primitive) when the operator clicks "Decision detail". The dialog
 * surfaces:
 *   - A.5 GO/NO-GO badge
 *   - Live GO/NO-GO badge (always NO-GO in this phase — structural barrier)
 *   - reasons[]
 *   - next_action
 *   - required_for_go_live[]
 *   - capital_exposure_usd ($0)
 *   - paper_mode, live_trading, private_relay, submit_enabled flags
 *
 * Structural assertions (regression-tested):
 *   - There is NO button that flips Live to ON.
 *   - There is NO submit/broadcast/private-relay control.
 *   - The verdict tile always reflects the backend response verbatim. If
 *     backend says NO_GO, UI shows NO_GO. If backend (hypothetically) said
 *     GO, the UI would show GO — but a separate test confirms that under
 *     the current code path, backend ALWAYS returns NO_GO regardless of
 *     blockers (go_live is hardcoded false in readiness-extras.ts).
 */

import * as React from "react";
import { ShieldCheckIcon, ShieldOffIcon, AlertCircleIcon, RefreshCwIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { getReadinessDecision } from "@/lib/api-client";
import type { ReadinessDecisionResponse } from "@/lib/schemas";

type State =
  | { kind: "loading" }
  | { kind: "error"; detail: string }
  | { kind: "ok"; data: ReadinessDecisionResponse };

const POLL_MS = 30_000;

export function GoNoGoPanel() {
  const [state, setState] = React.useState<State>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;
    const tick = async () => {
      const r = await getReadinessDecision();
      if (!alive) return;
      if (r.ok) setState({ kind: "ok", data: r.data });
      else setState({ kind: "error", detail: r.error.slice(0, 200) });
    };
    void tick();
    const id = setInterval(tick, POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  return (
    <Card data-slot="go-no-go-panel">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              GO / NO-GO decision
              <Badge variant="destructive" className="font-mono text-[10px] uppercase">
                Live trading: OFF
              </Badge>
            </CardTitle>
            <CardDescription>
              Verdict derived from <code className="font-mono text-[11px]">/api/readiness/decision</code>.
              <span className="ml-1">There is no UI control to flip Live to ON.</span>
            </CardDescription>
          </div>
          {state.kind === "ok" && (
            <DecisionDetailDialog data={state.data} />
          )}
        </div>
      </CardHeader>
      <CardContent>
        {state.kind === "loading" && (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <RefreshCwIcon className="size-3 animate-spin" />
            Loading decision…
          </div>
        )}
        {state.kind === "error" && (
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>Cannot fetch decision</AlertTitle>
            <AlertDescription className="font-mono text-xs">{state.detail}</AlertDescription>
          </Alert>
        )}
        {state.kind === "ok" && <VerdictTile data={state.data} />}
      </CardContent>
    </Card>
  );
}

function VerdictTile({ data }: { data: ReadinessDecisionResponse }) {
  const goLiveBadge =
    data.go_live ? (
      <Badge variant="info" className="font-mono">GO</Badge>
    ) : (
      <Badge variant="destructive" className="font-mono">NO-GO</Badge>
    );
  const goA5Badge =
    data.go_a5 ? (
      <Badge variant="info" className="font-mono">GO</Badge>
    ) : (
      <Badge variant="destructive" className="font-mono">NO-GO</Badge>
    );

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        <div className="rounded-md border bg-muted/30 p-3">
          <div className="text-[10px] uppercase tracking-wider text-muted-foreground">A.5 paper-shadow</div>
          <div className="mt-1 flex items-center gap-2">{goA5Badge}</div>
        </div>
        <div className="rounded-md border border-destructive/30 bg-destructive/5 p-3">
          <div className="text-[10px] uppercase tracking-wider text-destructive">Live trading</div>
          <div className="mt-1 flex items-center gap-2">{goLiveBadge}</div>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <FlagTile label="Capital" value={`$${data.capital_exposure_usd.toFixed(0)}`} tone="safe" />
        <FlagTile label="Paper mode" value={data.paper_mode ? "ON" : "OFF"} tone={data.paper_mode ? "safe" : "danger"} />
        <FlagTile label="Private relay" value={data.private_relay ? "ON" : "OFF"} tone={data.private_relay ? "danger" : "safe"} />
        <FlagTile label="Submit" value={data.submit_enabled ? "ON" : "OFF"} tone={data.submit_enabled ? "danger" : "safe"} />
      </div>

      {data.reasons.length > 0 && (
        <div className="rounded-md border bg-muted/30 p-3">
          <div className="text-[10px] uppercase tracking-wider text-muted-foreground">Top reasons (critical)</div>
          <ul className="mt-1 list-inside list-disc space-y-0.5 text-xs">
            {data.reasons.slice(0, 4).map((r, i) => (
              <li key={i}>{r}</li>
            ))}
          </ul>
        </div>
      )}

      <div className="rounded-md border bg-muted/30 p-3 text-xs">
        <div className="text-[10px] uppercase tracking-wider text-muted-foreground">Next action</div>
        <p className="mt-0.5">{data.next_action}</p>
      </div>
    </div>
  );
}

function FlagTile({ label, value, tone }: { label: string; value: string; tone: "safe" | "danger" }) {
  const accent =
    tone === "safe"
      ? "border-emerald-500/40 bg-emerald-500/5 text-emerald-700 dark:text-emerald-300"
      : "border-destructive/40 bg-destructive/5 text-destructive";
  return (
    <div data-slot="flag-tile" data-tone={tone} className={`rounded-md border px-2.5 py-1.5 text-[11px] uppercase tracking-wider ${accent}`}>
      <div className="text-foreground/60">{label}</div>
      <div className="mt-0.5 font-mono text-sm font-semibold normal-case tracking-tight">{value}</div>
    </div>
  );
}

function DecisionDetailDialog({ data }: { data: ReadinessDecisionResponse }) {
  return (
    <Dialog>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm" className="font-mono text-xs">
          Decision detail
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {data.go_live ? (
              <ShieldCheckIcon className="size-5 text-success" />
            ) : (
              <ShieldOffIcon className="size-5 text-destructive" />
            )}
            Verdict: <span className="font-mono">{data.verdict}</span>
          </DialogTitle>
          <DialogDescription>
            Generated at <code className="font-mono">{data.generated_at}</code>. Phase: <code className="font-mono">{data.phase}</code>.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-4">
            <DetailKV k="go_a5" v={String(data.go_a5)} />
            <DetailKV k="go_live" v={String(data.go_live)} />
            <DetailKV k="paper_mode" v={String(data.paper_mode)} />
            <DetailKV k="live_trading" v={String(data.live_trading)} />
            <DetailKV k="private_relay" v={String(data.private_relay)} />
            <DetailKV k="submit_enabled" v={String(data.submit_enabled)} />
            <DetailKV k="capital_exposure_usd" v={`$${data.capital_exposure_usd}`} />
            <DetailKV k="blockers_ref" v={data.blockers_ref} />
          </div>

          <div>
            <div className="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">Reasons</div>
            <ul className="list-inside list-disc space-y-0.5 text-xs">
              {data.reasons.length === 0 ? (
                <li className="italic text-muted-foreground">No critical reasons reported.</li>
              ) : (
                data.reasons.map((r, i) => <li key={i}>{r}</li>)
              )}
            </ul>
          </div>

          <div className="rounded-md border bg-muted/30 p-3 text-xs">
            <div className="text-[10px] uppercase tracking-wider text-muted-foreground">Next action</div>
            <p className="mt-0.5">{data.next_action}</p>
          </div>

          <div>
            <div className="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">
              Required for go_live ({data.required_for_go_live.length} items)
            </div>
            <ul className="list-inside list-disc space-y-0.5 text-xs">
              {data.required_for_go_live.map((r, i) => (
                <li key={i}>{r}</li>
              ))}
            </ul>
          </div>

          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>Structural barrier</AlertTitle>
            <AlertDescription className="text-xs">
              This dialog does NOT expose a flip-to-live control. Live activation requires a
              backend code change + A.9 formal sign-off in audit_logs. There is no operator
              path through this UI to enable live trading or submit transactions.
            </AlertDescription>
          </Alert>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function DetailKV({ k, v }: { k: string; v: string }) {
  return (
    <div className="rounded border bg-muted/20 p-1.5">
      <div className="text-[9px] uppercase text-muted-foreground">{k}</div>
      <div className="font-mono text-[11px]">{v}</div>
    </div>
  );
}
