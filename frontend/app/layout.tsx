import type { ReactNode } from "react";
import { GeistSans } from "geist/font/sans";
import { GeistMono } from "geist/font/mono";

import "./globals.css";
import { SiteHeader } from "@/components/site-header";
import { AppSidebar } from "@/components/app-sidebar";
import { ThemeScript } from "@/components/theme-toggle";

export const metadata = {
  title: "ArbitrageX v2 — Operator Console",
  description: "MEV-grade arbitrage platform operator dashboard",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className={`${GeistSans.variable} ${GeistMono.variable}`} suppressHydrationWarning>
      <head>
        <ThemeScript />
      </head>
      <body className="min-h-dvh bg-background font-sans antialiased">
        <div className="flex min-h-dvh flex-col">
          <SiteHeader />
          <div className="flex flex-1">
            <AppSidebar />
            <main className="min-w-0 flex-1">
              <div className="mx-auto w-full max-w-7xl px-4 py-8 lg:px-10 lg:py-10">
                {children}
              </div>
            </main>
          </div>
        </div>
      </body>
    </html>
  );
}
