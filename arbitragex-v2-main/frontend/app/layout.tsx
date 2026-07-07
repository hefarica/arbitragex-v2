import type { ReactNode } from "react";
import { headers } from "next/headers";
import { GeistSans } from "geist/font/sans";
import { GeistMono } from "geist/font/mono";

import "./globals.css";
import { SiteHeader } from "@/components/site-header";
import { AppSidebar } from "@/components/app-sidebar";
import { PageBreadcrumb } from "@/components/PageBreadcrumb";
import { HeroSphere } from "@/components/HeroSphere";
// import { AnimatedBg } from "@/components/animated-bg";
import { ThemeScript } from "@/components/theme-toggle";
import { SystemGuardBanner } from "@/components/SystemGuardBanner";
import { Web3Provider } from "@/app/providers/Web3Provider";
import { Toaster } from "sonner";

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

export default async function RootLayout({ children }: { children: ReactNode }) {
  // Forward the request Cookie header to the Web3Provider so wagmi can hydrate
  // its initial connection state on the server (cookieToInitialState), keeping
  // server + client paint in agreement (avoids React #418). headers() is async
  // in Next 15.
  const requestCookie = (await headers()).get("cookie");
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
        <HeroSphere />
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:border focus:bg-background focus:px-3 focus:py-2 focus:text-sm focus:font-medium focus:shadow-md focus:outline-none focus:ring-2 focus:ring-ring"
        >
          Skip to main content
        </a>
        {/*
          Web3Provider (wagmi + react-query + RainbowKit) wraps the whole app so
          the read-only /wallet surface can connect a browser wallet. It exposes
          ONLY read-only connectivity — no signer, no capital, no broadcast.
        */}
        <Web3Provider cookie={requestCookie}>
          <div className="flex min-h-dvh flex-col">
            <SiteHeader />
            <SystemGuardBanner />
            <div className="flex flex-1">
              <AppSidebar />
              <main id="main" tabIndex={-1} className="min-w-0 flex-1 outline-none">
                {/*
                  2026-05-10: max-w-7xl (1280px) wasted ~340px on each side of a
                  1920px monitor when rendering data-dense tables (/opportunities,
                  /dex-registry, /executions). Bumped to 1800px so wide monitors
                  use their real estate while ultrawide 4K screens still get a
                  centred reading line. Horizontal padding kept at lg:px-10 for
                  breathing room around the sidebar.
                */}
                <div className="mx-auto w-full max-w-[1800px] px-4 py-8 lg:px-10 lg:py-10">
                  <PageBreadcrumb />
                  {children}
                </div>
              </main>
            </div>
          </div>
        </Web3Provider>
        <Toaster richColors position="top-right" />
      </body>
    </html>
  );
}
