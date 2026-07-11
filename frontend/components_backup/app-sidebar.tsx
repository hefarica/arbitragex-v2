"use client";

import { useState, useEffect, useCallback } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { PanelLeftCloseIcon, PanelLeftOpenIcon } from "lucide-react";

import { cn } from "@/lib/utils";
import { NAV_ITEMS, type NavItem } from "@/components/nav-items";

// Status stack component - mockup parity
function StatusStack({ paperMode, collapsed }: { paperMode: boolean; collapsed: boolean }) {
  const [online, setOnline] = useState(true);
  const [latency, setLatency] = useState(12);
  const [lastUpdate, setLastUpdate] = useState(0);
  const [dateStr, setDateStr] = useState("");

  useEffect(() => {
    // Set initial date
    setDateStr(formatDate());

    // Update every second
    const interval = setInterval(() => {
      setDateStr(formatDate());
      setLastUpdate((prev) => prev + 1);
      // Random latency between 8-28ms
      setLatency(Math.floor(Math.random() * 20) + 8);
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  const formatDate = () => {
    const now = new Date();
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')} · ${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}:${String(now.getSeconds()).padStart(2, '0')}`;
  };

  const getUpdateText = () => {
    if (lastUpdate === 0) return 'ahora';
    if (lastUpdate < 60) return `hace ${lastUpdate}s`;
    return `hace ${Math.floor(lastUpdate / 60)}m`;
  };

  // Collapsed view - simple dot
  if (collapsed) {
    return (
      <div className="mt-auto rounded-md border border-border/50 bg-card/40 backdrop-blur-sm p-2">
        <div className="flex items-center justify-center">
          <span
            className={cn(
              "size-1.5 rounded-full shrink-0",
              paperMode ? "bg-success" : "bg-destructive animate-pulse",
            )}
            aria-hidden
          />
        </div>
      </div>
    );
  }

  // Expanded view - full status stack
  return (
    <div className="mt-auto flex flex-col gap-2">
      {/* Status stack panel - mockup parity */}
      <div
        className="rounded-lg border p-3 flex flex-col gap-2"
        style={{
          backgroundColor: 'color-mix(in oklab, var(--foreground) 5%, transparent)',
          borderColor: 'var(--border)',
        }}
      >
        {/* Latency badge row */}
        <div className="flex justify-center">
          <button
            onClick={() => setOnline(!online)}
            className={cn(
              "inline-flex items-center gap-2 rounded-md px-3 py-1.5 transition-all cursor-pointer",
              online && "shadow-[0_0_16px_color-mix(in_oklab,var(--success)_42%,transparent)]",
              !online && "shadow-[0_0_16px_color-mix(in_oklab,var(--destructive)_42%,transparent)]"
            )}
            style={{
              fontFamily: 'var(--font-data)',
              fontSize: '9.5px',
              letterSpacing: '0.14em',
              textTransform: 'uppercase',
              backgroundColor: online
                ? 'color-mix(in oklab, var(--success) 22%, transparent)'
                : 'color-mix(in oklab, var(--destructive) 22%, transparent)',
              color: online ? 'var(--success)' : 'var(--destructive)',
              border: `1px solid ${online
                ? 'color-mix(in oklab, var(--success) 45%, transparent)'
                : 'color-mix(in oklab, var(--destructive) 45%, transparent)'}`,
            }}
          >
            <span
              className={cn(
                "w-[7px] h-[7px] rounded-full",
                online && "animate-pulse"
              )}
              style={{
                backgroundColor: online ? 'var(--success)' : 'var(--destructive)',
                boxShadow: online ? '0 0 10px var(--success)' : '0 0 10px var(--destructive)',
              }}
            />
            <span>{online ? `${latency}ms` : '—'} · {online ? 'ONLINE' : 'OFFLINE'}</span>
          </button>
        </div>

        {/* Date/Time badge */}
        <div
          className="text-center rounded-md px-3 py-2 border"
          style={{
            fontFamily: 'var(--font-data)',
            fontSize: '11px',
            letterSpacing: '0.06em',
            backgroundColor: 'color-mix(in oklab, var(--foreground) 3%, transparent)',
            borderColor: 'color-mix(in oklab, var(--foreground) 7%, transparent)',
            color: 'var(--foreground)',
          }}
        >
          {dateStr || formatDate()}
        </div>

        {/* Capital row */}
        <div className="flex items-baseline justify-between gap-2">
          <span
            style={{
              fontFamily: 'var(--font-data)',
              fontSize: '9px',
              letterSpacing: '0.14em',
              textTransform: 'uppercase',
              color: 'var(--muted)',
            }}
          >
            Capital expuesto
          </span>
          <span
            style={{
              fontFamily: 'var(--font-data)',
              fontSize: '12px',
              fontWeight: 700,
              letterSpacing: '0.04em',
              color: 'var(--success)',
            }}
          >
            $0.00
          </span>
        </div>

        {/* Last update row */}
        <div className="text-center opacity-50 whitespace-nowrap">
          <span
            style={{
              fontFamily: 'var(--font-data)',
              fontSize: '8.5px',
              letterSpacing: '0.14em',
              textTransform: 'uppercase',
              color: 'var(--muted)',
            }}
          >
            ult. actualiza: <span>{getUpdateText()}</span>
          </span>
        </div>
      </div>
    </div>
  );
}

function isActive(pathname: string, item: NavItem): boolean {
  if (item.exact) return pathname === item.href;
  return pathname === item.href || pathname.startsWith(item.href + "/");
}

const COLLAPSE_KEY = "arbx:sidebar:collapsed";

function useSidebarCollapsed(): [boolean, () => void] {
  const [collapsed, setCollapsed] = useState(false);
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
    try {
      const stored = window.localStorage.getItem(COLLAPSE_KEY);
      if (stored === "true") setCollapsed(true);
    } catch {
      /* localStorage blocked (private mode / SSR guard) — default expanded */
    }
  }, []);

  const toggle = useCallback(() => {
    setCollapsed((prev) => {
      const next = !prev;
      try {
        window.localStorage.setItem(COLLAPSE_KEY, String(next));
      } catch {
        /* non-fatal: state lives in memory for the session */
      }
      return next;
    });
  }, []);

  // Until mounted, render expanded to avoid SSR/client mismatch (hydration rule R1).
  const effective = mounted ? collapsed : false;
  return [effective, toggle];
}

function NavList({
  group,
  credsNeedsAttention = 0,
  onNavigate,
  collapsed = false,
}: {
  group: NavItem["group"];
  credsNeedsAttention?: number;
  onNavigate?: () => void;
  collapsed?: boolean;
}) {
  const pathname = usePathname();
  const items = NAV_ITEMS.filter((i) => i.group === group);
  return (
    <ul className="space-y-0.5">
      {items.map((item) => {
        const Icon = item.icon;
        const active = isActive(pathname, item);
        const showCredsBadge = item.href === "/settings/credentials" && credsNeedsAttention > 0;
        return (
          <li key={item.href}>
            <Link
              href={item.href}
              onClick={onNavigate}
              title={collapsed ? item.label : undefined}
              className={cn(
                "group relative flex items-center gap-3 rounded-md text-sm font-medium transition-all",
                "text-sidebar-foreground/70 hover:text-sidebar-foreground",
                collapsed ? "justify-center px-2 py-2.5" : "px-3 py-2 hover:bg-sidebar-accent/60",
                active && "bg-sidebar-accent text-sidebar-foreground shadow-xs",
                active && collapsed && "bg-primary/15",
              )}
              aria-current={active ? "page" : undefined}
              aria-label={collapsed ? item.label : undefined}
            >
              <Icon className={cn("size-4 shrink-0 transition-colors", active && "text-primary")} />
              {!collapsed && <span className="truncate">{item.label}</span>}
              {/* Credentials "needs attention" badge (invalid + untested), server-resolved.
                  Falls back to the active-page dot when there's nothing to flag. */}
              {showCredsBadge ? (
                <span
                  className={cn(
                    "inline-flex min-w-4 items-center justify-center rounded-full bg-warning px-1 text-[10px] font-semibold text-warning-foreground",
                    collapsed ? "absolute right-1 top-1 px-0.5" : "ml-auto",
                  )}
                  title={`${credsNeedsAttention} credential${credsNeedsAttention === 1 ? "" : "s"} need attention`}
                  aria-label={`${credsNeedsAttention} credentials need attention`}
                >
                  {collapsed ? "" : credsNeedsAttention}
                </span>
              ) : active ? (
                !collapsed && <span className="ml-auto size-1.5 rounded-full bg-primary" aria-hidden />
              ) : null}
              {/* Collapsed active indicator: left edge accent bar */}
              {active && collapsed && (
                <span
                  className="absolute left-0 top-1/2 h-6 w-0.5 -translate-y-1/2 rounded-r-full bg-primary"
                  aria-hidden
                />
              )}
            </Link>
          </li>
        );
      })}
    </ul>
  );
}

export function AppSidebar({
  paperMode = true,
  credsNeedsAttention = 0,
}: { paperMode?: boolean; credsNeedsAttention?: number } = {}) {
  const [collapsed, toggleCollapsed] = useSidebarCollapsed();

  return (
    <aside
      className={cn(
        "hidden lg:flex lg:flex-col lg:sticky lg:top-16 lg:self-start lg:h-[calc(100dvh-4rem)] lg:shrink-0 lg:z-30",
        "lg:border-r lg:border-sidebar-border/60",
        // Glassmorphism: semi-transparent sidebar so the aurora background bleeds through.
        // supports-[backdrop-filter] degrades gracefully to solid bg-sidebar on old browsers.
        "lg:bg-sidebar/70 lg:backdrop-blur-xl lg:supports-[backdrop-filter]:lg:bg-sidebar/55",
        collapsed ? "lg:w-[4.5rem]" : "lg:w-64",
        "transition-[width] duration-200 ease-out",
      )}
    >
      {/* Collapse toggle — desktop only, sits at the top of the sidebar */}
      <button
        type="button"
        onClick={toggleCollapsed}
        aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        className={cn(
          "absolute -right-3 top-6 z-40 grid size-6 place-items-center rounded-full",
          "border border-sidebar-border/80 bg-card/90 backdrop-blur-md shadow-md",
          "text-muted-foreground hover:text-foreground hover:bg-accent/80",
          "transition-all hover:scale-105 active:scale-95",
        )}
      >
        {collapsed ? <PanelLeftOpenIcon className="size-3.5" /> : <PanelLeftCloseIcon className="size-3.5" />}
      </button>
      <div className={cn("flex-1 overflow-y-auto lg:py-6", collapsed ? "lg:px-2" : "lg:px-3")}>
        <SidebarContents
          paperMode={paperMode}
          credsNeedsAttention={credsNeedsAttention}
          collapsed={collapsed}
        />
      </div>
      {/* StatusStack outside scroll container - always visible at bottom */}
      <div className={cn("pb-3 shrink-0", collapsed ? "px-2" : "px-3")}>
        <StatusStack paperMode={paperMode} collapsed={collapsed} />
      </div>
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
  credsNeedsAttention = 0,
  onNavigate,
  collapsed = false,
}: { paperMode?: boolean; credsNeedsAttention?: number; onNavigate?: () => void; collapsed?: boolean } = {}) {
  return (
    <>
      {NAV_SECTIONS.map(({ group, title }, idx) => (
        <div key={group} className={idx > 0 && !collapsed ? "pt-5" : idx > 0 ? "pt-4" : ""}>
          {!collapsed && (
            <div className="px-3 pb-2 text-[11px] font-semibold uppercase tracking-wider text-gradient-primary">
              {title}
            </div>
          )}
          {/* Collapsed: thin separator line between sections */}
          {collapsed && idx > 0 && (
            <div className="mx-2 mb-2 border-t border-sidebar-border/40" aria-hidden />
          )}
          <NavList
            group={group}
            credsNeedsAttention={credsNeedsAttention}
            {...(onNavigate ? { onNavigate } : {})}
            collapsed={collapsed}
          />
        </div>
      ))}
    </>
  );
}
