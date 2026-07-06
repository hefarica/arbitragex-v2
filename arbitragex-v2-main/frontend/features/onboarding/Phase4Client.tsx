/**
 * Phase 4 — Real testing (paper-mode).
 *
 * R1 Mounted Snapshot Pattern. R8 Fail-Honest endpoint guard.
 */
"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { AlertCircleIcon, ArrowLeftIcon, CheckCircle2Icon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PageHeader } from "@/components/page-header";
import { hasAdminSession } from "@/lib/admin-token";
import { completeOnboardingPhase4, type OnboardingStatus } from "@/lib/api-client";

interface Props {
  initialSnapshot: OnboardingStatus;
}

export function Phase4Client({ initialSnapshot }: Props) {
  const [status, setStatus] = useState<OnboardingStatus>(initialSnapshot);
  const [confirmedBy, setConfirmedBy] = useState("");
  const [relayCount, setRelayCount] = useState("0");
  const [signerZeroBalanceVerified, setSignerZeroBalanceVerified] = useState(false);
  const [cfTunnelDeployed, setCfTunnelDeployed] = useState(false);
  const [hasSession, setHasSession] = useState(false);
  const [busy, setBusy] = useState(false);
  const [resultOk, setResultOk] = useState<string | null>(null);
  const [resultErr, setResultErr] = useState<string | null>(null);

  // R1: non-deterministic reads deferred to useEffect.
  useEffect(() => {
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  const alreadyDone = Boolean(status.phase_4_completed_at);

  const parsedRelayCount = Number(relayCount);
  const canSubmit =
    hasSession &&
    confirmedBy.trim().length >= 1 &&
    Number.isInteger(parsedRelayCount) &&
    parsedRelayCount >= 0 &&
    signerZeroBalanceVerified &&
    cfTunnelDeployed &&
    !busy;

  async function submit() {
    setBusy(true);
    setResultOk(null);
    setResultErr(null);
    const r = await completeOnboardingPhase4(
      confirmedBy.trim(),
      parsedRelayCount,
      signerZeroBalanceVerified,
      cfTunnelDeployed,
    );
    setBusy(false);
    if (!r.ok) {
      const isNotImpl = r.error.includes("404") || r.error.includes("501") || r.error.includes("not implemented");
      setResultErr(
        isNotImpl
          ? "endpoint not yet implemented on this deployment — use the curl command below"
          : r.error,
      );
      return;
    }
    setResultOk(
      `Phase 4 completed at ${new Date(r.data.phase_4_completed_at).toLocaleString()} by ${r.data.phase_4_completed_by}. Signer zero-balance verified: ${r.data.phase_4_signer_zero_balance_verified ? "yes" : "no"}.`,
    );
    setStatus((prev) => ({
      ...prev,
      phase_4_completed_at: r.data.phase_4_completed_at,
      phase_4_completed_by: r.data.phase_4_completed_by,
      phase_4_signer_zero_balance_verified: r.data.phase_4_signer_zero_balance_verified,
    }));
  }

  return (
    <>
      <PageHeader
        title="Phase 4 — Real testing (paper-mode)"
        lede="Populate the relay catalog, attach a zero-balance Flashbots signer, deploy the Cloudflare Tunnel for admin access."
        actions={
          <Link
            href="/onboarding"
            className="inline-flex items-center gap-2 rounded-md border px-3 py-1.5 text-sm hover:bg-accent"
          >
            <ArrowLeftIcon className="size-4" /> Back
          </Link>
        }
      />

      {!hasSession && (
        <Alert variant="destructive" className="mb-6">
          <AlertCircleIcon />
          <AlertTitle>No admin session</AlertTitle>
          <AlertDescription>
            Open{" "}
            <Link className="underline" href="/killswitch">
              /killswitch
            </Link>{" "}
            and unlock an admin session first.
          </AlertDescription>
        </Alert>
      )}

      {alreadyDone && (
        <Alert variant="success" className="mb-6">
          <CheckCircle2Icon />
          <AlertTitle>Phase 4 already completed</AlertTitle>
          <AlertDescription>
            Completed at {status.phase_4_completed_at && new Date(status.phase_4_completed_at).toLocaleString()} by{" "}
            <code>{status.phase_4_completed_by}</code>. Signer zero-balance verified:{" "}
            {status.phase_4_signer_zero_balance_verified ? "yes" : "no"}.
          </AlertDescription>
        </Alert>
      )}

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">Solicits</CardTitle>
          <CardDescription>Values provisioned outside this UI.</CardDescription>
        </CardHeader>
        <CardContent>
          <ul className="flex flex-wrap gap-2 text-xs">
            {[
              "relay catalog entries (DB rows)",
              "FLASHBOTS_SIGNER_KEY (zero balance)",
              "CF_API_TOKEN + domain + tunnel token",
            ].map((x) => (
              <li key={x}>
                <code className="rounded bg-muted px-2 py-0.5 font-mono">{x}</code>
              </li>
            ))}
          </ul>
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">Relay catalog</CardTitle>
          <CardDescription>
            Relays are populated directly in the database. Enter the total row count you have inserted. Zero is valid if you are using only Flashbots protect without additional relays.
          </CardDescription>
        </CardHeader>
        <CardContent className="max-w-xs">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="relay_count" className="text-xs text-muted-foreground">relay_count (DB rows inserted)</Label>
            <Input
              id="relay_count"
              type="number"
              min={0}
              step={1}
              value={relayCount}
              onChange={(e) => setRelayCount(e.target.value)}
            />
            <p className="text-xs text-muted-foreground">
              Verify with: <code>psql -c &apos;SELECT count(*) FROM relays;&apos;</code>
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Record completion</CardTitle>
          <CardDescription>
            Writes to <code>onboarding_progress</code> and <code>audit_log</code>.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 max-w-xl">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="confirmed_by_4">Operator identity</Label>
            <Input
              id="confirmed_by_4"
              placeholder="alice@arbx or handle"
              value={confirmedBy}
              onChange={(e) => setConfirmedBy(e.target.value)}
              autoComplete="off"
            />
          </div>

          <label htmlFor="signer_ack" className="flex items-start gap-3 cursor-pointer">
            <input
              id="signer_ack"
              type="checkbox"
              className="mt-1 size-4 rounded border-border accent-primary"
              checked={signerZeroBalanceVerified}
              onChange={(e) => setSignerZeroBalanceVerified(e.target.checked)}
            />
            <span className="text-sm leading-relaxed">
              I have verified with <code>cast balance &lt;SIGNER_ADDRESS&gt;</code> that the Flashbots signer wallet holds zero ETH balance. Real capital must not be on this key at paper-mode stage.
            </span>
          </label>

          <label htmlFor="cf_tunnel_ack" className="flex items-start gap-3 cursor-pointer">
            <input
              id="cf_tunnel_ack"
              type="checkbox"
              className="mt-1 size-4 rounded border-border accent-primary"
              checked={cfTunnelDeployed}
              onChange={(e) => setCfTunnelDeployed(e.target.checked)}
            />
            <span className="text-sm leading-relaxed">
              Cloudflare Tunnel is deployed and the admin panel is reachable at the configured domain. <code>cloudflared tunnel list</code> shows the tunnel as healthy.
            </span>
          </label>

          {resultErr && (
            <Alert variant="destructive">
              <AlertCircleIcon />
              <AlertTitle>
                {resultErr.includes("endpoint not yet") ? (
                  <span className="flex items-center gap-2">
                    Endpoint not implemented
                    <Badge variant="outline" className="font-mono text-xs">curl-only stub</Badge>
                  </span>
                ) : (
                  "Could not record completion"
                )}
              </AlertTitle>
              <AlertDescription>
                <code className="font-mono text-xs">{resultErr}</code>
              </AlertDescription>
            </Alert>
          )}

          {resultOk && (
            <Alert variant="success">
              <CheckCircle2Icon />
              <AlertTitle>Phase 4 complete</AlertTitle>
              <AlertDescription>{resultOk}</AlertDescription>
            </Alert>
          )}

          <div className="flex items-center gap-3 pt-2 border-t">
            <Button type="button" onClick={submit} disabled={!canSubmit}>
              {busy ? "Saving…" : alreadyDone ? "Re-record" : "Record phase 4 complete"}
            </Button>
            {!hasSession && <Badge variant="outline">admin session required</Badge>}
            {(!signerZeroBalanceVerified || !cfTunnelDeployed) && hasSession && (
              <Badge variant="outline">check both confirmations</Badge>
            )}
          </div>
        </CardContent>
      </Card>
    </>
  );
}
