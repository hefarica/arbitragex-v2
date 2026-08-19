"use client";

/**
 * RpcBackendToggle — per-service RPC implementation track selector
 * (alloy dual-track FASE 4).
 *
 * Pattern: PaperModeToggle — admin-session check deferred to useEffect (R1,
 * never read document.cookie during SSR), sonner toast feedback, font-mono
 * labels. State + 30s poll come from useRpcBackend (GET /api/admin/rpc-backend).
 *
 * Badges: ethers=blue (info), alloy=green (success), shadow=yellow (warning).
 * An out-of-enum stored value renders a warning badge verbatim (R8 — surface
 * the anomaly, never coerce).
 *
 * Control-plane only: selects the RPC track; never touches trading mode,
 * capital, or broadcast gates (§34 mode-invariant).
 */

import { useEffect, useState } from "react";
import { toast } from "sonner";
import { useRpcBackend } from "@/hooks/useRpcBackend";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { RPC_BACKEND_KINDS, type RpcBackendKind } from "@/lib/schemas";
import { hasAdminSession } from "@/lib/admin-token";

const KIND_VARIANT: Record<string, "info" | "success" | "warning"> = {
  ethers: "info",
  alloy: "success",
  shadow: "warning",
};

function isKind(v: string): v is RpcBackendKind {
  return (RPC_BACKEND_KINDS as readonly string[]).includes(v);
}

export function RpcBackendToggle() {
  const { data, isLoading, isRefreshing, error, setBackend } = useRpcBackend();
  const [mounted, setMounted] = useState(false);
  // R1: session check deferred to useEffect — never read document.cookie during SSR.
  const [hasSession, setHasSession] = useState(false);
  const [pending, setPending] = useState<string | null>(null);

  useEffect(() => {
    setMounted(true);
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  const handleChange = async (service: string, value: string) => {
    if (!isKind(value)) return; // Select only emits items we rendered; defensive.
    if (!hasAdminSession()) {
      setHasSession(false); // sync React state with reality
      toast.error("Admin session required — open /killswitch and unlock a session first.");
      return;
    }
    setPending(`${service}:${value}`);
    try {
      await setBackend(service, value);
      toast.success(`RPC backend — ${service} → ${value}`);
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setPending(null);
    }
  };

  if (!mounted) return null;

  // Fail-safe: no live read (initial load failed) → block changes, show why.
  const degraded = error !== null || data.updated_at === null;

  return (
    <div className="flex flex-col gap-2" data-testid="rpc-backend-toggle">
      <div className="flex items-center gap-2">
        <Label className="font-mono text-sm">RPC Backend</Label>
        {error ? (
          <Badge variant="warning" title={error.message}>
            unavailable
          </Badge>
        ) : isRefreshing ? (
          <span className="text-[11px] text-muted-foreground font-mono">sync…</span>
        ) : null}
      </div>

      {data.services.map((svc) => {
        const valid = isKind(svc.current);
        return (
          <div key={svc.name} className="flex items-center justify-between gap-3">
            <span className="font-mono text-[13px]">{svc.name}</span>
            <div className="flex items-center gap-2">
              <Badge
                variant={valid ? KIND_VARIANT[svc.current] : "warning"}
                title={valid ? undefined : `stored value "${svc.current}" is outside the enum — verify Redis key arbx:rpc_backend:${svc.name}`}
              >
                {svc.current}
              </Badge>
              <Select
                value={valid ? svc.current : undefined}
                onValueChange={(v: string) => void handleChange(svc.name, v)}
                disabled={isLoading || !hasSession || degraded || pending !== null}
              >
                <SelectTrigger
                  size="sm"
                  className="w-28 font-mono text-xs"
                  aria-label={`RPC backend for ${svc.name}`}
                  title={
                    !hasSession
                      ? "Admin session required — open /killswitch first"
                      : degraded
                        ? "State not confirmed from server — changes blocked"
                        : undefined
                  }
                >
                  <SelectValue placeholder={valid ? svc.current : "set…"} />
                </SelectTrigger>
                <SelectContent>
                  {svc.options.map((opt) => (
                    <SelectItem key={opt} value={opt} className="font-mono text-xs">
                      {opt}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
        );
      })}

      {pending !== null && (
        <span className="text-[11px] text-muted-foreground font-mono">
          applying {pending}…
        </span>
      )}
    </div>
  );
}
