"use client";
import { getApiBaseUrl } from "@/lib/api-client";

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
    if (!hasSession) {
      toast.error("Admin session required — open /killswitch and unlock a session first.");
      return;
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
