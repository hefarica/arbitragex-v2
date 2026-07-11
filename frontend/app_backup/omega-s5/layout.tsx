/**
 * OMEGA S5 sibling-route layout — extends the existing shell.
 *
 * Mirror Law: does NOT modify root layout, tailwind, globals.css or any
 * existing component. Only adds a vertical tab strip rendered inside
 * the standard <main> container the parent layout provides.
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
    <div className="flex gap-6">
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
