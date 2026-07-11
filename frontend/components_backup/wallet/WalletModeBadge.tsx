"use client";

/**
 * WalletModeBadge — renders the CURRENTLY-PERMITTED wallet mode (READ_ONLY / MANUAL_SIGN /
 * INTENT_SIGN / AUTOMATION_LOCKED) derived from the backend safety posture via the pure,
 * deny-by-default resolver in lib/web3/modes. In the current posture (live disabled) this always
 * resolves to READ_ONLY with automation LOCKED. Display-only: it renders a badge, nothing else —
 * it never signs, broadcasts, or unlocks anything.
 */

import { Badge } from "@/components/ui/badge";
import { useWalletSafety } from "@/hooks/useWalletSafety";
import { resolveAllowedMode, SAFE_MODE_POSTURE, type ModePosture, type WalletMode } from "@/lib/web3/modes";

const VARIANT: Record<WalletMode, "info" | "warning" | "destructive"> = {
  READ_ONLY: "info",
  MANUAL_SIGN: "warning",
  INTENT_SIGN: "warning",
  AUTOMATION_LOCKED: "destructive",
};

export function WalletModeBadge() {
  const { safety } = useWalletSafety();

  // Map the (already-safe-pinned) safety posture onto the mode posture. Fields the safety endpoint
  // does not expose (readiness/kill-switch/simulation/canary) stay safe-false, so the ceiling can
  // never exceed what the backend actually authorizes.
  const posture: ModePosture = {
    ...SAFE_MODE_POSTURE,
    live_enabled: safety?.live_enabled ?? false,
    blind_signing_enabled: safety?.blind_signing_enabled ?? false,
  };
  const decision = resolveAllowedMode(posture);

  return (
    <div className="flex flex-wrap items-center gap-2" data-testid="wallet-mode-badge">
      <span className="text-xs uppercase tracking-widest text-muted-foreground/70">Mode</span>
      <Badge variant={VARIANT[decision.mode]} data-testid="wallet-mode">
        {decision.mode}
      </Badge>
      {decision.automationLocked ? (
        <Badge variant="success" data-testid="automation-locked">
          AUTOMATION LOCKED
        </Badge>
      ) : null}
      <span className="text-xs text-muted-foreground/60">{decision.reason}</span>
    </div>
  );
}
