"use client";
import { getApiBaseUrl, getReadiness, getReadinessBlockers } from "@/lib/api-client";
import { usePaperModeState } from "@/hooks/usePaperModeState";

import { useState, useEffect } from "react";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import { useRouter } from "next/navigation";
import { getAdminToken, hasAdminSession } from "@/lib/admin-token";

export function PaperModeToggle({ chainId }: { chainId: number }) {
  const { data, isLoading } = usePaperModeState(chainId);
  const [loading, setLoading] = useState(false);
  const [checked, setChecked] = useState(data.enabled);
  const [mounted, setMounted] = useState(false);
  // R1: session check deferred to useEffect — never read document.cookie during SSR.
  const [hasSession, setHasSession] = useState(false);
  const router = useRouter();

  useEffect(() => {
    setMounted(true);
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  // Sync local checked state with canonical hook data
  useEffect(() => {
    setChecked(data.enabled);
  }, [data.enabled]);

  const handleToggle = async (val: boolean) => {
    // Validate chainId is positive integer before POST
    if (!Number.isFinite(chainId) || chainId <= 0 || !Number.isInteger(chainId)) {
      toast.error(`Invalid chain ID: ${chainId}`);
      return;
    }

    if (!hasAdminSession()) {
      setHasSession(false);  // sync React state with reality
      toast.error("Admin session required — open /killswitch and unlock a session first.");
      return;
    }

    // Block toggle if confidence is default_safe or conflict or degraded
    if (data.confidence === "default_safe" || data.conflict || data.degraded) {
      const reasons = [
        data.confidence === "default_safe" ? "DEFAULT_SAFE" : null,
        data.conflict ? "CONFLICT" : null,
        data.degraded ? "DEGRADED" : null,
      ].filter(Boolean).join(" · ");
      toast.error(`Paper mode toggle blocked: ${reasons}`);
      return;
    }

    // Gate the flip to LIVE (paper_mode OFF). Doctrine: never enable live capital
    // while readiness is blocked. Uses /api/readiness (flip_blocked) + the blocker
    // list — NOT getReadinessDecision (which has no flip_blocked field).
    if (val === false) {
      const rd = await getReadiness();
      if (!rd.ok) {
        toast.error(`Readiness check failed: ${rd.error} — live trading stays disabled`);
        setChecked(true);
        return;
      }
      if (rd.data.flip_blocked) {
        const bl = await getReadinessBlockers();
        const top =
          bl.ok && bl.data.blockers.length > 0
            ? bl.data.blockers.slice(0, 3).map((b) => b.title).join("; ")
            : `${rd.data.summary.total - rd.data.summary.green} readiness item(s) not green`;
        toast.error(`NO-GO: ${top} — resolve before enabling live trading`);
        setChecked(true);
        return;
      }
      const confirmed = window.confirm(
        "⚠ ACTIVATE LIVE TRADING\n\nReal capital will be at risk.\nConfirm?",
      );
      if (!confirmed) {
        setChecked(true);
        return;
      }
    }

    setLoading(true);
    setChecked(val);
    try {
      const token = getAdminToken() || "";
      const res = await fetch(`${getApiBaseUrl().replace(/\/$/, "")}/admin/config/paper-mode`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "x-arbx-admin-token": token,
        },
        credentials: "include", // For httpOnly session cookie
        body: JSON.stringify({ enabled: val, chain_id: chainId }),
      });
      if (!res.ok) {
        throw new Error(`Failed to update: HTTP ${res.status}`);
      }
      toast.success(`Paper mode ${val ? "enabled" : "disabled"}`);
      router.refresh();
    } catch (err) {
      toast.error((err as Error).message);
      setChecked(!val);
    } finally {
      setLoading(false);
    }
  };

  if (!mounted) return null;

  const isBlocked = data.confidence === "default_safe" || data.conflict || data.degraded;
  const confidenceLabel = data.confidence.toUpperCase();

  return (
    <div className="flex items-center space-x-2">
      <Switch
        id="paper-mode"
        checked={checked}
        onCheckedChange={handleToggle}
        disabled={loading || !hasSession || isLoading || isBlocked}
        title={
          !hasSession
            ? "Admin session required — open /killswitch first"
            : isBlocked
              ? `Blocked: ${confidenceLabel}`
              : undefined
        }
      />
      <Label htmlFor="paper-mode" className="font-mono text-sm cursor-pointer">
        {checked ? "Paper Mode: ON" : "Paper Mode: OFF"}
      </Label>
      <span className="text-[11px] text-muted-foreground font-mono">
        {confidenceLabel}
        {data.conflict && " · CONFLICT"}
        {data.degraded && " · DEGRADED"}
      </span>
    </div>
  );
}
