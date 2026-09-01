/**
 * Phase 3 — Advanced features.
 *
 * R1 Mounted Snapshot Pattern. R8 Fail-Honest endpoint guard.
 */
"use client";

import * as React from "react"; // SSR-test classic JSX runtime (house convention, see RuntimePostureBar)
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
import { completeOnboardingPhase3, type OnboardingStatus } from "@/lib/api-client";

interface Props {
  initialSnapshot: OnboardingStatus;
}

const SIM_PROVIDERS = ["anvil", "tenderly"] as const;
type SimProvider = (typeof SIM_PROVIDERS)[number];

export function Phase3Client({ initialSnapshot }: Props) {
  const [status, setStatus] = useState<OnboardingStatus>(initialSnapshot);
  const [confirmedBy, setConfirmedBy] = useState("");
  const [simProvider, setSimProvider] = useState<SimProvider>("anvil");
  const [simEndpoint, setSimEndpoint] = useState("");
  const [tokenSafetyProvider, setTokenSafetyProvider] = useState("");
  const [tokenSafetyBaseUrl, setTokenSafetyBaseUrl] = useState("");
  const [tokenSafetyApiKeyRef, setTokenSafetyApiKeyRef] = useState("");
  const [scoringWeightsSignedOff, setScoringWeightsSignedOff] = useState(false);
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

  const alreadyDone = Boolean(status.phase_3_completed_at);

  const canSubmit =
    hasSession &&
    confirmedBy.trim().length >= 1 &&
    simEndpoint.trim().startsWith("http") &&
    tokenSafetyProvider.trim().length >= 1 &&
    tokenSafetyBaseUrl.trim().startsWith("http") &&
    tokenSafetyApiKeyRef.trim().length >= 1 &&
    scoringWeightsSignedOff &&
    !busy;

  async function submit() {
    setBusy(true);
    setResultOk(null);
    setResultErr(null);
    const r = await completeOnboardingPhase3(
      confirmedBy.trim(),
      simProvider,
      simEndpoint.trim(),
      {
        provider: tokenSafetyProvider.trim(),
        base_url: tokenSafetyBaseUrl.trim(),
        api_key_ref: tokenSafetyApiKeyRef.trim(),
      },
      scoringWeightsSignedOff,
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
      `Phase 3 completed at ${new Date(r.data.phase_3_completed_at).toLocaleString()} by ${r.data.phase_3_completed_by}.`,
    );
    setStatus((prev) => ({
      ...prev,
      phase_3_completed_at: r.data.phase_3_completed_at,
      phase_3_completed_by: r.data.phase_3_completed_by,
    }));
  }

  return (
    <>
      <PageHeader
        title="Phase 3 — Advanced features"
        lede="Choose a simulation provider, a token-safety provider, sign off on the scoring weights, import any initial blacklist."
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
          <AlertTitle>Phase 3 already completed</AlertTitle>
          <AlertDescription>
            Completed at {status.phase_3_completed_at && new Date(status.phase_3_completed_at).toLocaleString()} by{" "}
            <code>{status.phase_3_completed_by}</code>. Re-submitting updates the record.
          </AlertDescription>
        </Alert>
      )}

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">Simulation provider</CardTitle>
          <CardDescription>
            The EVM fork endpoint used by the pre-trade simulation pipeline. Anvil (local fork) is the default; Tenderly can replace it for cloud forks.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 max-w-xl">
          <div className="flex gap-4">
            {/* DAPP-SURFACE (2026-09-01): explicit htmlFor/id — a wrapping
                <label> alone leaves the radio unnamed to the accessible-name
                predicate (no id → label[for] can't resolve). */}
            {SIM_PROVIDERS.map((p) => {
              const radioId = `sim_provider_${p}`;
              return (
                <label key={p} htmlFor={radioId} className="flex items-center gap-2 cursor-pointer text-sm">
                  <input
                    id={radioId}
                    type="radio"
                    name="sim_provider"
                    value={p}
                    checked={simProvider === p}
                    onChange={() => setSimProvider(p)}
                    className="accent-primary"
                  />
                  <code>{p}</code>
                </label>
              );
            })}
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="sim_endpoint" className="text-xs text-muted-foreground">
              {simProvider === "anvil" ? "ANVIL_FORK_URL" : "Tenderly simulation URL"}
            </Label>
            <Input
              id="sim_endpoint"
              placeholder={
                simProvider === "anvil"
                  ? "http://anvil:8545 or https://rpc.tenderly.co/fork/..."
                  : "https://api.tenderly.co/api/v1/simulate"
              }
              value={simEndpoint}
              onChange={(e) => setSimEndpoint(e.target.value)}
              autoComplete="off"
            />
            <p className="text-xs text-muted-foreground">
              Stored as <code>sim_endpoint</code> in <code>onboarding_progress</code>.
            </p>
          </div>
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">Token safety provider</CardTitle>
          <CardDescription>
            External API used to evaluate token risk scores. The API key is stored as a Vault path reference — the plaintext key never leaves Vault.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 max-w-xl">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="ts_provider" className="text-xs text-muted-foreground">Provider name</Label>
            <Input
              id="ts_provider"
              placeholder="gopluslabs or honeypot.is"
              value={tokenSafetyProvider}
              onChange={(e) => setTokenSafetyProvider(e.target.value)}
              autoComplete="off"
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="ts_base_url" className="text-xs text-muted-foreground">Base URL</Label>
            <Input
              id="ts_base_url"
              placeholder="https://api.gopluslabs.io/api/v1"
              value={tokenSafetyBaseUrl}
              onChange={(e) => setTokenSafetyBaseUrl(e.target.value)}
              autoComplete="off"
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="ts_api_key_ref" className="text-xs text-muted-foreground">Vault path (api_key_ref)</Label>
            <Input
              id="ts_api_key_ref"
              placeholder="secret/token-safety/api-key"
              value={tokenSafetyApiKeyRef}
              onChange={(e) => setTokenSafetyApiKeyRef(e.target.value)}
              autoComplete="off"
            />
            <p className="text-xs text-muted-foreground">
              The plaintext API key must already exist at this Vault path. Only the path is recorded here.
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
            <Label htmlFor="confirmed_by_3">Operator identity</Label>
            <Input
              id="confirmed_by_3"
              placeholder="alice@arbx or handle"
              value={confirmedBy}
              onChange={(e) => setConfirmedBy(e.target.value)}
              autoComplete="off"
            />
          </div>

          <label htmlFor="weights_ack" className="flex items-start gap-3 cursor-pointer">
            <input
              id="weights_ack"
              type="checkbox"
              className="mt-1 size-4 rounded border-border accent-primary"
              checked={scoringWeightsSignedOff}
              onChange={(e) => setScoringWeightsSignedOff(e.target.checked)}
            />
            <span className="text-sm leading-relaxed">
              I have reviewed the scoring weights in <code>configs/app.toml</code> and sign off that they reflect the intended risk tolerance for this deployment.
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
              <AlertTitle>Phase 3 complete</AlertTitle>
              <AlertDescription>{resultOk}</AlertDescription>
            </Alert>
          )}

          <div className="flex items-center gap-3 pt-2 border-t">
            <Button type="button" onClick={submit} disabled={!canSubmit}>
              {busy ? "Saving…" : alreadyDone ? "Re-record" : "Record phase 3 complete"}
            </Button>
            {!hasSession && <Badge variant="outline">admin session required</Badge>}
            {!scoringWeightsSignedOff && hasSession && (
              <Badge variant="outline">confirm scoring weights sign-off</Badge>
            )}
          </div>
        </CardContent>
      </Card>
    </>
  );
}
