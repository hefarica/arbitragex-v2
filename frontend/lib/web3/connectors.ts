/**
 * web3/connectors — connector availability metadata for the UI.
 *
 * The actual connector instances are wired by RainbowKit's getDefaultConfig
 * (see wagmiConfig.ts). This module exposes a small, SSR-safe descriptor of
 * which connection methods are available and WHY one might be degraded, so the
 * UI can render an honest disabled/degraded shell instead of a broken button.
 *
 * No private keys, no signers — purely descriptive.
 */

import { walletConnectEnabled, walletConnectStatus } from "./wagmiConfig";

export interface ConnectorDescriptor {
  id: string;
  label: string;
  /** True when this method can be offered to the operator. */
  available: boolean;
  /** Fail-honest reason when `available` is false (RULE 00 / R8). */
  reason: string | null;
}

/**
 * SSR-safe connector descriptors. `injected` (MetaMask / EIP-6963) is always
 * considered offerable (its real presence is detected client-side by wagmi).
 * `walletConnect` reflects the project-id configuration: degraded + honest
 * reason when NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID is absent.
 */
export function describeConnectors(): ConnectorDescriptor[] {
  const wc = walletConnectStatus();
  const wcEnabled = walletConnectEnabled();
  return [
    {
      id: "injected",
      label: "Browser wallet (MetaMask / EIP-6963)",
      available: true,
      reason: null,
    },
    {
      id: "walletConnect",
      label: "WalletConnect / Reown",
      available: wcEnabled && wc.available,
      reason: !wcEnabled ? "wallet_connect_disabled" : wc.reason,
    },
  ];
}
