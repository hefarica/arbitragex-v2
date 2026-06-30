"use client";

import { useEffect, useState } from "react";
import { AlertCircleIcon, CheckCircle2Icon, RefreshCwIcon, ShieldCheckIcon, ShieldOffIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ClearAdminTokenButton } from "@/components/clear-admin-token";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PageHeader } from "@/components/page-header";
import { SourceMeta } from "@/components/SourceMeta";
import { adminTokenExpiresInMs, fmtRemaining, getAdminToken, setAdminToken } from "@/lib/admin-token";
import { getStatus, toggleKillswitch, type KillSwitchState } from "@/lib/api-client";

export default function KillSwitchPage() {
  const [state, setState] = useState<KillSwitchState | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [token, setToken] = useState<string>("");
  const [tokenExpiresIn, setTokenExpiresIn] = useState<number>(0);
  const [reason, setReason] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [lastOutcome, setLastOutcome] = useState<string | null>(null);
  const [fetchedAt, setFetchedAt] = useState<number | null>(null);

  useEffect(() => {
    const t = getAdminToken();
    if (t !== "__session_active__") {
      setToken(t);
    }
    setTokenExpiresIn(adminTokenExpiresInMs());
    void refresh();
  }, []);

  async function refresh() {
    setLoading(true);
    const r = await getStatus();
    setLoading(false);
    if (!r.ok) { setError(r.error); return; }
    setError(null);
    setState(r.data.killswitch);
    setFetchedAt(Date.now());
  }

  async function onToggle(next: boolean) {
    if (!token && getAdminToken() !== "__session_active__") { setError("admin token required"); return; }
    if (!reason.trim()) { setError("reason is required for the audit log"); return; }
    setBusy(true);
    let tokenToUse = token;
    if (!token && getAdminToken() === "__session_active__") {
      // If they have an active session but didn't enter a new token, let the proxy use the cookie.
      tokenToUse = "";
    } else {
      // V-AT-1: setAdminToken is now async — establishes httpOnly cookie session.
      const sessionOk = await setAdminToken(token);
      if (!sessionOk) { setBusy(false); setError("failed to establish admin session — check token"); return; }
    }
    setTokenExpiresIn(adminTokenExpiresInMs());
    // Token is now in the httpOnly cookie; the header is a fallback for this request.
    const r = await toggleKillswitch(next, reason.trim(), tokenToUse);
    setBusy(false);
    if (!r.ok) { setError(r.error); setLastOutcome(null); return; }
    setError(null);
    setState(r.data);
    setLastOutcome(`kill-switch set to ${next ? "ARMED" : "disabled"} at ${new Date().toLocaleTimeString()}`);
    setReason("");
  }

  return (
    <>
      <PageHeader
        title="Kill-switch"
        lede="Arming blocks every hot-path service from submitting new executions within ≤ 5 s. This is the first step of every incident runbook. Every toggle is recorded in the audit log."
      />

      {loading && !state && (
        <p className="mb-6 inline-flex items-center gap-2 text-sm text-muted-foreground" role="status" aria-busy>
          <RefreshCwIcon className="size-3.5 motion-safe:animate-spin" aria-hidden /> loading current kill-switch state…
        </p>
      )}

      {state && (
        <Alert variant={state.enabled ? "destructive" : "success"} className="mb-6">
          {state.enabled ? <ShieldOffIcon /> : <ShieldCheckIcon />}
          <AlertTitle className="text-base">
            {state.enabled ? "ARMED — executions blocked" : "DISABLED — executions permitted"}
          </AlertTitle>
          <AlertDescription>
            {state.reason && <div>reason: <em>{state.reason}</em></div>}
            <div className="text-xs">
              updated {new Date(state.updated_at).toLocaleString()}
              {state.triggered_by && <> · by <code className="font-mono">{state.triggered_by}</code></>}
            </div>
            <SourceMeta source="api-server" at={fetchedAt} className="mt-1.5" />
          </AlertDescription>
        </Alert>
      )}

      {error && (
        <Alert variant="destructive" className="mb-4">
          <AlertCircleIcon />
          <AlertTitle>error</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{error}</code></AlertDescription>
        </Alert>
      )}
      {lastOutcome && (
        <Alert variant="success" className="mb-4">
          <CheckCircle2Icon />
          <AlertDescription>{lastOutcome}</AlertDescription>
        </Alert>
      )}

      <Card className="max-w-xl">
        <CardHeader>
          <CardTitle>Toggle</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-5">
          <div className="flex flex-col gap-2">
            <Label htmlFor="ks-token">admin token <span className="text-muted-foreground">(stored in secure httpOnly session)</span></Label>
            <Input
              id="ks-token"
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              autoComplete="off"
            />
            <div className="flex items-center justify-between gap-3">
              <p className="text-xs text-muted-foreground">
                Persists in this browser only. {tokenExpiresIn > 0
                  ? <>Auto-clears in <strong>{fmtRemaining(tokenExpiresIn)}</strong>.</>
                  : <>Clear it before leaving a shared workstation.</>}
              </p>
              <ClearAdminTokenButton
                onCleared={() => { setToken(""); setTokenExpiresIn(0); }}
              />
            </div>
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="ks-reason">reason <span className="text-muted-foreground">(required — goes to audit_log)</span></Label>
            <Input
              id="ks-reason"
              type="text"
              value={reason}
              placeholder="e.g. investigating high revert rate"
              onChange={(e) => setReason(e.target.value)}
            />
          </div>
          <div className="flex flex-wrap gap-2">
            <Button type="button" variant="destructive" disabled={busy} onClick={() => onToggle(true)}>
              <ShieldOffIcon /> Arm kill-switch
            </Button>
            <Button type="button" variant="success" disabled={busy} onClick={() => onToggle(false)}>
              <ShieldCheckIcon /> Disable
            </Button>
            <Button type="button" variant="ghost" onClick={() => void refresh()}>
              <RefreshCwIcon /> Refresh
            </Button>
          </div>
        </CardContent>
      </Card>
    </>
  );
}
