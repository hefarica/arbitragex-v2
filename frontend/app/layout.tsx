import type { ReactNode } from "react";
import { GeistSans } from "geist/font/sans";
import { GeistMono } from "geist/font/mono";

import "./globals.css";
import { SiteHeader } from "@/components/site-header";
import { AppSidebar } from "@/components/app-sidebar";
import { AnimatedBg } from "@/components/animated-bg";
import { ThemeScript } from "@/components/theme-toggle";
import { Toaster } from "sonner";

export const metadata = {
  title: "ArbitrageX v2 — Operator Console",
  description: "MEV-grade arbitrage platform operator dashboard",
  // Block automatic translation by Chrome/Edge/Firefox/Safari on mobile.
  // Auto-translation replaces text nodes in the DOM, which breaks React
  // reconciliation with `removeChild on Node` errors when state updates.
  // See https://github.com/facebook/react/issues/11538
  other: {
    google: "notranslate",
  },
};

export default function RootLayout({ children }: { children: ReactNode }) {
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
        <AnimatedBg />
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:border focus:bg-background focus:px-3 focus:py-2 focus:text-sm focus:font-medium focus:shadow-md focus:outline-none focus:ring-2 focus:ring-ring"
        >
          Skip to main content
        </a>
        <div className="flex min-h-dvh flex-col">
          <SiteHeader />
          <div className="flex flex-1">
            <AppSidebar />
            <main id="main" tabIndex={-1} className="min-w-0 flex-1 outline-none">
              <div className="mx-auto w-full max-w-7xl px-4 py-8 lg:px-10 lg:py-10">
                {children}
              </div>
            </main>
          </div>
        </div>
        <Toaster richColors position="top-right" />
      </body>
    </html>
  );
}
