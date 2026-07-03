// Wallet install registry — a STATIC, versioned, auditable allowlist of supported wallets and their
// OFFICIAL download URLs. The onboarding guard may only ever open a URL that comes from THIS registry
// (never a dynamic/user-supplied URL). This module holds no key, signs nothing, and installs nothing —
// it maps a wallet id to its official site so the operator can install it themselves.

export type SupportedWalletId = "metamask" | "rabby" | "coinbase" | "phantom" | "rainbow" | "walletconnect";

export interface WalletInstallRegistryEntry {
  id: SupportedWalletId;
  name: string;
  /** Self-attested EIP-6963 rdns values — used as a SIGNAL for discovery, never as sole proof. */
  rdnsAllowlist: string[];
  /** The ONLY URL the guard may open for this wallet. Official site only. */
  installUrl: string;
  docsUrl?: string;
  /**
   * Best-effort RainbowKit/wagmi connector id, used ONLY as a fallback in resolveConnectorId. The
   * AUTHORITATIVE match for an installed injected wallet is its EIP-6963 rdns (wagmi's mipd sets the
   * discovered connector's id === rdns). Values here for wallets outside RainbowKit's default set
   * (rabby/phantom/rainbow) are not guaranteed to be real connector ids — they only help if the wallet
   * happens to also be a getDefaultConfig built-in; otherwise the rdns path is what connects them.
   */
  connectorId?: string;
  expectedProviderFlags?: string[];
  supportedPlatforms: Array<"desktop" | "mobile" | "extension" | "walletconnect">;
  warning: string;
}

export const WALLET_INSTALL_REGISTRY: WalletInstallRegistryEntry[] = [
  {
    id: "metamask",
    name: "MetaMask",
    rdnsAllowlist: ["io.metamask"],
    installUrl: "https://metamask.io/download",
    docsUrl: "https://docs.metamask.io/",
    connectorId: "metaMask",
    expectedProviderFlags: ["isMetaMask"],
    supportedPlatforms: ["desktop", "mobile", "extension"],
    warning: "Descarga MetaMask únicamente desde el sitio oficial.",
  },
  {
    id: "rabby",
    name: "Rabby Wallet",
    rdnsAllowlist: ["io.rabby", "com.debank.rabby"],
    installUrl: "https://rabby.io/",
    connectorId: "rabby",
    expectedProviderFlags: ["isRabby"],
    supportedPlatforms: ["desktop", "extension"],
    warning: "Descarga Rabby únicamente desde el sitio oficial.",
  },
  {
    id: "coinbase",
    name: "Coinbase Wallet",
    rdnsAllowlist: ["com.coinbase.wallet"],
    installUrl: "https://www.coinbase.com/wallet/downloads",
    connectorId: "coinbaseWallet",
    expectedProviderFlags: ["isCoinbaseWallet"],
    supportedPlatforms: ["desktop", "mobile", "extension"],
    warning: "Descarga Coinbase Wallet únicamente desde el sitio oficial.",
  },
  {
    id: "phantom",
    name: "Phantom",
    rdnsAllowlist: ["app.phantom"],
    installUrl: "https://phantom.com/download",
    connectorId: "phantom",
    expectedProviderFlags: ["isPhantom"],
    supportedPlatforms: ["desktop", "mobile", "extension"],
    warning: "Descarga Phantom únicamente desde el sitio oficial.",
  },
  {
    id: "rainbow",
    name: "Rainbow",
    rdnsAllowlist: ["me.rainbow"],
    installUrl: "https://rainbow.me/download",
    connectorId: "rainbow",
    expectedProviderFlags: ["isRainbow"],
    supportedPlatforms: ["desktop", "mobile", "extension"],
    warning: "Descarga Rainbow únicamente desde el sitio oficial.",
  },
  {
    id: "walletconnect",
    name: "WalletConnect",
    rdnsAllowlist: [],
    installUrl: "https://walletconnect.network/",
    connectorId: "walletConnect",
    supportedPlatforms: ["desktop", "mobile", "walletconnect"],
    warning: "WalletConnect no requiere instalar una extensión local; usa QR o deep link.",
  },
];

const BY_ID = new Map<SupportedWalletId, WalletInstallRegistryEntry>(WALLET_INSTALL_REGISTRY.map((e) => [e.id, e]));
const ALLOWED_INSTALL_URLS = new Set<string>(WALLET_INSTALL_REGISTRY.map((e) => e.installUrl));

export function getWalletEntry(id: SupportedWalletId): WalletInstallRegistryEntry | undefined {
  return BY_ID.get(id);
}

/** The ONLY safe way to obtain an install URL — returns the registry's official URL or null. */
export function installUrlFor(id: SupportedWalletId): string | null {
  return BY_ID.get(id)?.installUrl ?? null;
}

/** Guard: a URL may be opened ONLY if it is an exact official URL in the registry + https. */
export function isAllowedInstallUrl(url: string): boolean {
  if (!ALLOWED_INSTALL_URLS.has(url)) return false;
  try {
    return new URL(url).protocol === "https:";
  } catch {
    return false;
  }
}

export function allWalletIds(): SupportedWalletId[] {
  return WALLET_INSTALL_REGISTRY.map((e) => e.id);
}
