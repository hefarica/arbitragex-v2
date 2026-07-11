/**
 * Phase 5 — Production operation.
 *
 * R1 Mounted Snapshot Pattern. R8 Fail-Honest endpoint guard.
 * This is the production-flip phase: paper_mode_off is a significant
 * irreversible operation. The form requires an explicit capital cap
 * acknowledgement and incident contacts count.
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
import { completeOnboardingPhase5, type OnboardingStatus } from "@/lib/api-client";

interface Props {
  initialSnapshot: OnboardingStatus;
}

export function Phase5Client({ initialSnapshot }: Props) {
  const [status, setStatus] = useState<OnboardingStatus>(initialSnapshot);
  const [confirmedBy, setConfirmedBy] = useState("");
  const [pagerdutKeyRef, setPagerdutKeyRef] = useState("");
  const [backupDestination, setBackupDestination] = useState("");
  const [capitalCapUsd, setCapitalCapUsd] = useState("");
  const [incidentContactsCount, setIncidentContactsCount] = useState("0");
  const [paperModeOff, setPaperModeOff] = useState(false);
  const [ackCapital, setAckCapital] = useState(false);
  const [ackIrreversible, setAckIrreversible] = useState(false);
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

  const alreadyDone = Boolean(status.phase_5_completed_at);

  const parsedCapital = Number(capitalCapUsd);
  const parsedContacts = Number(incidentContactsCount);
  const canSubmit =
    hasSession &&
    confirmedBy.trim().length >= 1 &&
    pagerdutKeyRef.trim().length >= 1 &&
    backupDestination.trim().length >= 1 &&
    Number.isFinite(parsedCapital) &&
    parsedCapital > 0 &&
    Number.isInteger(parsedContacts) &&
    parsedContacts >= 1 &&
    ackCapital &&
    ackIrreversible &&
    !busy;

  async function submit() {
    setBusy(true);
    setResultOk(null);
    setResultErr(null);
    const r = await completeOnboardingPhase5(
      confirmedBy.trim(),
      pagerdutKeyRef.trim(),
      backupDestination.trim(),
      parsedCapital,
      parsedContacts,
      paperModeOff,
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
    const paperAt = r.data.phase_5_paper_mode_off_at
      ? new Date(r.data.phase_5_paper_mode_off_at).toLocaleString()
      : "not flipped";
    setResultOk(
      `Phase 5 completed at ${new Date(r.data.phase_5_completed_at).toLocaleString()} by ${r.data.phase_5_completed_by}. Paper mode flipped: ${paperAt}.`,
    );
    setStatus((prev) => ({
      ...prev,
      phase_5_completed_at: r.data.phase_5_completed_at,
      phase_5_completed_by: r.data.phase_5_completed_by,
      phase_5_paper_mode_off_at: r.data.phase_5_paper_mode_off_at,
    }));
  }

  return (
    <>
      <PageHeader
        title="Phase 5 — Production operation"
        lede="Flip paper-mode off, wire PagerDuty, wire off-site backups, record incident contacts, confirm a capital cap. This is the final gate before live execution."
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
          <AlertTitle>Phase 5 already completed</AlertTitle>
          <AlertDescription>
            Completed at {status.phase_5_completed_at && new Date(status.phase_5_completed_at).toLocaleString()} by{" "}
            <code>{status.phase_5_completed_by}</code>.{" "}
            {status.phase_5_paper_mode_off_at
              ? `Paper mode was flipped off at ${new Date(status.phase_5_paper_mode_off_at).toLocaleString()}.`
              : "Paper mode was NOT flipped in this record."}
          </AlertDescription>
        </Alert>
      )}

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">PagerDuty integration</CardTitle>
          <CardDescription>
            The integration key is stored as a Vault path reference. The plaintext key must already exist at that path.
          </CardDescription>
        </CardHeader>
        <CardContent className="max-w-xl">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="pd_key_ref" className="text-xs text-muted-foreground">
              PAGERDUTY_INTEGRATION_KEY — Vault path
            </Label>
            <Input
              id="pd_key_ref"
              placeholder="secret/pagerduty/integration-key"
              value={pagerdutKeyRef}
              onChange={(e) => setPagerdutKeyRef(e.target.value)}
              autoComplete="off"
            />
            <p className="text-xs text-muted-foreground">
              Only the Vault path is recorded — never the key itself.
            </p>
          </div>
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">Off-site backup destination</CardTitle>
          <CardDescription>
            Daily encrypted PostgreSQL dumps are pushed here. Format: <code>b2://bucket-name/prefix</code> or an S3-compatible URI.
          </CardDescription>
        </CardHeader>
        <CardContent className="max-w-xl">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="backup_dest" className="text-xs text-muted-foreground">Backup destination URI</Label>
            <Input
              id="backup_dest"
              placeholder="b2://my-arbx-backups/db"
              value={backupDestination}
              onChange={(e) => setBackupDestination(e.target.value)}
              autoComplete="off"
            />
          </div>
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">Capital cap &amp; incident contacts</CardTitle>
          <CardDescription>
            The capital cap is the absolute ceiling on USD at-risk for any single trade. It is recorded as a signed-off value — the system never enforces it automatically; that is the risk engine's responsibility.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 max-w-xl">
          <div className="grid grid-cols-2 gap-4">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="capital_cap" className="text-xs text-muted-foreground">Capital cap (USD)</Label>
              <Input
                id="capital_cap"
                type="number"
                min={1}
                step={100}
                placeholder="50000"
                value={capitalCapUsd}
                onChange={(e) => setCapitalCapUsd(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">Must be &gt; 0. R8: no default invented.</p>
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="incident_contacts" className="text-xs text-muted-foreground">
                Incident contact count
              </Label>
              <Input
                id="incident_contacts"
                type="number"
                min={1}
                step={1}
                placeholder="2"
                value={incidentContactsCount}
                onChange={(e) => setIncidentContactsCount(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                Minimum 1. Actual contact data lives in a separate DB table.
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Record completion &amp; paper-mode flip</CardTitle>
          <CardDescription>
            Writes to <code>onboarding_progress</code> and <code>audit_log</code>. Flipping paper-mode causes the executor to begin signing and submitting bundles.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 max-w-xl">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="confirmed_by_5">Operator identity</Label>
            <Input
              id="confirmed_by_5"
              placeholder="alice@arbx or handle"
              value={confirmedBy}
              onChange={(e) => setConfirmedBy(e.target.value)}
              autoComplete="off"
            />
          </div>

          <label htmlFor="paper_mode_toggle" className="flex items-start gap-3 cursor-pointer">
            <input
              id="paper_mode_toggle"
              type="checkbox"
              className="mt-1 size-4 rounded border-border accent-primary"
              checked={paperModeOff}
              onChange={(e) => setPaperModeOff(e.target.checked)}
            />
            <span className="text-sm leading-relaxed">
              Flip <code>ARBX_PAPER_TRADE</code> to <code>false</code> — enable live execution. Leave unchecked to record phase 5 completion without enabling live execution yet.
            </span>
          </label>

          <label htmlFor="ack_capital" className="flex items-start gap-3 cursor-pointer">
            <input
              id="ack_capital"
              type="checkbox"
              className="mt-1 size-4 rounded border-border accent-primary"
              checked={ackCapital}
              onChange={(e) => setAckCapital(e.target.checked)}
            />
            <span className="text-sm leading-relaxed">
              I confirm the capital cap of <strong>{parsedCapital > 0 ? `$${parsedCapital.toLocaleString()}` : "—"}</strong> is within my risk tolerance and has been reviewed with the team.
            </span>
          </label>

          <label htmlFor="ack_irreversible" className="flex items-start gap-3 cursor-pointer">
            <input
              id="ack_irreversible"
              type="checkbox"
              className="mt-1 size-4 rounded border-border accent-primary"
              checked={ackIrreversible}
              onChange={(e) => setAckIrreversible(e.target.checked)}
            />
            <span className="text-sm leading-relaxed">
              I understand that once live execution is enabled, the system will begin submitting real transactions to the mempool. I have tested extensively in paper-mode and am satisfied with the results.
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
              <AlertTitle>Phase 5 complete</AlertTitle>
              <AlertDescription>{resultOk}</AlertDescription>
            </Alert>
          )}

          <div className="flex items-center gap-3 pt-2 border-t">
            <Button
              type="button"
              onClick={submit}
              disabled={!canSubmit}
              variant={paperModeOff ? "destructive" : "default"}
            >
              {busy
                ? "Saving…"
                : alreadyDone
                  ? "Re-record"
                  : paperModeOff
                    ? "Record phase 5 + ENABLE LIVE EXECUTION"
                    : "Record phase 5 complete"}
            </Button>
            {!hasSession && <Badge variant="outline">admin session required</Badge>}
          </div>
        </CardContent>
      </Card>
    </>
  );
}
