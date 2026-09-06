"use client";

/**
 * GoNoGoSignOffCard — A.9 formal sign-off visibility surface.
 *
 * Consumes /api/go-no-go/status (edge → /api/v1/go-no-go/status): the
 * RECORDED state of the A.9 ledger generation — hash, sign-off rows,
 * quorum state, and the go_live_eligible derivation.
 *
 * What this card IS: read-only visibility. The operator sees exactly which
 * ledger hash has been generated, who has signed it (actor + decision), and
 * what the recorded state machine says (awaiting_first → awaiting_second →
 * signed_go / signed_no_go / conflicted). A "Regenerate ledger" button hits
 * the GET ledger endpoint (self-generating + deduplicating + persisted to
 * audit_log) — it creates documents, never signatures.
 *
 * What this card is NOT: a signing surface. POST /admin/go-no-go/sign-off
 * requires x-arbx-admin-token + x-arbx-actor and is NEVER routed through
 * the edge (backend/api-server only). Signing is curl-only BY DESIGN —
 * §34.3: sign-off is an operator-only act, not inferable from a UI click.
 * The runbook <pre> below documents the exact procedure with placeholders.
 *
 * Structural assertions (regression-tested):
 *   - There is NO button that signs GO or NO_GO.
 *   - There is NO flip-to-live control.
 *   - The runbook shows PLACEHOLDERS (<VPS-IP>, $ARBX_ADMIN_TOKEN,
 *     <LEDGER_HASH>) — never real hostnames or tokens (RULE 00 / no-hardcode).
 */

import * as React from "react";
import { AlertCircleIcon, RefreshCwIcon, SignatureIcon } from "lucide-react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { LastUpdated } from "@/components/last-updated";
import { getGoNoGoStatus, regenerateGoNoGoLedger } from "@/lib/api-client";
import type { GoNoGoLedgerResponse, GoNoGoStatusResponse } from "@/lib/schemas";

type State =
  | { kind: "loading" }
  | { kind: "error"; detail: string }
  | { kind: "ok"; data: GoNoGoStatusResponse };

type RegenState =
  | { kind: "idle" }
  | { kind: "busy" }
  | { kind: "ok"; ledger: GoNoGoLedgerResponse }
  | { kind: "error"; detail: string };

const POLL_MS = 30_000;

const STATE_LABEL: Record<GoNoGoStatusResponse["state"], string> = {
  awaiting_first: "awaiting first sign-off",
  awaiting_second: "awaiting second sign-off",
  signed_go: "signed GO (quorum 2/2)",
  signed_no_go: "signed NO-GO (quorum 2/2)",
  conflicted: "conflicted (GO + NO-GO)",
  no_ledger: "no ledger generated",
};

function shortHash(hash: string): string {
  // 64-char sha256 → leading 12 chars is unambiguous in this UI.
  return hash.length > 12 ? `${hash.slice(0, 12)}…` : hash;
}

