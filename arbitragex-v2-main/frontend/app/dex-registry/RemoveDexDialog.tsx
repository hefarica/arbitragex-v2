"use client";

/**
 * FE-CRIT-06 — RemoveDexDialog
 *
 * Confirmation modal for deleting a DEX via DELETE /api/v1/dexes/:id (deleteDex).
 *
 * Doctrine:
 *   - Destructive action requires an explicit confirm — never a one-click delete.
 *   - Fail-honest: a 404 from the mutation surfaces as `missing_backend_contract`;
 *     any other server rejection (e.g. dex_has_pools 409) surfaces verbatim.
 *     NEVER a fake success.
 *   - The actual mutation lives in the parent (handleRemove) so the parent owns
 *     the refetch on success; this dialog only renders confirm/loading/error.
 */

import * as React from "react";
import { AlertCircle, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { DexRow } from "@/lib/api/dexes";

const NOT_IMPLEMENTED_PREFIX = "API endpoint not yet implemented";

interface Props {
  dex: DexRow;
  removing: boolean;
  error: string | null;
  adminToken: string;
  onConfirm: () => void;
  onClose: () => void;
}

export function RemoveDexDialog({
  dex,
  removing,
  error,
  adminToken,
  onConfirm,
  onClose,
}: Props) {
  const missingBackend =
    error != null && error.startsWith(NOT_IMPLEMENTED_PREFIX);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="remove-dex-title"
      data-testid="dex-remove-dialog"
    >
      <div className="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-md">
        <div className="flex items-center justify-between px-5 py-3 border-b border-border">
          <h2 id="remove-dex-title" className="text-lg font-bold text-destructive">
            Remove DEX
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground"
            aria-label="Close"
            data-testid="dex-remove-close"
          >
            <X className="size-4" />
          </button>
        </div>

        <div className="p-5 space-y-4">
          <p className="text-sm text-foreground/90">
            Permanently remove{" "}
            <span className="font-semibold">{dex.name}</span>{" "}
            <span className="font-mono text-xs text-muted-foreground">
              ({dex.protocol_type})
            </span>{" "}
            from the registry? The backend refuses if any pool still references it.
          </p>

          {!adminToken && (
            <div className="flex items-start gap-2 rounded-lg border border-warning/40 bg-warning/10 p-3 text-xs text-warning">
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span>
                No admin session — unlock the operator session at /killswitch
                before removing a DEX.
              </span>
            </div>
          )}

          {missingBackend && (
            <div
              className="flex items-start gap-2 rounded-lg border border-warning/40 bg-warning/10 p-3 text-xs text-warning"
              data-testid="dex-remove-missing-backend"
            >
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span>
                <span className="font-mono font-semibold">missing_backend_contract</span>
                {" — "}DELETE /api/v1/dexes/:id is not wired. Nothing was removed.
              </span>
            </div>
          )}

          {error && !missingBackend && (
            <div
              className="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive"
              data-testid="dex-remove-error"
            >
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span className="font-mono break-all">{error}</span>
            </div>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border bg-muted/30">
          <Button type="button" variant="ghost" onClick={onClose} disabled={removing}>
            Cancel
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={onConfirm}
            disabled={removing || adminToken === ""}
            data-testid="dex-remove-confirm"
          >
            {removing ? "Removing…" : "Remove"}
          </Button>
        </div>
      </div>
    </div>
  );
}

export default RemoveDexDialog;
