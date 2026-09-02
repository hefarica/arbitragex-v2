"use client";

/**
 * FE-13 — SettingsClient
 *
 * Client form for user preferences. Persists to localStorage via useUserPrefs.
 *
 * R1 (Mounted Snapshot Pattern):
 *  - `useState(DEFAULT_PREFS)` provides a deterministic SSR-safe initial render.
 *  - useUserPrefs() reads localStorage inside a useEffect on mount.
 *  - No window / localStorage access in render path.
 *
 * R8 (Fail-Honest): renders DEFAULT_PREFS until localStorage is read on mount.
 * The form is disabled until mount so the user never submits defaults
 * believing they are saved values.
 *
 * FE-6 integration: changing notification_threshold_usd here propagates to
 * OpportunitiesClient via the shared useUserPrefs hook (both components read
 * from the same localStorage key on mount / poll cycle).
 *
 * FE-11 integration: polling_interval_ms is respected by StatusClient (hardcoded
 * POLL_INTERVAL_MS there is a separate constant; this preference is advisory
 * until FE-11 is wired, but it is persisted for future use).
 */

import React, { useEffect, useState } from "react";
import { toast } from "sonner";
import { SaveIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
// DAPP-SURFACE (2026-09-01B): native select replaces Radix Select here — the
// Radix hidden form proxy is unlabeled at SSR/pre-hydration (see
// ThemeOverrideSelect). Radix Select stays for form-less surfaces.
import { ThemeOverrideSelect } from "@/components/settings/ThemeOverrideSelect";
import { Separator } from "@/components/ui/separator";

import { useUserPrefs, DEFAULT_PREFS, type UserPrefs } from "@/lib/user-prefs";
// FE-0041 (§53): /settings is presentation scope by contract — every control
// here writes ONLY localStorage; the badge makes that explicit per card.
import { ControlScopeBadge } from "@/components/ControlScopeBadge";

// ─── Local form state type (all fields as strings for controlled inputs) ──────

interface FormState {
  notification_threshold_usd: string;
  polling_interval_ms: string;
  default_chain_id: string;
  theme_override: UserPrefs["theme_override"];
  compact_density: boolean;
}

function prefsToForm(p: UserPrefs): FormState {
  return {
    notification_threshold_usd: String(p.notification_threshold_usd),
    polling_interval_ms: String(p.polling_interval_ms),
    default_chain_id: p.default_chain_id === null ? "" : String(p.default_chain_id),
    theme_override: p.theme_override,
    compact_density: p.compact_density,
  };
}

function formToPrefs(f: FormState): UserPrefs {
  const threshold = parseFloat(f.notification_threshold_usd);
  const interval = parseInt(f.polling_interval_ms, 10);
  const chainId = f.default_chain_id.trim() === "" ? null : parseInt(f.default_chain_id, 10);

  return {
    notification_threshold_usd: isFinite(threshold) && threshold >= 0 ? threshold : DEFAULT_PREFS.notification_threshold_usd,
    polling_interval_ms: isFinite(interval) && interval >= 1000 ? interval : DEFAULT_PREFS.polling_interval_ms,
    default_chain_id: chainId !== null && isFinite(chainId) ? chainId : null,
    theme_override: f.theme_override,
    compact_density: f.compact_density,
  };
}

// ─── Component ────────────────────────────────────────────────────────────────

export function SettingsClient() {
  const { prefs, savePrefs } = useUserPrefs();

  // R1: isMounted gates form enablement — never render localStorage-derived
  // state on the server or before hydration settles.
  const [isMounted, setIsMounted] = useState(false);
  const [form, setForm] = useState<FormState>(prefsToForm(DEFAULT_PREFS));

  useEffect(() => {
    setIsMounted(true);
  }, []);

  // Sync form state when prefs load from localStorage on mount.
  useEffect(() => {
    if (isMounted) {
      setForm(prefsToForm(prefs));
    }
  }, [isMounted, prefs]);

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!isMounted) return;
    const next = formToPrefs(form);
    savePrefs(next);
    toast.success("Preferences saved", {
      description: "Changes are active immediately in this browser.",
    });
  }

  function handleReset() {
    setForm(prefsToForm(DEFAULT_PREFS));
    savePrefs(DEFAULT_PREFS);
    toast.info("Preferences reset to defaults");
  }

  const disabled = !isMounted;

  return (
    <form onSubmit={handleSubmit} className="space-y-6 max-w-2xl">

      {/* Notifications */}
      <Card>
        <CardHeader className="pb-3">
          <h2 className="text-base font-semibold mt-0 mb-0">
            Notifications <ControlScopeBadge kind="LOCAL_PREFS" className="ml-2 align-middle" />
          </h2>
          <p className="text-xs text-muted-foreground">
            Toast alerts triggered when a live opportunity exceeds the yield threshold.
          </p>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="notification_threshold_usd">
              Yield threshold (USD)
            </Label>
            <Input
              id="notification_threshold_usd"
              type="number"
              min={0}
              step={1}
              inputMode="decimal"
              disabled={disabled}
              value={form.notification_threshold_usd}
              onChange={(e) =>
                setForm((f) => ({ ...f, notification_threshold_usd: e.target.value }))
              }
              className="max-w-xs"
              placeholder="50"
            />
            <p className="text-xs text-muted-foreground">
              Opportunities with net yield &ge; this value show a toast. 0 = notify on all.
            </p>
          </div>
        </CardContent>
      </Card>

      {/* Feed settings */}
      <Card>
        <CardHeader className="pb-3">
          <h2 className="text-base font-semibold mt-0 mb-0">
            Feed &amp; Polling <ControlScopeBadge kind="LOCAL_PREFS" className="ml-2 align-middle" />
          </h2>
          <p className="text-xs text-muted-foreground">
            Controls how often live feeds poll the edge. Minimum 1000 ms.
          </p>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="polling_interval_ms">
              Polling interval (ms)
            </Label>
            <Input
              id="polling_interval_ms"
              type="number"
              min={1000}
              step={500}
              inputMode="numeric"
              disabled={disabled}
              value={form.polling_interval_ms}
              onChange={(e) =>
                setForm((f) => ({ ...f, polling_interval_ms: e.target.value }))
              }
              className="max-w-xs"
              placeholder="5000"
            />
            <p className="text-xs text-muted-foreground">
              Advisory — individual pages have their own poll interval, which will be
              unified with this value in a future sprint.
            </p>
          </div>
          <Separator />
          <div className="space-y-1.5">
            <Label htmlFor="default_chain_id">
              Default chain filter
            </Label>
            <Input
              id="default_chain_id"
              type="number"
              min={1}
              step={1}
              inputMode="numeric"
              disabled={disabled}
              value={form.default_chain_id}
              onChange={(e) =>
                setForm((f) => ({ ...f, default_chain_id: e.target.value }))
              }
              className="max-w-xs"
              placeholder="All chains (leave blank)"
            />
            <p className="text-xs text-muted-foreground">
              Leave blank to show all chains. Enter a chain ID (e.g. 1 for Ethereum Mainnet)
              to pre-filter tables that support it.
            </p>
          </div>
        </CardContent>
      </Card>

      {/* Display */}
      <Card>
        <CardHeader className="pb-3">
          <h2 className="text-base font-semibold mt-0 mb-0">
            Display <ControlScopeBadge kind="LOCAL_PREFS" className="ml-2 align-middle" />
          </h2>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="theme_override">
              Theme override
            </Label>
            <ThemeOverrideSelect
              disabled={disabled}
              value={form.theme_override}
              onChange={(v) => setForm((f) => ({ ...f, theme_override: v }))}
            />
            <p className="text-xs text-muted-foreground">
              &ldquo;System&rdquo; defers to the theme toggle in the header (persisted as{" "}
              <code className="font-mono text-[11px]">arbx_theme</code> in localStorage).
              Selecting Dark or Light here persists your intent across refreshes.
            </p>
          </div>
          <Separator />
          <div className="flex items-center justify-between">
            <div className="space-y-0.5">
              <Label htmlFor="compact_density">
                Compact density
              </Label>
              <p className="text-xs text-muted-foreground">
                Reduces table row padding and font size. Useful on small screens.
              </p>
            </div>
            <Switch
              id="compact_density"
              disabled={disabled}
              checked={form.compact_density}
              onCheckedChange={(checked) =>
                setForm((f) => ({ ...f, compact_density: checked }))
              }
            />
          </div>
        </CardContent>
      </Card>

      {/* Actions */}
      <div className="flex flex-wrap items-center gap-3">
        <Button type="submit" disabled={disabled} className="gap-2">
          <SaveIcon size={15} />
          Save preferences
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={disabled}
          onClick={handleReset}
        >
          Reset to defaults
        </Button>
        {!isMounted && (
          <p className="text-xs text-muted-foreground animate-pulse">
            Loading saved preferences...
          </p>
        )}
      </div>
    </form>
  );
}
