"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { cn } from "@/lib/utils";
import { NAV_ITEMS, type NavItem } from "@/components/nav-items";

function isActive(pathname: string, item: NavItem): boolean {
  if (item.exact) return pathname === item.href;
  return pathname === item.href || pathname.startsWith(item.href + "/");
}

function NavList({ group, onNavigate }: { group: NavItem["group"]; onNavigate?: () => void }) {
  const pathname = usePathname();
  const items = NAV_ITEMS.filter((i) => i.group === group);
  return (
    <ul className="space-y-0.5">
      {items.map((item) => {
        const Icon = item.icon;
        const active = isActive(pathname, item);
        return (
          <li key={item.href}>
            <Link
              href={item.href}
              onClick={onNavigate}
              className={cn(
                "group flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                "text-sidebar-foreground/70 hover:text-sidebar-foreground hover:bg-sidebar-accent",
                active && "bg-sidebar-accent text-sidebar-foreground shadow-xs",
              )}
              aria-current={active ? "page" : undefined}
            >
              <Icon className={cn("size-4 shrink-0", active && "text-primary")} />
              <span className="truncate">{item.label}</span>
              {active && (
                <span className="ml-auto size-1.5 rounded-full bg-primary" aria-hidden />
              )}
            </Link>
          </li>
        );
      })}
    </ul>
  );
}

export function AppSidebar({ paperMode = true }: { paperMode?: boolean } = {}) {
  return (
    <aside className="hidden lg:flex lg:flex-col lg:gap-6 lg:sticky lg:top-16 lg:self-start lg:h-[calc(100dvh-4rem)] lg:w-64 lg:shrink-0 lg:border-r lg:bg-sidebar lg:px-3 lg:py-6">
      <SidebarContents paperMode={paperMode} />
    </aside>
  );
}

const NAV_SECTIONS: { group: NavItem["group"]; title: string }[] = [
  { group: "pipeline", title: "Pipeline" },
  { group: "control", title: "Risk & Control" },
  { group: "setup", title: "Configuration" },
  { group: "omega", title: "Omega S5" },
];

export function SidebarContents({
  paperMode = true,
  onNavigate,
}: { paperMode?: boolean; onNavigate?: () => void } = {}) {
  return (
    <>
      {NAV_SECTIONS.map(({ group, title }) => (
        <div key={group}>
          <div className="px-3 pb-2 text-[11px] font-semibold uppercase tracking-wider text-sidebar-foreground/50">
            {title}
          </div>
          <NavList group={group} {...(onNavigate ? { onNavigate } : {})} />
        </div>
      ))}
      <div className="mt-auto rounded-md border bg-card/40 p-3 text-xs text-muted-foreground">
        <div className="flex items-center gap-2 font-medium text-foreground">
          <span
            className={cn(
              "size-1.5 rounded-full",
              paperMode ? "bg-success" : "bg-destructive animate-pulse",
            )}
            aria-hidden
          />
          {paperMode ? "paper-mode" : "⚠ LIVE TRADING"}
        </div>
        <p className="mt-1 leading-relaxed">
          {paperMode
            ? "Executions are simulated only. No capital at risk until S9."
            : "LIVE CAPITAL EXECUTION ENABLED. Kill-switch armed."}
        </p>
      </div>
    </>
  );
}
