"use client";
import { getApiBaseUrl } from "@/lib/api-client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { AlertCircleIcon, ArrowLeftIcon, CheckCircle2Icon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ClearAdminTokenButton } from "@/components/clear-admin-token";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PageHeader } from "@/components/page-header";
import { adminTokenExpiresInMs, fmtRemaining, getAdminToken, setAdminToken } from "@/lib/admin-token";
import {
  completeOnboardingPhase1,
  getOnboardingStatus,
  type OnboardingStatus,
} from "@/lib/api-client";

export default function Phase1InitPage() {
  const [status, setStatus] = useState<OnboardingStatus | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);
  const [adminToken, setAdminTokenState] = useState("");
  const [tokenExpiresIn, setTokenExpiresIn] = useState(0);
  const [confirmedBy, setConfirmedBy] = useState("");
  const [notes, setNotes] = useState("");
  const [ackAdminToken, setAckAdminToken] = useState(false);
  const [ackEdgeToken, setAckEdgeToken] = useState(false);
  const [ackJwt, setAckJwt] = useState(false);
  const [ackGrafana, setAckGrafana] = useState(false);
  const [ackVault, setAckVault] = useState(false);
  const [busy, setBusy] = useState(false);
  const [resultOk, setResultOk] = useState<string | null>(null);
  const [resultErr, setResultErr] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      const r = await getOnboardingStatus();
      if (!r.ok) { setLoadErr(r.error); return; }
      setStatus(r.data);
    })();
    setAdminTokenState(getAdminToken());
    setTokenExpiresIn(adminTokenExpiresInMs());
  }, []);

  const allAcked = ackAdminToken && ackEdgeToken && ackJwt && ackGrafana && ackVault;
  const canSubmit = allAcked && adminToken.trim().length >= 16 && confirmedBy.trim().length >= 1 && !busy;

  async function submit() {
    setBusy(true);
    setResultOk(null);
    setResultErr(null);
    setAdminToken(adminToken);
    setTokenExpiresIn(adminTokenExpiresInMs());
    const r = await completeOnboardingPhase1(confirmedBy.trim(), true, notes.trim(), adminToken);
    setBusy(false);
    if (!r.ok) { setResultErr(r.error); return; }
    setResultOk(
      `Phase 1 completed at ${new Date(r.data.phase_1_completed_at).toLocaleString()} by ${r.data.phase_1_completed_by}.`,
    );
    const fresh = await getOnboardingStatus();
    if (fresh.ok) setStatus(fresh.data);
  }

  const alreadyDone = Boolean(status?.phase_1_completed_at);

  return (
    <>
      <PageHeader
        title="Phase 1 — Install & boot"
        lede="Confirm the platform has been installed with real, operator-generated secrets. This page never generates tokens for you; it records that you already provisioned them via your own CLI / secret manager."
        actions={
          <Link href="/onboarding" className="inline-flex items-center gap-2 rounded-md border px-3 py-1.5 text-sm">
            <ArrowLeftIcon className="size-4" /> Back
          </Link>
        }
      />

      {loadErr && (
        <Alert variant="destructive" className="mb-6">
          <AlertCircleIcon />
          <AlertTitle>could not load onboarding status</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{loadErr}</code></AlertDescription>
        </Alert>
      )}

      {alreadyDone && (
        <Alert variant="success" className="mb-6">
          <CheckCircle2Icon />
          <AlertTitle>Phase 1 already completed</AlertTitle>
          <AlertDescription>
            Completed at{" "}
            {status?.phase_1_completed_at && new Date(status.phase_1_completed_at).toLocaleString()}
            {" "}by <code>{status?.phase_1_completed_by}</code>. Re-submitting this form will update the timestamp and
            create a new audit entry. Use that only if you rotated secrets.
          </AlertDescription>
        </Alert>
      )}

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">What phase 1 covers</CardTitle>
          <CardDescription>
            The operator provisions the platform's own secrets — admin token, edge token, JWT secret, Grafana admin
            password — and unseals Vault. The application never generates these for you because the doctrine requires
            that productive values originate from a source you control.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3 text-sm">
          <SecretCheck
            id="ack-admin"
            checked={ackAdminToken}
            onChange={setAckAdminToken}
            label={<><code>ARBX_ADMIN_TOKEN</code> generated (≥ 32 bytes random) and written to Vault path <code>tokens/admin</code>.</>}
          />
          <SecretCheck
            id="ack-edge"
            checked={ackEdgeToken}
            onChange={setAckEdgeToken}
            label={<><code>ARBX_EDGE_TOKEN</code> generated (≥ 32 bytes random) and written to Vault path <code>tokens/edge</code>.</>}
          />
          <SecretCheck
            id="ack-jwt"
            checked={ackJwt}
            onChange={setAckJwt}
            label={<><code>JWT_SECRET</code> generated (≥ 64 bytes random base64) and written to Vault path <code>tokens/jwt</code>.</>}
          />
          <SecretCheck
            id="ack-grafana"
            checked={ackGrafana}
            onChange={setAckGrafana}
            label={<><code>GRAFANA_ADMIN_PASSWORD</code> chosen (≥ 16 chars) and written to Vault path <code>grafana/admin_password</code>.</>}
          />
          <SecretCheck
            id="ack-vault"
            checked={ackVault}
            onChange={setAckVault}
            label={<>Vault is initialised and unsealed; <code>vault status</code> reports <code>Sealed: false</code> from each node.</>}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Record completion</CardTitle>
          <CardDescription>
            Writing this row goes to <code>onboarding_progress</code> and <code>audit_log</code>. It does NOT store
            any of the secrets themselves — only the admission that they exist.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 max-w-xl">
          <div className="flex flex-col gap-2">
            <Label htmlFor="confirmed_by">Operator identity</Label>
            <Input
              id="confirmed_by"
              placeholder="alice@arbx or handle"
              value={confirmedBy}
              onChange={(e) => setConfirmedBy(e.target.value)}
              autoComplete="off"
            />
            <p className="text-xs text-muted-foreground">Recorded in <code>phase_1_completed_by</code> and <code>audit_log.actor</code>.</p>
          </div>

          <div className="flex flex-col gap-2">
            <Label htmlFor="admin_token">Admin token (for auth)</Label>
            <Input
              id="admin_token"
              type="password"
              placeholder="paste ARBX_ADMIN_TOKEN"
              value={adminToken}
              onChange={(e) => setAdminTokenState(e.target.value)}
              autoComplete="off"
            />
            <div className="flex items-center justify-between gap-3">
              <p className="text-xs text-muted-foreground">
                Stored locally in <code>localStorage</code> for this browser only. {tokenExpiresIn > 0
                  ? <>Auto-clears in <strong>{fmtRemaining(tokenExpiresIn)}</strong>.</>
                  : <>Never sent to a third party.</>}
              </p>
              <ClearAdminTokenButton
                onCleared={() => { setAdminTokenState(""); setTokenExpiresIn(0); }}
              />
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <Label htmlFor="notes">Notes (optional)</Label>
            <Input
              id="notes"
              placeholder="e.g. 'rotated after incident 2026-04-19'"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
            />
          </div>

          {resultErr && (
            <Alert variant="destructive">
              <AlertCircleIcon />
              <AlertTitle>could not record completion</AlertTitle>
              <AlertDescription><code className="font-mono text-xs">{resultErr}</code></AlertDescription>
            </Alert>
          )}
          {resultOk && (
            <Alert variant="success">
              <CheckCircle2Icon />
              <AlertTitle>Phase 1 complete</AlertTitle>
              <AlertDescription>{resultOk}</AlertDescription>
            </Alert>
          )}

          <div className="flex items-center gap-3">
            <Button type="button" onClick={submit} disabled={!canSubmit}>
              {alreadyDone ? "Re-record (after rotation)" : "Record phase 1 complete"}
            </Button>
            {!allAcked && <Badge variant="outline">check all five confirmations</Badge>}
          </div>
        </CardContent>
      </Card>
    </>
  );
}

function SecretCheck({
  id,
  checked,
  onChange,
  label,
}: {
  id: string;
  checked: boolean;
  onChange: (next: boolean) => void;
  label: React.ReactNode;
}) {
  return (
    <label htmlFor={id} className="flex items-start gap-3 cursor-pointer">
      <input
        id={id}
        type="checkbox"
        className="mt-1 size-4 rounded border-border accent-primary"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="text-sm leading-relaxed">{label}</span>
    </label>
  );
}
