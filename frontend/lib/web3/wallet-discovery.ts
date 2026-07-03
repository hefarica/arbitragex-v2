// EIP-6963 multi-injected provider discovery for the onboarding guard.
//
// EIP-6963 lets a dApp discover MULTIPLE injected wallets without the window.ethereum collision. The
// dApp dispatches `eip6963:requestProvider`; each wallet answers with `eip6963:announceProvider`
// carrying { info: { uuid, name, icon, rdns }, provider }. `rdns` is SELF-ATTESTED (per the EIP) — we
// use it as a SIGNAL matched against the static registry allowlist, never as sole proof, and we never
// touch the announced `provider` object here (no requests, no signing).

import { getWalletEntry, type SupportedWalletId, type WalletInstallRegistryEntry } from "./wallet-registry";

export interface EIP6963ProviderInfo {
  uuid: string;
  name: string;
  icon: string;
  rdns: string;
}

export interface EIP6963ProviderDetail {
  info: EIP6963ProviderInfo;
  provider: unknown; // never used here — discovery only
}

export interface WalletAvailability {
  id: SupportedWalletId;
  installed: boolean;
  matchedRdns: string | null;
  name: string;
}

const lc = (s: string): string => (s || "").toLowerCase();

// A DOM-like target so the collector is testable without a real browser.
interface EventTargetLike {
  addEventListener: (t: string, l: (ev: Event) => void) => void;
  removeEventListener: (t: string, l: (ev: Event) => void) => void;
  dispatchEvent: (ev: Event) => boolean;
}

/**
 * Dispatch eip6963:requestProvider and collect announcements for `timeoutMs`. SSR-safe: returns [] when
 * there is no window/target. Read-only: it never calls the announced provider.
 */
export function requestProviders(opts?: { timeoutMs?: number; target?: EventTargetLike | null }): Promise<EIP6963ProviderDetail[]> {
  const target: EventTargetLike | null =
    opts?.target ?? (typeof window !== "undefined" ? (window as unknown as EventTargetLike) : null);
  if (!target) return Promise.resolve([]);

  const found = new Map<string, EIP6963ProviderDetail>();
  return new Promise((resolve) => {
    const onAnnounce = (ev: Event): void => {
      const detail = (ev as CustomEvent<EIP6963ProviderDetail>).detail;
      if (detail && detail.info && typeof detail.info.uuid === "string") {
        found.set(detail.info.uuid, detail);
      }
    };
    target.addEventListener("eip6963:announceProvider", onAnnounce);
    target.dispatchEvent(new Event("eip6963:requestProvider"));
    setTimeout(() => {
      target.removeEventListener("eip6963:announceProvider", onAnnounce);
      resolve([...found.values()]);
    }, opts?.timeoutMs ?? 300);
  });
}

/**
 * PURE. Resolve whether a registry wallet is installed given the announced EIP-6963 providers. Matches
 * by rdns (signal only). WalletConnect (empty rdnsAllowlist) is a protocol, not an injected wallet — it
 * is never "installed" via discovery and never needs a local install.
 */
export function resolveWalletAvailability(
  id: SupportedWalletId,
  announced: EIP6963ProviderDetail[],
  entryOverride?: WalletInstallRegistryEntry,
): WalletAvailability {
  const entry = entryOverride ?? getWalletEntry(id);
  if (!entry) return { id, installed: false, matchedRdns: null, name: id };
  if (entry.rdnsAllowlist.length === 0) {
    // Protocol connector (e.g. WalletConnect): no injected provider to discover.
    return { id, installed: false, matchedRdns: null, name: entry.name };
  }
  const rdnsSet = new Set(entry.rdnsAllowlist.map(lc));
  const match = announced.find((a) => rdnsSet.has(lc(a.info.rdns)));
  return {
    id,
    installed: Boolean(match),
    matchedRdns: match ? match.info.rdns : null,
    name: entry.name,
  };
}

/** True when this wallet is a protocol connector (WalletConnect) — no local install, connect directly. */
export function isProtocolConnector(id: SupportedWalletId): boolean {
  const entry = getWalletEntry(id);
  return Boolean(entry && entry.rdnsAllowlist.length === 0);
}
