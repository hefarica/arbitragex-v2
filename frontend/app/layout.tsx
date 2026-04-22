import type { ReactNode } from "react";
import Link from "next/link";

export const metadata = {
  title: "ArbitrageX v2 — Operator Console",
  description: "MEV-grade arbitrage platform operator dashboard",
};

const NAV: Array<{ href: string; label: string }> = [
  { href: "/",              label: "home" },
  { href: "/status",        label: "status" },
  { href: "/opportunities", label: "opportunities" },
  { href: "/executions",    label: "executions" },
  { href: "/risk",          label: "risk & alerts" },
  { href: "/recon",         label: "recon & PnL" },
  { href: "/config",        label: "config" },
  { href: "/killswitch",    label: "kill-switch" },
];

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body style={{
        fontFamily: "system-ui, -apple-system, 'Segoe UI', sans-serif",
        background: "#0b0d10", color: "#e7e9eb", margin: 0, padding: 0,
      }}>
        <header style={{ padding: "12px 20px", borderBottom: "1px solid #1f2937" }}>
          <strong>ArbitrageX v2</strong>{" "}
          <span style={{ color: "#9ca3af" }}>operator console</span>
        </header>
        <div style={{ display: "grid", gridTemplateColumns: "200px 1fr", minHeight: "calc(100vh - 46px)" }}>
          <nav style={{ borderRight: "1px solid #1f2937", padding: "16px 0" }}>
            <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
              {NAV.map((n) => (
                <li key={n.href}>
                  <Link href={n.href} style={{
                    display: "block", padding: "8px 20px", color: "#e7e9eb", textDecoration: "none",
                    fontSize: 14,
                  }}>
                    {n.label}
                  </Link>
                </li>
              ))}
            </ul>
          </nav>
          <main style={{ padding: 20 }}>{children}</main>
        </div>
      </body>
    </html>
  );
}
