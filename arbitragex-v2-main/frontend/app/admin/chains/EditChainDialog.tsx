/**
 * B1.b — EditChainDialog
 *
 * Modal form to edit a chains_runtime row via PUT /api/admin/chains/:chain_id.
 *
 * chain_id is immutable per backend schema; rendered read-only with the
 * "Chain ID" label and disabled input. All other runtime-relevant fields
 * recompute the backend config_hash → if and only if the hash changes does
 * the searcher hot-reload that chain (B1.c).
 *
 * R8: notes is metadata-only and does not participate in the hash.
 */
"use client";

import * as React from "react";
import { useCallback, useMemo, useState } from "react";
import { AlertCircle, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { updateAdminChain } from "@/lib/api-client";
import { AdminChainUpdateBodySchema, type AdminChainRow, type AdminChainUpdateBody } from "@/lib/schemas";

interface Props {
  row: AdminChainRow;
  adminToken: string;
  actor: string;
  onClose: () => void;
  onSaved: (row: AdminChainRow) => void;
}

export function EditChainDialog({ row, adminToken, actor, onClose, onSaved }: Props) {
  const [name, setName] = useState(row.name);
  const [rpcHttpUrl, setRpcHttpUrl] = useState(row.rpc_http_url);
  const [rpcWsUrl, setRpcWsUrl] = useState(row.rpc_ws_url ?? "");
  const [nativeCurrency, setNativeCurrency] = useState(row.native_currency);
  const [blockTimeMsStr, setBlockTimeMsStr] = useState(String(row.block_time_ms));
  const [enabled, setEnabled] = useState(row.enabled);
  const [notes, setNotes] = useState(row.notes ?? "");
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);

  const candidate: AdminChainUpdateBody | null = useMemo(() => {
    const blockTimeMs = Number(blockTimeMsStr);
    if (!Number.isInteger(blockTimeMs) || blockTimeMs <= 0) return null;
    const body: AdminChainUpdateBody = {
      name: name.trim(),
      rpc_http_url: rpcHttpUrl.trim(),
      ...(rpcWsUrl.trim() ? { rpc_ws_url: rpcWsUrl.trim() } : {}),
      ...(nativeCurrency.trim() ? { native_currency: nativeCurrency.trim() } : {}),
      block_time_ms: blockTimeMs,
      enabled,
      ...(notes.trim() ? { notes: notes.trim() } : {}),
    };
    return AdminChainUpdateBodySchema.safeParse(body).success ? body : null;
  }, [name, rpcHttpUrl, rpcWsUrl, nativeCurrency, blockTimeMsStr, enabled, notes]);

  const canSubmit = candidate !== null && adminToken !== "" && !submitting;

  const submit = useCallback(async () => {
    if (!candidate) return;
    setServerError(null);
    setSubmitting(true);
    const res = await updateAdminChain(row.chain_id, candidate, adminToken, actor);
    setSubmitting(false);
    if (!res.ok) {
      setServerError(res.error);
      return;
    }
    onSaved(res.data);
  }, [candidate, row.chain_id, adminToken, actor, onSaved]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="edit-chain-title"
    >
      <div className="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-xl max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between px-5 py-3 border-b border-border">
          <h2 id="edit-chain-title" className="text-lg font-bold">
            Edit chain · <span className="font-mono">{row.chain_id}</span>
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
              <Label htmlFor="ec-chain-id" className="text-xs uppercase tracking-wide">
                Chain ID (immutable)
              </Label>
              <Input id="ec-chain-id" value={row.chain_id} readOnly disabled className="font-mono" />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="ec-name" className="text-xs uppercase tracking-wide">
                Name <span className="text-destructive">*</span>
              </Label>
              <Input
                id="ec-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                maxLength={64}
              />
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="ec-rpc-http" className="text-xs uppercase tracking-wide">
              RPC HTTP URL <span className="text-destructive">*</span>
            </Label>
            <Input
              id="ec-rpc-http"
              type="url"
              value={rpcHttpUrl}
              onChange={(e) => setRpcHttpUrl(e.target.value)}
              className="font-mono text-xs"
            />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="ec-rpc-ws" className="text-xs uppercase tracking-wide">
              RPC WebSocket URL
            </Label>
            <Input
              id="ec-rpc-ws"
              type="url"
              value={rpcWsUrl}
              onChange={(e) => setRpcWsUrl(e.target.value)}
              placeholder="wss://… (optional)"
              className="font-mono text-xs"
            />
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="space-y-1.5">
              <Label htmlFor="ec-native" className="text-xs uppercase tracking-wide">
                Native currency
              </Label>
              <Input
                id="ec-native"
                value={nativeCurrency}
                onChange={(e) => setNativeCurrency(e.target.value)}
                maxLength={16}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="ec-block-time" className="text-xs uppercase tracking-wide">
                Block time (ms)
              </Label>
              <Input
                id="ec-block-time"
                type="number"
                inputMode="numeric"
                min={100}
                max={600000}
                value={blockTimeMsStr}
                onChange={(e) => setBlockTimeMsStr(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs uppercase tracking-wide">Enabled</Label>
              <div className="h-9 flex items-center gap-2 px-3 rounded-md border border-input bg-transparent">
                <input
                  id="ec-enabled"
                  type="checkbox"
                  aria-label="Enabled"
                  title="Toggle whether this chain is enabled in chains_runtime"
                  checked={enabled}
                  onChange={(e) => setEnabled(e.target.checked)}
                  className="size-4"
                />
                <Label htmlFor="ec-enabled" className="text-xs font-normal cursor-pointer">
                  {enabled ? "Active" : "Disabled"}
                </Label>
              </div>
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="ec-notes" className="text-xs uppercase tracking-wide">
              Notes (metadata only, not hashed)
            </Label>
            <Input
              id="ec-notes"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              maxLength={1024}
            />
          </div>

          {row.config_hash && (
            <p className="text-[10px] text-muted-foreground font-mono break-all">
              current config_hash: {row.config_hash}
            </p>
          )}

          {!adminToken && (
            <div className="flex items-start gap-2 rounded-lg border border-warning/40 bg-warning/10 p-3 text-xs text-warning">
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span>No admin session — unlock at /killswitch.</span>
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
            {submitting ? "Saving…" : "Save changes"}
          </Button>
        </div>
      </div>
    </div>
  );
}
