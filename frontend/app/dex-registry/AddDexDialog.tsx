"use client";

/**
 * FE-CRIT-06 — AddDexDialog
 *
 * Modal form to register a new DEX via POST /api/v1/dexes (createDex).
 *
 * Doctrine:
 *   - Zero hidden defaults. name, protocol_type and at least one
 *     {chain_id, address} factory are shown explicitly. The operator picks
 *     every value; nothing is fabricated (RULE 00).
 *   - Client-side validation mirrors CreateDexBody 1:1 — if the body is
 *     invalid we never hit the network and the submit stays disabled.
 *   - Fail-honest: a 404 from the mutation surfaces as `missing_backend_contract`
 *     (the backend route is not wired) — NEVER a fake success toast. Any other
 *     4xx/5xx surfaces the backend error verbatim.
 *   - Mirrors the hand-rolled fixed-overlay pattern used by CreateChainDialog
 *     under admin/chains for visual + behavioural consistency.
 */

import * as React from "react";
import { useCallback, useMemo, useState } from "react";
import { AlertCircle, Plus, Trash2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { createDex, type CreateDexBody, type CreateDexFactory, type DexRow } from "@/lib/api/dexes";

// The mutateJson helper surfaces a 404 as this exact string; we map it to a
// distinct fail-honest state so the operator sees "backend not wired" rather
// than a generic error.
const NOT_IMPLEMENTED_PREFIX = "API endpoint not yet implemented";

interface FactoryDraft {
  chainIdStr: string;
  address: string;
}

interface Props {
  edgeUrl: string;
  adminToken: string;
  protocols: readonly string[];
  /** Chains the operator may pick from (id + label). May be empty. */
  chainCatalog: Array<{ id: number; label: string }>;
  onClose: () => void;
  onCreated: (row: DexRow) => void;
}

function isAddressLike(v: string): boolean {
  return /^0x[a-fA-F0-9]{40}$/.test(v.trim());
}

export function AddDexDialog({
  edgeUrl,
  adminToken,
  protocols,
  chainCatalog,
  onClose,
  onCreated,
}: Props) {
  const [name, setName] = useState("");
  const [protocolType, setProtocolType] = useState<string>(protocols[0] ?? "");
  const [factories, setFactories] = useState<FactoryDraft[]>([
    { chainIdStr: "", address: "" },
  ]);
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);
  const [missingBackend, setMissingBackend] = useState(false);

  const updateFactory = useCallback(
    (idx: number, patch: Partial<FactoryDraft>) => {
      setFactories((prev) =>
        prev.map((f, i) => (i === idx ? { ...f, ...patch } : f)),
      );
    },
    [],
  );

  const addFactoryRow = useCallback(() => {
    setFactories((prev) => [...prev, { chainIdStr: "", address: "" }]);
  }, []);

  const removeFactoryRow = useCallback((idx: number) => {
    setFactories((prev) =>
      prev.length <= 1 ? prev : prev.filter((_, i) => i !== idx),
    );
  }, []);

  // Build + validate the candidate body. Returns null when invalid so the
  // submit button stays disabled and we never send a malformed request.
  const candidate: CreateDexBody | null = useMemo(() => {
    const trimmedName = name.trim();
    if (trimmedName.length === 0) return null;
    if (protocolType.trim().length === 0) return null;

    const parsedFactories: CreateDexFactory[] = [];
    for (const f of factories) {
      const chainId = Number(f.chainIdStr);
      const addr = f.address.trim();
      // A wholly-empty extra row is ignored; a partially-filled one is invalid.
      const empty = f.chainIdStr.trim() === "" && addr === "";
      if (empty) continue;
      if (!Number.isInteger(chainId) || chainId <= 0) return null;
      if (!isAddressLike(addr)) return null;
      parsedFactories.push({ chain_id: chainId, address: addr });
    }
    if (parsedFactories.length === 0) return null;

    return {
      name: trimmedName,
      protocol_type: protocolType.trim(),
      factories: parsedFactories,
    };
  }, [name, protocolType, factories]);

  const canSubmit = candidate !== null && adminToken !== "" && !submitting;

  const submit = useCallback(async () => {
    if (!candidate) return;
    setServerError(null);
    setMissingBackend(false);
    setSubmitting(true);
    const res = await createDex(edgeUrl, candidate, adminToken);
    setSubmitting(false);
    if (res.ok) {
      onCreated(res.data);
      return;
    }
    // Fail-honest: distinguish "backend route not wired" (404) from a real
    // server-side rejection. Never render a fake success.
    if (res.error.startsWith(NOT_IMPLEMENTED_PREFIX)) {
      setMissingBackend(true);
      return;
    }
    setServerError(res.error);
  }, [candidate, edgeUrl, adminToken, onCreated]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="add-dex-title"
      data-testid="dex-add-dialog"
    >
      <div className="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-xl max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between px-5 py-3 border-b border-border">
          <h2 id="add-dex-title" className="text-lg font-bold">
            Register DEX
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground"
            aria-label="Close"
            data-testid="dex-add-close"
          >
            <X className="size-4" />
          </button>
        </div>

        <div className="p-5 space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label htmlFor="ad-name" className="text-xs uppercase tracking-wide">
                Name <span className="text-destructive">*</span>
              </Label>
              <Input
                id="ad-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Uniswap V3"
                maxLength={64}
                data-testid="dex-add-name"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="ad-protocol" className="text-xs uppercase tracking-wide">
                Protocol type <span className="text-destructive">*</span>
              </Label>
              <select
                id="ad-protocol"
                value={protocolType}
                onChange={(e) => setProtocolType(e.target.value)}
                className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring"
                data-testid="dex-add-protocol"
              >
                {protocols.map((p) => (
                  <option key={p} value={p}>
                    {p}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label className="text-xs uppercase tracking-wide">
                Factories (per chain) <span className="text-destructive">*</span>
              </Label>
              <button
                type="button"
                onClick={addFactoryRow}
                className="inline-flex items-center gap-1 text-xs text-primary hover:underline"
                data-testid="dex-add-factory-row"
              >
                <Plus className="size-3" />
                Add chain
              </button>
            </div>

            {factories.map((f, idx) => (
              <div key={idx} className="grid grid-cols-12 gap-2 items-start">
                <div className="col-span-4">
                  {chainCatalog.length > 0 ? (
                    <select
                      value={f.chainIdStr}
                      onChange={(e) =>
                        updateFactory(idx, { chainIdStr: e.target.value })
                      }
                      className="flex h-9 w-full rounded-md border border-input bg-transparent px-2 py-1 text-xs shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring"
                      data-testid={`dex-add-factory-chain-${idx}`}
                      aria-label={`Factory ${idx + 1} chain`}
                    >
                      <option value="">Chain…</option>
                      {chainCatalog.map((c) => (
                        <option key={c.id} value={String(c.id)}>
                          {c.label}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <Input
                      type="number"
                      inputMode="numeric"
                      min={1}
                      value={f.chainIdStr}
                      onChange={(e) =>
                        updateFactory(idx, { chainIdStr: e.target.value })
                      }
                      placeholder="Chain ID"
                      className="text-xs"
                      data-testid={`dex-add-factory-chain-${idx}`}
                      aria-label={`Factory ${idx + 1} chain id`}
                    />
                  )}
                </div>
                <div className="col-span-7">
                  <Input
                    value={f.address}
                    onChange={(e) => updateFactory(idx, { address: e.target.value })}
                    placeholder="0x… factory address"
                    className="font-mono text-xs"
                    data-testid={`dex-add-factory-address-${idx}`}
                    aria-label={`Factory ${idx + 1} address`}
                  />
                </div>
                <div className="col-span-1 flex justify-center pt-1">
                  <button
                    type="button"
                    onClick={() => removeFactoryRow(idx)}
                    disabled={factories.length <= 1}
                    className="text-muted-foreground hover:text-destructive disabled:opacity-30"
                    aria-label={`Remove factory ${idx + 1}`}
                    data-testid={`dex-add-factory-remove-${idx}`}
                  >
                    <Trash2 className="size-3.5" />
                  </button>
                </div>
              </div>
            ))}
          </div>

          {!adminToken && (
            <div className="flex items-start gap-2 rounded-lg border border-warning/40 bg-warning/10 p-3 text-xs text-warning">
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span>
                No admin session — unlock the operator session at /killswitch
                before registering a DEX.
              </span>
            </div>
          )}

          {missingBackend && (
            <div
              className="flex items-start gap-2 rounded-lg border border-warning/40 bg-warning/10 p-3 text-xs text-warning"
              data-testid="dex-add-missing-backend"
            >
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span>
                <span className="font-mono font-semibold">missing_backend_contract</span>
                {" — "}POST /api/v1/dexes is not wired on the edge/api-server.
                Nothing was created (no fabricated success). Wire the backend
                route before this action can persist.
              </span>
            </div>
          )}

          {serverError && (
            <div
              className="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive"
              data-testid="dex-add-error"
            >
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span className="font-mono break-all">{serverError}</span>
            </div>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border bg-muted/30">
          <Button type="button" variant="ghost" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button
            type="button"
            onClick={submit}
            disabled={!canSubmit}
            data-testid="dex-add-submit"
          >
            {submitting ? "Saving…" : "Register DEX"}
          </Button>
        </div>
      </div>
    </div>
  );
}

export default AddDexDialog;
