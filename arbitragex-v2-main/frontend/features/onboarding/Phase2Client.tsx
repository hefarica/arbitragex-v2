/**
 * Phase 2 — Connect primary services.
 *
 * R1 Mounted Snapshot Pattern: receives initialSnapshot from the Server
 * Component. All non-deterministic reads (hasAdminSession, DOM) happen
 * exclusively inside useEffect.
 *
 * R8 Fail-Honest: If the endpoint returns 404/501 the error is surfaced
 * verbatim as "endpoint not implemented" rather than a broken UI state.
 */
"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { AlertCircleIcon, ArrowLeftIcon, CheckCircle2Icon, PlusIcon, TrashIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PageHeader } from "@/components/page-header";
import { hasAdminSession } from "@/lib/admin-token";
import { completeOnboardingPhase2, type OnboardingStatus } from "@/lib/api-client";

interface RpcEntry {
  chain_id: string;
  http: string;
  ws: string;
}

interface Props {
  initialSnapshot: OnboardingStatus;
}

export function Phase2Client({ initialSnapshot }: Props) {
  const [status, setStatus] = useState<OnboardingStatus>(initialSnapshot);
  const [confirmedBy, setConfirmedBy] = useState("");
  const [rpcs, setRpcs] = useState<RpcEntry[]>([{ chain_id: "1", http: "", ws: "" }]);
  const [slackWebhook, setSlackWebhook] = useState("");
  const [rpcProbeOk, setRpcProbeOk] = useState(false);
  const [hasSession, setHasSession] = useState(false);
  const [busy, setBusy] = useState(false);
  const [resultOk, setResultOk] = useState<string | null>(null);
  const [resultErr, setResultErr] = useState<string | null>(null);

  // R1: all non-deterministic reads deferred to useEffect — never during SSR render.
  useEffect(() => {
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  const alreadyDone = Boolean(status.phase_2_completed_at);

  const allRpcsValid = rpcs.every(
    (r) => r.chain_id.trim() !== "" && r.http.trim().startsWith("http") && r.ws.trim().startsWith("ws"),
  );
  const canSubmit =
    hasSession &&
    confirmedBy.trim().length >= 1 &&
    rpcs.length >= 1 &&
    allRpcsValid &&
    rpcProbeOk &&
    !busy;

  function addRpc() {
    setRpcs((prev) => [...prev, { chain_id: "", http: "", ws: "" }]);
  }

  function removeRpc(i: number) {
    setRpcs((prev) => prev.filter((_, idx) => idx !== i));
  }

  function updateRpc(i: number, field: keyof RpcEntry, value: string) {
    setRpcs((prev) => prev.map((r, idx) => (idx === i ? { ...r, [field]: value } : r)));
  }

  async function submit() {
    setBusy(true);
    setResultOk(null);
    setResultErr(null);
    const parsedRpcs = rpcs.map((r) => ({
      chain_id: Number(r.chain_id),
      http: r.http.trim(),
      ws: r.ws.trim(),
    }));
    const r = await completeOnboardingPhase2(
      confirmedBy.trim(),
      parsedRpcs,
      slackWebhook.trim() || null,
      rpcProbeOk,
    );
    setBusy(false);
    if (!r.ok) {
      // R8 fail-honest: surface 404/501 as "endpoint not implemented" badge.
      const isNotImpl = r.error.includes("404") || r.error.includes("501") || r.error.includes("not implemented");
      setResultErr(
        isNotImpl
          ? "endpoint not yet implemented on this deployment — use the curl command below"
          : r.error,
      );
      return;
    }
    setResultOk(
      `Phase 2 completed at ${new Date(r.data.phase_2_completed_at).toLocaleString()} by ${r.data.phase_2_completed_by}. RPC probe: ${r.data.phase_2_rpc_probe_ok ? "ok" : "not ok"}.`,
    );
    setStatus((prev) => ({
      ...prev,
      phase_2_completed_at: r.data.phase_2_completed_at,
      phase_2_completed_by: r.data.phase_2_completed_by,
      phase_2_rpc_probe_ok: r.data.phase_2_rpc_probe_ok,
    }));
  }

  return (
    <>
      <PageHeader
        title="Phase 2 — Connect primary services"
        lede="Attach real RPC endpoints per enabled chain so searcher-rs stops being idle. Optionally add a Slack webhook for warning-level alerts."
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
          <AlertTitle>Phase 2 already completed</AlertTitle>
          <AlertDescription>
            Completed at {status.phase_2_completed_at && new Date(status.phase_2_completed_at).toLocaleString()} by{" "}
            <code>{status.phase_2_completed_by}</code>. RPC probe:{" "}
            {status.phase_2_rpc_probe_ok ? "ok" : "not ok"}. Re-submitting updates the timestamp and
            creates a new audit entry.
          </AlertDescription>
        </Alert>
      )}

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">Solicits</CardTitle>
          <CardDescription>Values provisioned from your own secret manager. Nothing here is generated by the application.</CardDescription>
        </CardHeader>
        <CardContent>
          <ul className="flex flex-wrap gap-2 text-xs">
            {["RPC_HTTP_<chain_id>", "RPC_WS_<chain_id>", "SLACK_WEBHOOK_URL (optional)"].map((x) => (
              <li key={x}>
                <code className="rounded bg-muted px-2 py-0.5 font-mono">{x}</code>
              </li>
            ))}
          </ul>
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">RPC endpoints</CardTitle>
          <CardDescription>
            One row per chain. <code>chain_id</code> must be a numeric EVM chain ID (e.g. 1 = Ethereum mainnet, 42161 = Arbitrum).
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {rpcs.map((rpc, i) => (
            <div key={i} className="grid grid-cols-[80px_1fr_1fr_auto] gap-2 items-end">
              <div className="flex flex-col gap-1.5">
                <Label className="text-xs text-muted-foreground">chain_id</Label>
                <Input
                  type="number"
                  placeholder="1"
                  value={rpc.chain_id}
                  onChange={(e) => updateRpc(i, "chain_id", e.target.value)}
                  min={1}
                />
              </div>
              <div className="flex flex-col gap-1.5">
                <Label className="text-xs text-muted-foreground">HTTP RPC</Label>
                <Input
                  placeholder="https://eth-mainnet.g.alchemy.com/v2/..."
                  value={rpc.http}
                  onChange={(e) => updateRpc(i, "http", e.target.value)}
                  autoComplete="off"
                />
              </div>
              <div className="flex flex-col gap-1.5">
                <Label className="text-xs text-muted-foreground">WebSocket RPC</Label>
                <Input
                  placeholder="wss://eth-mainnet.g.alchemy.com/v2/..."
                  value={rpc.ws}
                  onChange={(e) => updateRpc(i, "ws", e.target.value)}
                  autoComplete="off"
                />
              </div>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="mb-0.5"
                onClick={() => removeRpc(i)}
                disabled={rpcs.length <= 1}
                title="Remove this RPC"
              >
                <TrashIcon className="size-4" />
              </Button>
            </div>
          ))}
          <Button type="button" variant="outline" size="sm" className="w-max" onClick={addRpc}>
            <PlusIcon className="size-4 mr-1" /> Add chain
          </Button>
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">Optional: Slack webhook</CardTitle>
          <CardDescription>Warning-level alerts will be posted here. Leave blank to skip.</CardDescription>
        </CardHeader>
        <CardContent className="max-w-xl">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="slack_webhook" className="text-xs text-muted-foreground">
              SLACK_WEBHOOK_URL
            </Label>
            <Input
              id="slack_webhook"
              placeholder="https://hooks.slack.com/services/..."
              value={slackWebhook}
              onChange={(e) => setSlackWebhook(e.target.value)}
              autoComplete="off"
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Record completion</CardTitle>
          <CardDescription>
            Writes to <code>onboarding_progress</code> and <code>audit_log</code>. Does not store RPC keys.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 max-w-xl">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="confirmed_by_2">Operator identity</Label>
            <Input
              id="confirmed_by_2"
              placeholder="alice@arbx or handle"
              value={confirmedBy}
              onChange={(e) => setConfirmedBy(e.target.value)}
              autoComplete="off"
            />
          </div>

          <label htmlFor="rpc_probe_ok_2" className="flex items-start gap-3 cursor-pointer">
            <input
              id="rpc_probe_ok_2"
              type="checkbox"
              className="mt-1 size-4 rounded border-border accent-primary"
              checked={rpcProbeOk}
              onChange={(e) => setRpcProbeOk(e.target.checked)}
            />
            <span className="text-sm leading-relaxed">
              I have probed each RPC with <code>cast block-number</code> (or equivalent) and confirmed it returns the current block number.
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
              <AlertTitle>Phase 2 complete</AlertTitle>
              <AlertDescription>{resultOk}</AlertDescription>
            </Alert>
          )}

          <div className="flex items-center gap-3 pt-2 border-t">
            <Button type="button" onClick={submit} disabled={!canSubmit}>
              {busy ? "Saving…" : alreadyDone ? "Re-record (after rotation)" : "Record phase 2 complete"}
            </Button>
            {!hasSession && <Badge variant="outline">admin session required</Badge>}
            {!rpcProbeOk && hasSession && <Badge variant="outline">confirm RPC probe</Badge>}
          </div>
        </CardContent>
      </Card>
    </>
  );
}
