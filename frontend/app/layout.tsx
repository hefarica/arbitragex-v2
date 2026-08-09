import type { ReactNode } from "react";
import { GeistSans } from "geist/font/sans";
import { GeistMono } from "geist/font/mono";

import "./globals.css";
import { SiteHeader } from "@/components/site-header";
import { AppSidebar } from "@/components/app-sidebar";
import { PageBreadcrumb } from "@/components/PageBreadcrumb";
import { HeroSphere } from "@/components/HeroSphere";
import { ThemeScript } from "@/components/theme-toggle";
import { SystemGuardBanner } from "@/components/SystemGuardBanner";
import { Toaster } from "sonner";

import { OpportunityTicker } from "@/components/OpportunityTicker";

export const metadata = {
  title: "QuantumX — Control Plane",
  description: "Institutional-grade stochastic convergence operator console",
  // Block automatic translation by Chrome/Edge/Firefox/Safari on mobile.
  // Auto-translation replaces text nodes in the DOM, which breaks React
  // reconciliation with `removeChild on Node` errors when state updates.
  // See https://github.com/facebook/react/issues/11538
  other: {
    google: "notranslate",
  },
};

// Resolve the real paper-mode state server-side so the sidebar badge reflects
// runtime config (not a hardcoded green dot). FAIL-SAFE: any uncertainty
// (missing env, non-OK, parse error, or paper_mode not explicitly false) →
// `true`. We must NEVER paint "LIVE TRADING" on a failed/ambiguous fetch.
// The edge exposes the NON-/v1 path: /api/config/current → /api/v1/config/current.
// (Fetching /api/v1/config/current here 404s at the edge and silently fail-safes.)
async function getPaperMode(): Promise<boolean> {
  try {
    const base = process.env.INTERNAL_EDGE_URL ?? process.env.NEXT_PUBLIC_EDGE_URL;
    if (!base) return true;
    const res = await fetch(`${base.replace(/\/$/, "")}/api/config/current`, {
      headers: { accept: "application/json" },
      cache: "no-store",
    });
    if (!res.ok) return true;
    const json = (await res.json()) as { execution?: { paper_mode?: boolean } };
    return json?.execution?.paper_mode !== false;
  } catch {
    return true;
  }
}

// Count of credentials needing attention (invalid + untested) for the sidebar
// badge on /settings/credentials. Counts only — no sensitive data. FAIL-SAFE:
// any uncertainty → 0 (no badge); we never invent a count. Non-/v1 edge path
// (edge proxies /api/credentials/summary → /api/v1/credentials/summary).
async function getCredsNeedingAttention(): Promise<number> {
  try {
    const base = process.env.INTERNAL_EDGE_URL ?? process.env.NEXT_PUBLIC_EDGE_URL;
    if (!base) return 0;
    const res = await fetch(`${base.replace(/\/$/, "")}/api/credentials/summary`, {
      headers: { accept: "application/json" },
      cache: "no-store",
    });
    if (!res.ok) return 0;
    const json = (await res.json()) as { needs_attention?: number };
    return typeof json?.needs_attention === "number" && json.needs_attention > 0
      ? json.needs_attention
      : 0;
  } catch {
    return 0;
  }
}

export default async function RootLayout({ children }: { children: ReactNode }) {
  const [paperMode, credsNeedsAttention] = await Promise.all([
    getPaperMode(),
    getCredsNeedingAttention(),
  ]);
  return (
    <html
      lang="en"
      translate="no"
      className={`${GeistSans.variable} ${GeistMono.variable} notranslate`}
      suppressHydrationWarning
    >
      <head>
        <ThemeScript />
        <meta name="google" content="notranslate" />
      </head>
      <body className="min-h-dvh bg-background font-sans antialiased notranslate" translate="no">
        {/* Grain Overlay */}
        <div className="grain-overlay" aria-hidden="true" />

        {/* Hero Sphere Background */}
        <HeroSphere />
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:border focus:bg-background focus:px-3 focus:py-2 focus:text-sm focus:font-medium focus:shadow-md focus:outline-none focus:ring-2 focus:ring-ring"
        >
          Skip to main content
        </a>
        {/* B-01: Web3Provider (wagmi/RainbowKit) was removed from the root layout.
            It previously loaded WalletConnect on ALL 56 pages (hydration lag +
            walletconnect.org calls on every route). It now mounts ONLY in
            app/wallet/layout.tsx — the sole surface that connects a wallet. The
            95% observe-only dapp no longer boots Web3. */}
        <div className="flex min-h-dvh flex-col">
            <SiteHeader paperMode={paperMode} />
            {/* Layout: Sidebar starts at top-0, banner is inside main content area */}
            <div className="flex flex-1 relative">
              <AppSidebar paperMode={paperMode} credsNeedsAttention={credsNeedsAttention} />
              <main id="main" tabIndex={-1} className="min-w-0 flex-1 outline-none flex flex-col">
                {/* SystemGuardBanner now inside main, limited by sidebar width */}
                <SystemGuardBanner />
                {/*
                  2026-05-10: max-w-7xl (1280px) wasted ~340px on each side of a
                  1920px monitor when rendering data-dense tables (/opportunities,
                  /dex-registry, /executions). Bumped to 1800px so wide monitors
                  use their real estate while ultrawide 4K screens still get a
                  centred reading line. Horizontal padding kept at lg:px-10 for
                  breathing room around the sidebar.
                */}
                <div className="mx-auto w-full max-w-[1800px] px-4 py-8 lg:px-10 lg:py-10 flex-1">
                  <PageBreadcrumb />
                  {children}
                </div>
              </main>
            </div>
          </div>
        <OpportunityTicker />

        <Toaster richColors position="top-right" />
      </body>
    </html>
  );
}
