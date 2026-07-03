"use client";

/**
 * useWalletOnboarding — the pure onboarding state machine. Given the discovered EIP-6963 providers and a
 * selected wallet id, it resolves the next phase: `install` (not detected → show the official-site
 * prompt) or `connect` (detected → ask for explicit confirmation). WalletConnect (a protocol connector)
 * skips install and goes straight to connect. This hook holds no key, connects nothing, and never
 * auto-selects — the operator drives every transition.
 */

import { useCallback, useMemo, useState } from "react";
import { getWalletEntry, type SupportedWalletId, type WalletInstallRegistryEntry } from "@/lib/web3/wallet-registry";
import {
  resolveWalletAvailability,
  isProtocolConnector,
  type EIP6963ProviderDetail,
} from "@/lib/web3/wallet-discovery";

export type OnboardingPhase = "idle" | "install" | "connect";

export interface UseWalletOnboarding {
  phase: OnboardingPhase;
  selected: SupportedWalletId | null;
  entry: WalletInstallRegistryEntry | null;
  installed: boolean;
  /** rdns of the announced EIP-6963 provider matched for the selected wallet (null if none). */
  matchedRdns: string | null;
  select: (id: SupportedWalletId) => void;
  cancel: () => void;
}

export function useWalletOnboarding(providers: EIP6963ProviderDetail[]): UseWalletOnboarding {
  const [selected, setSelected] = useState<SupportedWalletId | null>(null);

  const entry = useMemo(() => (selected ? getWalletEntry(selected) ?? null : null), [selected]);

  const availability = useMemo(
    () => (selected && entry ? resolveWalletAvailability(selected, providers) : null),
    [selected, entry, providers],
  );

  // "installed" is decided by EIP-6963 discovery (an announced injected provider) OR protocol connectors
  // (WalletConnect). NOTE: non-extension paths that some wallets also offer — Coinbase's SDK/QR connector,
  // Phantom mobile — are NOT surfaced here; when only those are available the flow routes to the install
  // prompt (a fail-honest state, never a silent no-op). WalletConnect remains offered as the QR/protocol
  // alternative. Expanding to SDK connectors would require verifying their exact wagmi connector ids first.
  const installed = useMemo(() => {
    if (!selected || !entry) return false;
    if (isProtocolConnector(selected)) return true; // no local install needed
    return availability?.installed ?? false;
  }, [selected, entry, availability]);

  const matchedRdns = availability?.matchedRdns ?? null;

  const phase: OnboardingPhase = useMemo(() => {
    if (!selected || !entry) return "idle";
    return installed ? "connect" : "install";
  }, [selected, entry, installed]);

  const select = useCallback((id: SupportedWalletId) => setSelected(id), []);
  const cancel = useCallback(() => setSelected(null), []);

  return { phase, selected, entry, installed, matchedRdns, select, cancel };
}
