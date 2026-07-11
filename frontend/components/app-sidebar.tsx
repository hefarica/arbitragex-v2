"use client";

import { useState, useEffect, useCallback } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { PanelLeftCloseIcon, PanelLeftOpenIcon } from "lucide-react";

import { cn } from "@/lib/utils";
import { NAV_ITEMS, type NavItem } from "@/components/nav-items";

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
    <ul className={cn("space-y-1", collapsed && "space-y-2")}>
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
                "group relative flex items-center transition-all duration-200",
                collapsed
                  ? "justify-center w-10 h-10 mx-auto rounded-xl"
                  : "gap-3 px-3 py-2.5 rounded-lg",
                // Glassmorphism effect - almost transparent
                "bg-[color-mix(in_oklab,white_3%,transparent)] hover:bg-[color-mix(in_oklab,white_8%,transparent)]",
                active && "bg-[color-mix(in_oklab,oklch(0.62_0.22_263)_15%,transparent)]",
                // Border for definition
                "border border-transparent",
                active
                  ? "border-[color-mix(in_oklab,oklch(0.62_0.22_263)_30%,transparent)]"
                  : "hover:border-[color-mix(in_oklab,white_10%,transparent)]",
                // Text colors
                active
                  ? "text-[oklch(0.85_0.1_263)]"
                  : "text-[color-mix(in_oklab,white_55%,transparent)] hover:text-[color-mix(in_oklab,white_80%,transparent)]",
              )}
              aria-current={active ? "page" : undefined}
              aria-label={collapsed ? item.label : undefined}
            >
              <Icon
                className={cn(
                  "shrink-0 transition-all duration-200",
                  collapsed ? "size-5" : "size-[18px]",
                  active && "text-[oklch(0.75_0.15_263)]",
                )}
              />
              {!collapsed && <span className="text-[13px] font-medium truncate">{item.label}</span>}

              {/* Active indicator dot */}
              {active && collapsed && (
                <span
                  className="absolute -right-0.5 -top-0.5 size-2 rounded-full bg-[oklch(0.62_0.22_263)] shadow-[0_0_6px_oklch(0.62_0.22_263/0.8)]"
                  aria-hidden
                />
              )}

              {/* Credentials badge */}
              {showCredsBadge && (
                <span
                  className={cn(
                    "inline-flex min-w-4 items-center justify-center rounded-full bg-[oklch(0.55_0.18_95)] px-1 text-[10px] font-semibold text-white",
                    collapsed ? "absolute -right-0.5 -bottom-0.5 px-0.5 min-w-3 h-3" : "ml-auto",
                  )}
                  title={`${credsNeedsAttention} credential${credsNeedsAttention === 1 ? "" : "s"} need attention`}
                  aria-label={`${credsNeedsAttention} credentials need attention`}
                >
                  {collapsed ? "" : credsNeedsAttention}
                </span>
              )}

              {/* Active right border indicator (expanded only) */}
              {active && !collapsed && (
                <span
                  className="absolute right-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-l-full bg-[oklch(0.62_0.22_263)] shadow-[0_0_8px_oklch(0.62_0.22_263/0.6)]"
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
        "hidden lg:flex lg:flex-col lg:sticky lg:top-0 lg:self-start lg:h-[calc(100dvh-3.5rem)] lg:shrink-0 lg:z-30",
        // Ultra-thin border
        "lg:border-r lg:border-[color-mix(in_oklab,white_8%,transparent)]",
        // Glassmorphism: almost transparent with strong blur
        "lg:bg-[color-mix(in_oklab,oklch(0.18_0.05_264)_25%,transparent)] lg:backdrop-blur-2xl",
        "lg:supports-[backdrop-filter]:lg:bg-[color-mix(in_oklab,oklch(0.18_0.05_264)_18%,transparent)]",
        collapsed ? "lg:w-[4.5rem]" : "lg:w-[220px]",
        "transition-[width] duration-300 ease-out",
      )}
    >
      {/* Collapse toggle */}
      <button
        type="button"
        onClick={toggleCollapsed}
        aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        className={cn(
          "absolute -right-3 top-5 z-40 grid size-6 place-items-center rounded-full",
          "border border-[color-mix(in_oklab,white_15%,transparent)]",
          "bg-[color-mix(in_oklab,oklch(0.24_0.06_258)_80%,transparent)] backdrop-blur-xl",
          "text-[color-mix(in_oklab,white_60%,transparent)] hover:text-white",
          "hover:bg-[color-mix(in_oklab,oklch(0.62_0.22_263)_30%,transparent)]",
          "transition-all hover:scale-110 active:scale-95 shadow-lg",
        )}
      >
        {collapsed ? <PanelLeftOpenIcon className="size-3" /> : <PanelLeftCloseIcon className="size-3" />}
      </button>

      <div className={cn("flex-1 lg:py-5 overflow-y-auto", collapsed ? "lg:px-2" : "lg:px-3")}>
        <SidebarContents
          paperMode={paperMode}
          credsNeedsAttention={credsNeedsAttention}
          collapsed={collapsed}
        />
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
        <div key={group} className={idx > 0 ? (collapsed ? "pt-4" : "pt-5") : ""}>
          {/* Section title - minimal, only when expanded */}
          {!collapsed && (
            <div className="px-3 pb-2 text-[10px] font-semibold uppercase tracking-[0.15em] text-[color-mix(in_oklab,white_35%,transparent)]">
              {title}
            </div>
          )}
          {/* Separator for collapsed mode */}
          {collapsed && idx > 0 && (
            <div className="mx-auto mb-3 w-6 border-t border-[color-mix(in_oklab,white_10%,transparent)]" aria-hidden />
          )}
          <NavList
            group={group}
            credsNeedsAttention={credsNeedsAttention}
            {...(onNavigate ? { onNavigate } : {})}
            collapsed={collapsed}
          />
        </div>
      ))}

      {/* Paper/Live indicator at bottom */}
      <div
        className={cn(
          "mt-6 rounded-xl border p-2.5",
          collapsed ? "mx-auto w-10 h-10 flex items-center justify-center p-0" : "",
          paperMode
            ? "border-[oklch(0.55_0.18_145/0.4)] bg-[color-mix(in_oklab,oklch(0.55_0.18_145)_10%,transparent)]"
            : "border-[oklch(0.55_0.18_25/0.4)] bg-[color-mix(in_oklab,oklch(0.55_0.18_25)_15%,transparent)]",
        )}
        title={paperMode ? "Paper Mode - No capital at risk" : "⚠ LIVE TRADING ENABLED"}
      >
        <div className={cn("flex items-center gap-2", collapsed && "justify-center")}>
          <span
            className={cn(
              "rounded-full shrink-0",
              collapsed ? "size-2" : "size-1.5",
              paperMode ? "bg-[oklch(0.65_0.16_145)]" : "bg-[oklch(0.65_0.18_25)] animate-pulse",
              paperMode && !collapsed && "shadow-[0_0_6px_oklch(0.65_0.16_145/0.6)]",
            )}
            aria-hidden
          />
          {!collapsed && (
            <span
              className={cn(
                "text-[11px] font-medium",
                paperMode
                  ? "text-[oklch(0.75_0.12_145)]"
                  : "text-[oklch(0.8_0.1_25)]",
              )}
            >
              {paperMode ? "PAPER" : "LIVE"}
            </span>
          )}
        </div>
        {!collapsed && paperMode && (
          <p className="mt-1.5 text-[10px] leading-relaxed text-[color-mix(in_oklab,white_45%,transparent)]">
            Simulated execution
          </p>
        )}
      </div>
    </>
  );
}
