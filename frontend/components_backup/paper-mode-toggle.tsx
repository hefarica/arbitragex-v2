"use client";
import { getApiBaseUrl, getReadiness, getReadinessBlockers } from "@/lib/api-client";

import { useState, useEffect } from "react";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import { useRouter } from "next/navigation";
import { getAdminToken, hasAdminSession } from "@/lib/admin-token";

export function PaperModeToggle({ initialValue }: { initialValue: boolean }) {
  const [loading, setLoading] = useState(false);
  const [checked, setChecked] = useState(initialValue);
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

  const handleToggle = async (val: boolean) => {
    if (!hasAdminSession()) {
      setHasSession(false);  // sync React state with reality
      toast.error("Admin session required — open /killswitch and unlock a session first.");
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
        body: JSON.stringify({ enabled: val }),
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

  return (
    <div className="flex items-center space-x-2">
      <Switch
        id="paper-mode"
        checked={checked}
        onCheckedChange={handleToggle}
        disabled={loading || !hasSession}
        title={!hasSession ? "Admin session required — open /killswitch first" : undefined}
      />
      <Label htmlFor="paper-mode" className="font-mono text-sm cursor-pointer">
        {checked ? "Paper Mode: ON" : "Paper Mode: OFF"}
      </Label>
    </div>
  );
}