export function GoNoGoSignOffCard() {
  const [state, setState] = React.useState<State>({ kind: "loading" });
  const [fetchedAt, setFetchedAt] = React.useState<number | null>(null);
  const [regen, setRegen] = React.useState<RegenState>({ kind: "idle" });

  const tick = React.useCallback(async () => {
    const r = await getGoNoGoStatus();
    if (r.ok) setState({ kind: "ok", data: r.data });
    else setState({ kind: "error", detail: r.error.slice(0, 200) });
    setFetchedAt(Date.now());
  }, []);

  React.useEffect(() => {
    let alive = true;
    const wrapped = async () => {
      if (alive) await tick();
    };
    void wrapped();
    const id = setInterval(wrapped, POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [tick]);

  const onRegenerate = async () => {
    setRegen({ kind: "busy" });
    const r = await regenerateGoNoGoLedger();
    if (r.ok) {
      setRegen({ kind: "ok", ledger: r.data });
      await tick();
    } else {
      setRegen({ kind: "error", detail: r.error.slice(0, 200) });
    }
  };

  return (
    <Card data-slot="go-no-go-signoff-card">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              <SignatureIcon className="size-4" />
              A.9 formal sign-off
              {state.kind === "ok" && <StateBadge state={state.data.state} />}
            </CardTitle>
            <CardDescription>
              Recorded ledger state from <code className="font-mono text-[11px]">/api/go-no-go/status</code>.
              <span className="ml-1">Sign-off happens via admin API only — there is no sign button here.</span>
            </CardDescription>
          </div>
          <div className="flex flex-col items-end gap-1.5">
            <Button
              variant="outline"
              size="sm"
              className="font-mono text-xs"
              onClick={() => void onRegenerate()}
              disabled={regen.kind === "busy"}
            >
              {regen.kind === "busy" ? (
                <RefreshCwIcon className="size-3 animate-spin" />
              ) : (
                <RefreshCwIcon className="size-3" />
              )}
              Regenerate ledger
            </Button>
            <LastUpdated at={fetchedAt} pollMs={POLL_MS} />
          </div>
        </div>
      </CardHeader>
      <CardContent>
        {state.kind === "loading" && (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <RefreshCwIcon className="size-3 animate-spin" />
            Loading sign-off ledger…
          </div>
        )}
        {state.kind === "error" && (
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>Cannot fetch sign-off status</AlertTitle>
            <AlertDescription className="font-mono text-xs">{state.detail}</AlertDescription>
          </Alert>
        )}
        {state.kind === "ok" && <SignOffBody data={state.data} />}
        {regen.kind === "ok" && (
          <div className="mt-3 rounded-md border bg-muted/30 p-3 text-xs">
            Ledger regenerated: <code className="font-mono">{shortHash(regen.ledger.ledger_hash)}</code>{" "}
            {regen.ledger.deduplicated ? "(identical facts — deduplicated, no new audit row)" : "(new generation persisted to audit_log)"}
          </div>
        )}
        {regen.kind === "error" && (
          <div className="mt-3 rounded-md border border-destructive/40 bg-destructive/5 p-3 text-xs text-destructive">
            Regeneration failed: <span className="font-mono">{regen.detail}</span>
          </div>
        )}
        {/* Runbook always rendered — the operator must be able to read the
            procedure in ANY state (loading / no_ledger / conflicted). Uses the
            live hash when known, <LEDGER_HASH> placeholder otherwise. */}
        <div className="mt-3">
          <RunbookPre ledgerHash={state.kind === "ok" ? state.data.ledger_hash : null} />
        </div>
      </CardContent>
    </Card>
  );
}

function StateBadge({ state }: { state: GoNoGoStatusResponse["state"] }) {
  if (state === "signed_go") {
    return (
      <Badge variant="info" className="font-mono text-[10px] uppercase">
        {STATE_LABEL[state]}
      </Badge>
    );
  }
  if (state === "signed_no_go" || state === "conflicted") {
    return (
      <Badge variant="destructive" className="font-mono text-[10px] uppercase">
        {STATE_LABEL[state]}
      </Badge>
    );
  }
  return (
    <Badge variant="outline" className="font-mono text-[10px] uppercase">
      {STATE_LABEL[state]}
    </Badge>
  );
}

function SignOffBody({ data }: { data: GoNoGoStatusResponse }) {
  const unresolved =
    data.ledger_summary?.unresolved_blockers === undefined || data.ledger_summary.unresolved_blockers === null
      ? "—"
      : String(data.ledger_summary.unresolved_blockers);
  const paperSafe =
    data.ledger_summary?.paper_safe === undefined || data.ledger_summary.paper_safe === null
      ? "—"
      : data.ledger_summary.paper_safe
        ? "YES"
        : "NO";

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
        <FlagTile
          label="go_live_eligible"
          value={data.go_live_eligible ? "YES" : "NO"}
          tone={data.go_live_eligible ? "safe" : "neutral"}
        />
        <FlagTile
          label="unresolved_blockers"
          value={unresolved}
          tone={data.ledger_summary?.unresolved_blockers === 0 ? "safe" : "neutral"}
        />
        <FlagTile
          label="paper_safe"
          value={paperSafe}
          tone={data.ledger_summary?.paper_safe === true ? "safe" : "neutral"}
        />
      </div>

      <div className="rounded-md border bg-muted/30 p-3">
        <div className="text-[10px] uppercase tracking-wider text-muted-foreground">Ledger generation</div>
        <div className="mt-1 space-y-0.5 font-mono text-xs">
          <div>
            hash:{" "}
            {data.ledger_hash ? (
              <span title={data.ledger_hash}>{shortHash(data.ledger_hash)}</span>
            ) : (
              <span className="italic text-muted-foreground">none persisted</span>
            )}
          </div>
          <div>
            generated_at:{" "}
            {data.generated_at ? (
              <span>{data.generated_at}</span>
            ) : (
              <span className="italic text-muted-foreground">—</span>
            )}
          </div>
        </div>
      </div>

      <div className="rounded-md border bg-muted/30 p-3">
        <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
          Sign-offs recorded ({data.sign_offs.length})
        </div>
        {data.sign_offs.length === 0 ? (
          <div className="mt-1 text-xs italic text-muted-foreground">
            No operator has signed this ledger generation.
          </div>
        ) : (
          <ul className="mt-1 space-y-1 text-xs">
            {data.sign_offs.map((s, i) => (
              <li key={i} className="flex flex-wrap items-center gap-2">
                <span className="font-mono">{s.actor}</span>
                <Badge variant={s.decision === "GO" ? "info" : "destructive"} className="font-mono text-[10px]">
                  {s.decision}
                </Badge>
                <span className="text-muted-foreground">{s.signed_at ?? "—"}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function FlagTile({ label, value, tone }: { label: string; value: string; tone: "safe" | "neutral" }) {
  const accent =
    tone === "safe"
      ? "border-emerald-500/40 bg-emerald-500/5 text-emerald-700 dark:text-emerald-300"
      : "border-border bg-muted/20 text-foreground";
  return (
    <div data-slot="signoff-flag-tile" className={`rounded-md border px-2.5 py-1.5 text-[11px] uppercase tracking-wider ${accent}`}>
      <div className="text-foreground/60">{label}</div>
      <div className="mt-0.5 font-mono text-sm font-semibold normal-case tracking-tight">{value}</div>
    </div>
  );
}

/**
 * Curl runbook with PLACEHOLDERS ONLY (RULE 00 — no real hosts/tokens in the
 * repo). Two operators, two distinct actors, same ledger_hash → quorum 2/2.
 */
function RunbookPre({ ledgerHash }: { ledgerHash: string | null }) {
  const hashPlaceholder = ledgerHash ?? "<LEDGER_HASH>";
  return (
    <div className="rounded-md border bg-muted/30 p-3">
      <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
        Sign-off runbook (admin API — 2 distinct operators, same ledger hash)
      </div>
      <pre className="mt-1 overflow-x-auto font-mono text-[10px] leading-relaxed text-muted-foreground">
{`# 1. Regenerate + read the current ledger (any operator, read-only):
curl -s http://<VPS-IP>:8080/api/v1/go-no-go/ledger | head -c 600

# 2. Each operator signs (GO or NO_GO) — direct to api-server, NEVER via edge:
curl -X POST http://<VPS-IP>:8080/admin/go-no-go/sign-off \\
  -H "Content-Type: application/json" \\
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \\
  -H "x-arbx-actor: <OPERATOR_NAME>" \\
  -d '{"decision":"GO","ledger_hash":"${hashPlaceholder}"}'

# 3. Verify quorum (read-only):
curl -s http://<VPS-IP>:8080/api/v1/go-no-go/status`}
      </pre>
    </div>
  );
}
