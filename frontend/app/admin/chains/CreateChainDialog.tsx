/**
 * B1.b — CreateChainDialog
 *
 * Modal form to create a new chains_runtime row via POST /api/admin/chains.
 *
 * Doctrine:
 *   - Zero hidden defaults. native_currency, block_time_ms, enabled are
 *     shown explicitly with placeholders. The backend Zod schema accepts
 *     them as optional but we make the operator pick or accept the value.
 *   - Client-side validation mirrors AdminChainCreateBodySchema 1:1.
 *   - On 4xx/5xx, render the backend error verbatim — never swallow.
 *   - R8: never silently send a value the operator did not see.
 */
"use client";

import * as React from "react";
import { useCallback, useMemo, useState } from "react";
import { AlertCircle, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { createAdminChain } from "@/lib/api-client";
import { AdminChainCreateBodySchema, type AdminChainRow, type AdminChainCreateBody } from "@/lib/schemas";

interface Props {
  adminToken: string;
  actor: string;
  onClose: () => void;
  onCreated: (row: AdminChainRow) => void;
}

const DEFAULT_BLOCK_TIME_MS = 12000;

export function CreateChainDialog({ adminToken, actor, onClose, onCreated }: Props) {
  const [chainIdStr, setChainIdStr] = useState("");
  const [name, setName] = useState("");
  const [rpcHttpUrl, setRpcHttpUrl] = useState("");
  const [rpcWsUrl, setRpcWsUrl] = useState("");
  const [nativeCurrency, setNativeCurrency] = useState("ETH");
  const [blockTimeMsStr, setBlockTimeMsStr] = useState(String(DEFAULT_BLOCK_TIME_MS));
  const [enabled, setEnabled] = useState(true);
  const [notes, setNotes] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);

  // Mirror the backend Zod schema 1:1 — if the body fails this parse,
  // we never hit the network. The same constraints also live in
  // AdminChainCreateBodySchema and admin-chains.ts on the server.
  const candidate: AdminChainCreateBody | null = useMemo(() => {
    const chainId = Number(chainIdStr);
    const blockTimeMs = Number(blockTimeMsStr);
    if (!Number.isInteger(chainId) || chainId <= 0) return null;
    if (!Number.isInteger(blockTimeMs) || blockTimeMs <= 0) return null;
    const body: AdminChainCreateBody = {
      chain_id: chainId,
      name: name.trim(),
      rpc_http_url: rpcHttpUrl.trim(),
      ...(rpcWsUrl.trim() ? { rpc_ws_url: rpcWsUrl.trim() } : {}),
      ...(nativeCurrency.trim() ? { native_currency: nativeCurrency.trim() } : {}),
      block_time_ms: blockTimeMs,
      enabled,
      ...(notes.trim() ? { notes: notes.trim() } : {}),
    };
    return AdminChainCreateBodySchema.safeParse(body).success ? body : null;
  }, [chainIdStr, name, rpcHttpUrl, rpcWsUrl, nativeCurrency, blockTimeMsStr, enabled, notes]);

  const canSubmit = candidate !== null && adminToken !== "" && !submitting;

  const submit = useCallback(async () => {
    if (!candidate) return;
    setServerError(null);
    setSubmitting(true);
    const res = await createAdminChain(candidate, adminToken, actor);
    setSubmitting(false);
    if (!res.ok) {
      setServerError(res.error);
      return;
    }
    onCreated(res.data);
  }, [candidate, adminToken, actor, onCreated]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="create-chain-title"
    >
      <div className="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-xl max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between px-5 py-3 border-b border-border">
          <h2 id="create-chain-title" className="text-lg font-bold">
            Register chain
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground"
            aria-label="Close"
          >
            <X className="size-4" />
          </button>
        </div>

        <div className="p-5 space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label htmlFor="cc-chain-id" className="text-xs uppercase tracking-wide">
                Chain ID <span className="text-destructive">*</span>
              </Label>
              <Input
                id="cc-chain-id"
                type="number"
                inputMode="numeric"
                min={1}
                value={chainIdStr}
                onChange={(e) => setChainIdStr(e.target.value)}
                placeholder="e.g. 1"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="cc-name" className="text-xs uppercase tracking-wide">
                Name <span className="text-destructive">*</span>
              </Label>
              <Input
                id="cc-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="ethereum"
                maxLength={64}
              />
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="cc-rpc-http" className="text-xs uppercase tracking-wide">
              RPC HTTP URL <span className="text-destructive">*</span>
            </Label>
            <Input
              id="cc-rpc-http"
              type="url"
              value={rpcHttpUrl}
              onChange={(e) => setRpcHttpUrl(e.target.value)}
              placeholder="https://eth-mainnet.g.alchemy.com/v2/…"
              className="font-mono text-xs"
            />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="cc-rpc-ws" className="text-xs uppercase tracking-wide">
              RPC WebSocket URL
            </Label>
            <Input
              id="cc-rpc-ws"
              type="url"
              value={rpcWsUrl}
              onChange={(e) => setRpcWsUrl(e.target.value)}
              placeholder="wss://eth-mainnet.g.alchemy.com/v2/… (optional)"
              className="font-mono text-xs"
            />
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="space-y-1.5">
              <Label htmlFor="cc-native" className="text-xs uppercase tracking-wide">
                Native currency
              </Label>
              <Input
                id="cc-native"
                value={nativeCurrency}
                onChange={(e) => setNativeCurrency(e.target.value)}
                placeholder="ETH"
                maxLength={16}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="cc-block-time" className="text-xs uppercase tracking-wide">
                Block time (ms)
              </Label>
              <Input
                id="cc-block-time"
                type="number"
                inputMode="numeric"
                min={100}
                max={600000}
                value={blockTimeMsStr}
                onChange={(e) => setBlockTimeMsStr(e.target.value)}
                placeholder={String(DEFAULT_BLOCK_TIME_MS)}
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs uppercase tracking-wide">Enabled</Label>
              <div className="h-9 flex items-center gap-2 px-3 rounded-md border border-input bg-transparent">
                <input
                  id="cc-enabled"
                  type="checkbox"
                  aria-label="Enabled on save"
                  title="Whether this chain starts enabled (the searcher will pick it up on next reload)"
                  checked={enabled}
                  onChange={(e) => setEnabled(e.target.checked)}
                  className="size-4"
                />
                <Label htmlFor="cc-enabled" className="text-xs font-normal cursor-pointer">
                  {enabled ? "Active on save" : "Disabled on save"}
                </Label>
              </div>
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="cc-notes" className="text-xs uppercase tracking-wide">
              Notes
            </Label>
            <Input
              id="cc-notes"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder="Operator notes (visible only here, not hashed for reload)"
              maxLength={1024}
            />
          </div>

          {!adminToken && (
            <div className="flex items-start gap-2 rounded-lg border border-warning/40 bg-warning/10 p-3 text-xs text-warning">
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span>No admin session — unlock the operator session at /killswitch before creating chains.</span>
            </div>
          )}

          {serverError && (
            <div className="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span className="font-mono break-all">{serverError}</span>
            </div>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border bg-muted/30">
          <Button type="button" variant="ghost" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button type="button" onClick={submit} disabled={!canSubmit}>
            {submitting ? "Saving…" : "Create"}
          </Button>
        </div>
      </div>
    </div>
  );
}
