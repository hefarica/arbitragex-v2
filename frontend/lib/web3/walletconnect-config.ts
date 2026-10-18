/**
 * web3/walletconnect-config — SERVER-SAFE WalletConnect configuration facts.
 *
 * Deliberately free of `wagmi` / `@rainbow-me/rainbowkit` imports so that Server
 * Components can read the deployment facts and render an honest degraded state
 * during SSR. `wagmiConfig.ts` re-exports these for the client components, so
 * there is exactly ONE definition of "is WalletConnect configured".
 *
 * PR-A (2026-10-18): before this split the module that *built* the wagmi config
 * also owned the status, so the only way to know "WalletConnect is not
 * configured" was to import the client-only SDK — and the degraded path was
 * rendered client-side only (absent from the SSR HTML).
 */

/** Whether the WalletConnect (dApp) surface is enabled at all. */
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
 * Fail-honest machine reason for a degraded WalletConnect connector.
 *
 * PR-A: renamed from the old placeholder project-id literal to
 * `walletconnect_not_configured`. The old token was the old placeholder project-id
 * value that the buggy path passed to the WalletConnect SDK; reusing that exact
 * string as a status code made it impossible to tell "status" from "the bogus
 * value we sent upstream" — and it is the token the anti-regression gate
 * `G-WALLET-PROJECTID-01` greps for in the BUILT chunks. A status code should
 * name the state, not the value that used to leak.
 */
export const WALLETCONNECT_NOT_CONFIGURED = "walletconnect_not_configured";

export function walletConnectStatus(): { available: boolean; reason: string | null } {
  if (walletConnectProjectId()) return { available: true, reason: null };
  return { available: false, reason: WALLETCONNECT_NOT_CONFIGURED };
}
