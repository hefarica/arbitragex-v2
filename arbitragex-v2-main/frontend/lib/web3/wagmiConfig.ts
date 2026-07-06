/**
 * web3/wagmiConfig — the wagmi v2 config for the read-only wallet surface.
 *
 * Stack (pinned to avoid the wagmi-v3 / RainbowKit-2 incompatibility):
 *   wagmi@2, viem@2, @rainbow-me/rainbowkit@2, @tanstack/react-query@5.
 *
 * SSR correctness (Next 15 App Router): we use wagmi `cookieStorage` so the
 * server and client agree on the initial connection state, avoiding React #418
 * hydration mismatches. The Web3Provider hydrates with `cookieToInitialState`.
 *
 * No-hardcode (RULE 00): the WalletConnect project id, the enabled chains, and
 * the default chain all come from env. If NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID
 * is absent the WalletConnect connector degrades to a fail-honest
 * `walletconnect_project_id_missing` state — injected / MetaMask via EIP-6963
 * still works.
 *
 * SECURITY: this config NEVER provisions a signer key. It only lets the browser
 * wallet expose its address read-only. No private key, no broadcast path here.
 */

import { cookieStorage, createStorage } from "wagmi";
import { getDefaultConfig } from "@rainbow-me/rainbowkit";
import type { Chain } from "viem";

import { resolveSupportedChains } from "./chains";

/** Whether wallet connect (the dApp surface) is enabled at all. */
export function walletConnectEnabled(): boolean {
  const v = (process.env.NEXT_PUBLIC_WALLET_CONNECT_ENABLED ?? "").trim().toLowerCase();
  // Default ON (the page is always safe-gated). Only an explicit "false"/"0"/"off"
  // disables the connect affordances (they then render as a disabled shell).
  return !(v === "false" || v === "0" || v === "off");
}

/** The WalletConnect (Reown) project id, or null when not configured. */
export function walletConnectProjectId(): string | null {
  const v = (process.env.NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID ?? "").trim();
  return v.length > 0 ? v : null;
}

/**
 * Fail-honest reason for a degraded WalletConnect connector. When the project id
 * is missing, WalletConnect is unavailable but injected/EIP-6963 wallets still
 * work — the UI surfaces this reason instead of silently breaking.
 */
export function walletConnectStatus(): { available: boolean; reason: string | null } {
  if (walletConnectProjectId()) return { available: true, reason: null };
  return { available: false, reason: "walletconnect_project_id_missing" };
}

let _config: ReturnType<typeof getDefaultConfig> | null = null;

/**
 * Build (and memoise) the wagmi config. getDefaultConfig wires the standard
 * RainbowKit connectors (injected/EIP-6963 + WalletConnect when the project id
 * is present). We pass a placeholder project id only to satisfy the type when
 * WalletConnect is disabled; the WalletConnect connector is non-functional
 * without a real id, which is exactly the fail-honest degradation we want
 * (injected wallets remain fully functional).
 */
export function getWagmiConfig() {
  if (_config) return _config;
  const chains = resolveSupportedChains();
  // getDefaultConfig requires a non-empty chains tuple.
  const chainTuple = (chains.length > 0 ? chains : resolveSupportedChains()) as readonly [Chain, ...Chain[]];

  _config = getDefaultConfig({
    appName: "QuantumX — Wallet (read-only)",
    // A real project id enables WalletConnect; the placeholder leaves it
    // degraded (fail-honest), injected/EIP-6963 wallets still connect.
    projectId: walletConnectProjectId() ?? "walletconnect_project_id_missing",
    chains: chainTuple,
    ssr: true,
    storage: createStorage({ storage: cookieStorage }),
  });
  return _config;
}
