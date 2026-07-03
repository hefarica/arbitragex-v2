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

/** Minimal shape of a live wagmi connector (from useConnectors) needed to match — id + name only. */
export interface ConnectorMeta {
  id: string;
  name: string;
}

/**
 * PURE. Resolve which live wagmi connector to use for a selected registry wallet.
 *
 * wagmi's `multiInjectedProviderDiscovery` (enabled by RainbowKit getDefaultConfig) converts each
 * announced EIP-6963 provider into an `injected` connector whose `id` IS the provider's rdns (via the
 * `mipd` library). So for a DETECTED wallet the precise link is rdns → connector.id. Priority:
 *   1. the rdns matched during discovery (the exact detected wallet),
 *   2. any rdns in the registry allowlist advertised as a connector id,
 *   3. the registry's RainbowKit `connectorId` (metaMask / coinbaseWallet / walletConnect built-ins),
 *   4. a case-insensitive name overlap (last resort, both directions).
 *
 * Returns the connector id to connect, or `null` when NOTHING matches — the UI must then render a
 * fail-honest "not connectable" state instead of a dead button (never a silent no-op).
 */
export function resolveConnectorId(
  entry: Pick<WalletInstallRegistryEntry, "connectorId" | "name" | "rdnsAllowlist">,
  matchedRdns: string | null,
  connectors: ConnectorMeta[],
): string | null {
  const find = (pred: (c: ConnectorMeta) => boolean): string | null => connectors.find(pred)?.id ?? null;

  // 1. exact rdns matched during discovery.
  if (matchedRdns) {
    const byRdns = find((c) => lc(c.id) === lc(matchedRdns));
    if (byRdns) return byRdns;
  }
  // 2. any allowlisted rdns advertised as a connector id.
  for (const rdns of entry.rdnsAllowlist) {
    const byAllow = find((c) => lc(c.id) === lc(rdns));
    if (byAllow) return byAllow;
  }
  // 3. RainbowKit built-in connectorId.
  if (entry.connectorId) {
    const cid = entry.connectorId;
    const byCid = find((c) => lc(c.id) === lc(cid));
    if (byCid) return byCid;
  }
  // 4. case-insensitive name overlap — DEFENSIVE last resort. The install gate makes this unreachable in
  //    the wired connect flow (rdns already matched by then); it exists only for robustness. Guard against
  //    empty/degenerate names: a connector's name is self-attested (EIP-6963), so without this guard an
  //    empty name would substring-match EVERY wallet (`want.includes("")` is always true) and defeat the
  //    fail-honest guarantee. Require both names present and a meaningful (>=3 char) overlap.
  const want = lc(entry.name);
  if (want.length < 3) return null;
  return find((c) => {
    const have = lc(c.name);
    if (have.length < 3) return false;
    return have === want || have.includes(want) || want.includes(have);
  });
}
