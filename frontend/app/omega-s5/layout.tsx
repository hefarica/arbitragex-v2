/**
 * OMEGA S5 sibling-route layout — extends the existing shell.
 *
 * Mirror Law: does NOT modify root layout, tailwind, globals.css or any
 * existing component. Only adds a vertical tab strip rendered inside
 * the standard <main> container the parent layout provides.
 *
 * PR-E cluster (2026-10-18): at 390px the row `flex gap-6` kept a 176px
 * `shrink-0` sidebar and a 24px gap, leaving ~190px for the content column —
 * and the cards/tables inside it have a min-content wider than that, so the
 * PAGE scrolled horizontally (measured 93px on the 12 registry entities and
 * 26px on /omega-s5/factory; `body{overflow-x:hidden}` does not stop it because
 * the scroll happens at the documentElement).
 *
 * Fix: stack the row below `lg`. The desktop layout (>=1024px) is identical —
 * same sidebar width, same gap, same borders. Measured, not clipped: no
 * `overflow-x:hidden` is added, so nothing is hidden to pass the gate.
 */
import type { ReactNode } from "react";
import Link from "next/link";

const TABS = [
  { href: "/omega-s5/factory",   label: "Factory" },
  { href: "/omega-s5/wallets",   label: "Wallets" },
  { href: "/omega-s5/core",      label: "Resolution Core" },
  { href: "/omega-s5/adapters",  label: "Adapters" },
  { href: "/omega-s5/crucible",  label: "Crucible" },
  { href: "/omega-s5/operator",  label: "Operator" },
  { href: "/omega-s5/drift",     label: "Drift" },
  { href: "/omega-s5/registry",  label: "Registries" },
] as const;

export default function OmegaS5Layout({ children }: { children: ReactNode }) {
  return (
    <div className="flex flex-col gap-6 lg:flex-row">
      <aside className="w-44 shrink-0 border-r border-border pr-4">
        <h2 className="mb-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          OMEGA S5
        </h2>
        <nav className="flex flex-col gap-1">
          {TABS.map((t) => (
            <Link
              key={t.href}
              href={t.href}
              className="rounded px-2 py-1.5 text-sm hover:bg-muted"
            >
              {t.label}
            </Link>
          ))}
        </nav>
      </aside>
      <section className="flex-1 min-w-0">{children}</section>
    </div>
  );
}
