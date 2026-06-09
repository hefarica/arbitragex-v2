"use client";

/**
 * Web3Provider — wraps the app in wagmi + react-query + RainbowKit so the
 * read-only wallet surface can connect a browser wallet.
 *
 * SSR correctness (Next 15 App Router, avoid React #418):
 *   - Client component. WagmiProvider receives `initialState` derived from the
 *     request cookies (cookieToInitialState) so server and client agree on the
 *     connection state at first paint — no hydration mismatch.
 *   - QueryClient is created once via useState so it is stable across renders.
 *
 * SECURITY: this provider exposes ONLY read-only wallet connectivity. There is
 * no signer key, no broadcast transport, no capital path. RainbowKitProvider is
 * used purely for the connect/account modal UI.
 */

import { useState, type ReactNode } from "react";
import { WagmiProvider, cookieToInitialState } from "wagmi";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RainbowKitProvider, darkTheme } from "@rainbow-me/rainbowkit";

import "@rainbow-me/rainbowkit/styles.css";

import { getWagmiConfig } from "@/lib/web3/wagmiConfig";

export function Web3Provider({
  children,
  cookie,
}: {
  children: ReactNode;
  /** Raw Cookie header forwarded from the server for SSR hydration. */
  cookie?: string | null;
}) {
  const config = getWagmiConfig();
  const initialState = cookieToInitialState(config, cookie ?? undefined);
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            // Wallet/identity reads are cheap and rarely change; keep them quiet.
            staleTime: 30_000,
            retry: 1,
          },
        },
      }),
  );

  return (
    <WagmiProvider config={config} initialState={initialState}>
      <QueryClientProvider client={queryClient}>
        <RainbowKitProvider theme={darkTheme()} modalSize="compact">
          {children}
        </RainbowKitProvider>
      </QueryClientProvider>
    </WagmiProvider>
  );
}
