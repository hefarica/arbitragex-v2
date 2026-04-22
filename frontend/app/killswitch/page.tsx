"use client";

import { useEffect, useState } from "react";
import { AlertCircleIcon, CheckCircle2Icon, RefreshCwIcon, ShieldCheckIcon, ShieldOffIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PageHeader } from "@/components/page-header";
import { getStatus, toggleKillswitch, type KillSwitchState } from "@/lib/api-client";

export default function KillSwitchPage() {
  const [state, setState] = useState<KillSwitchState | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [token, setToken] = useState<string>("");
  const [reason, setReason] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [lastOutcome, setLastOutcome] = useState<string | null>(null);

  useEffect(() => {
    const t = typeof window !== "undefined" ? (window.localStorage.getItem("arbx_admin_token") ?? "") : "";
    setToken(t);
    void refresh();
  }, []);

  async function refresh() {
    setLoading(true);
    const r = await getStatus();
    setLoading(false);
    if (!r.ok) { setError(r.error); return; }
    setError(null);
    setState(r.data.killswitch);
  }

  async function onToggle(next: boolean) {
    if (!token) { setError("admin token required"); return; }
    if (!reason.trim()) { setError("reason is required for the audit log"); return; }
    setBusy(true);
    window.localStorage.setItem("arbx_admin_token", token);
    const r = await toggleKillswitch(next, reason.trim(), token);
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

      {loading && <p className="text-sm text-muted-foreground">loading current state…</p>}

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
            <Label htmlFor="ks-token">admin token <span className="text-muted-foreground">(stored in localStorage)</span></Label>
            <Input
              id="ks-token"
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              autoComplete="off"
            />
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
