import type { ReactNode } from "react";
import Link from "next/link";
import { Poppins, Lora, JetBrains_Mono } from "next/font/google";
import "./globals.css";

const poppins = Poppins({
  subsets: ["latin"],
  weight: ["300", "400", "500", "600", "700"],
  display: "swap",
  variable: "--font-poppins",
});
const lora = Lora({
  subsets: ["latin"],
  weight: ["400", "500", "600", "700"],
  style: ["normal", "italic"],
  display: "swap",
  variable: "--font-lora",
});
const jetMono = JetBrains_Mono({
  subsets: ["latin"],
  weight: ["400", "500"],
  display: "swap",
  variable: "--font-mono",
});

export const metadata = {
  title: "ArbitrageX v2 — Operator Console",
  description: "MEV-grade arbitrage platform operator dashboard",
};

const NAV_OBS: Array<{ href: string; label: string }> = [
  { href: "/",              label: "Home" },
  { href: "/status",        label: "System status" },
  { href: "/opportunities", label: "Opportunities" },
  { href: "/executions",    label: "Executions" },
  { href: "/risk",          label: "Risk & alerts" },
  { href: "/recon",         label: "Recon & PnL" },
];
const NAV_CTRL: Array<{ href: string; label: string }> = [
  { href: "/config",     label: "Config" },
  { href: "/killswitch", label: "Kill-switch" },
];

export default function RootLayout({ children }: { children: ReactNode }) {
  const fontClasses = `${poppins.variable} ${lora.variable} ${jetMono.variable}`;
  return (
    <html lang="en" className={fontClasses}>
      <body>
        <header className="hdr">
          <div className="hdr__brand">
            <span className="hdr__logo" aria-hidden />
            <span>ArbitrageX <span className="faint">v2</span></span>
          </div>
          <span className="hdr__sub hide-sm">operator console</span>
          <div className="hdr__right">
            <span className="badge badge--accent">paper-mode</span>
          </div>
        </header>
        <div className="shell">
          <nav className="nav" aria-label="Primary">
            <div className="nav__section">Observe</div>
            <ul className="nav__list">
              {NAV_OBS.map((n) => (
                <li key={n.href}>
                  <Link href={n.href} className="nav__item">
                    <span className="nav__dot" aria-hidden />
                    {n.label}
                  </Link>
                </li>
              ))}
            </ul>
            <div className="nav__section mt-6">Control</div>
            <ul className="nav__list">
              {NAV_CTRL.map((n) => (
                <li key={n.href}>
                  <Link href={n.href} className="nav__item">
                    <span className="nav__dot" aria-hidden />
                    {n.label}
                  </Link>
                </li>
              ))}
            </ul>
          </nav>
          <main className="main">{children}</main>
        </div>
      </body>
    </html>
  );
}
