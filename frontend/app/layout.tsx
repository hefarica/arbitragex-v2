import type { ReactNode } from "react";

export const metadata = {
  title: "ArbitrageX v2 — Operator Console",
  description: "MEV-grade arbitrage platform operator dashboard",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body style={{
        fontFamily: "system-ui, -apple-system, 'Segoe UI', sans-serif",
        background: "#0b0d10", color: "#e7e9eb", margin: 0, padding: 0,
      }}>
        <header style={{ padding: "12px 20px", borderBottom: "1px solid #1f2937" }}>
          <strong>ArbitrageX v2</strong>{" "}
          <span style={{ color: "#9ca3af" }}>operator console · Sprint 1 (foundations)</span>
        </header>
        <main style={{ padding: 20 }}>{children}</main>
      </body>
    </html>
  );
}
