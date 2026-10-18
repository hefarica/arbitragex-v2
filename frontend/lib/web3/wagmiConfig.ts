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
 * the default chain all come from env.
 *
 * PR-A (2026-10-18) — SEV-1 FIX. Before this change the absent-config path fed
 * the old placeholder project-id literal into
 * RainbowKit's `getDefaultConfig` as the `projectId`. RainbowKit 2.2.x builds
 * its WalletConnect connector on Reown AppKit, so the page initialised that SDK
 * and the live site called PRODUCTION endpoints with a bogus id:
 *   POST https://pulse.walletconnect.org/e?projectId=<placeholder>&st=appkit  -> 400
 *   GET  https://api.web3modal.org/appkit/v1/config?projectId=<placeholder>                      -> 403
 *
 * Now, when no real project id is configured we build the SAME RainbowKit
 * config but with a wallet list that contains NO WalletConnect/Reown wallet
 * (`injectedWallet` only). No WalletConnect connector is created, therefore no
 * AppKit SDK is initialised and no upstream call happens — injected / EIP-6963
 * wallets keep working. The degraded state is declared in the DOM by the
 * server-rendered `WalletConnectDegradedNotice`.
 *
 * SECURITY: this config NEVER provisions a signer key. It only lets the browser
 * wallet expose its address read-only. No private key, no broadcast path here.
 */

import { cookieStorage, createStorage } from "wagmi";
import { getDefaultConfig } from "@rainbow-me/rainbowkit";
import { injectedWallet } from "@rainbow-me/rainbowkit/wallets";
import type { Chain } from "viem";

import { resolveSupportedChains } from "./chains";
import {
  walletConnectEnabled,
  walletConnectProjectId,
  walletConnectStatus,
  WALLETCONNECT_NOT_CONFIGURED,
} from "./walletconnect-config";

// Re-exported so existing client imports keep working with ONE definition.
export {
  walletConnectEnabled,
  walletConnectProjectId,
  walletConnectStatus,
  WALLETCONNECT_NOT_CONFIGURED,
};

let _config: ReturnType<typeof getDefaultConfig> | null = null;

/**
 * Build (and memoise) the wagmi config.
 *
 * Configured   → RainbowKit's default wallet list (includes WalletConnect).
 * Not configured → same RainbowKit surface with an injected-only wallet list,
 *                  so the WalletConnect SDK is never instantiated. `projectId`
 *                  is passed as the empty string: it is unused by the injected
 *                  wallet, and passing a placeholder is exactly the bug this
 *                  fix removes.
 */
export function getWagmiConfig() {
  if (_config) return _config;
  const chains = resolveSupportedChains();
  // getDefaultConfig requires a non-empty chains tuple.
  const chainTuple = (chains.length > 0 ? chains : resolveSupportedChains()) as readonly [Chain, ...Chain[]];

  const projectId = walletConnectProjectId();
  if (projectId) {
    _config = getDefaultConfig({
      appName: "QuantumX — Wallet (read-only)",
      projectId,
      chains: chainTuple,
      ssr: true,
      storage: createStorage({ storage: cookieStorage }),
    });
    return _config;
  }

  _config = getDefaultConfig({
    appName: "QuantumX — Wallet (read-only)",
    // Unused by the injected-only wallet list below; never a placeholder that
    // reaches an upstream SDK.
    projectId: "",
    wallets: [{ groupName: "Browser", wallets: [injectedWallet] }],
    chains: chainTuple,
    ssr: true,
    storage: createStorage({ storage: cookieStorage }),
  });
  return _config;
}

/** True when a WalletConnect/Reown connector exists in the built config. */
export function walletConnectConnectorAvailable(): boolean {
  return walletConnectProjectId() !== null;
}
