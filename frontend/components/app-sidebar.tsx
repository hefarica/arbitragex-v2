"use client";

import { useState, useEffect, useCallback } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  PanelLeftCloseIcon,
  PanelLeftOpenIcon,
  ChevronDownIcon,
  WorkflowIcon,
  ShieldAlertIcon,
  SettingsIcon,
  SparklesIcon,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@/lib/utils";
import { NAV_ITEMS, type NavItem } from "@/components/nav-items";

function isActive(pathname: string, item: NavItem): boolean {
  if (item.exact) return pathname === item.href;
  return pathname === item.href || pathname.startsWith(item.href + "/");
}

const COLLAPSE_KEY = "arbx:sidebar:collapsed";
// Visual micro-polish: remember which accordion sections the operator closed.
const CLOSED_SECTIONS_KEY = "arbx:sidebar:closedSections";

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

/**
 * Per-section open/close state for the accordion headers. Persisted to
 * localStorage as the set of CLOSED groups (default = all open, so a fresh
 * session sees the full menu — matches pre-polish behaviour).
 */
function useOpenSections(allGroups: string[]): [Set<string>, (g: string) => void] {
  const [closed, setClosed] = useState<Set<string>>(new Set());
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
    try {
      const raw = window.localStorage.getItem(CLOSED_SECTIONS_KEY);
      if (raw) setClosed(new Set(JSON.parse(raw)));
    } catch {
      /* private mode / corrupt JSON → all open */
    }
  }, []);

  const toggle = useCallback((group: string) => {
    setClosed((prev) => {
      const next = new Set(prev);
      if (next.has(group)) next.delete(group);
      else next.add(group);
      try {
        window.localStorage.setItem(CLOSED_SECTIONS_KEY, JSON.stringify([...next]));
      } catch {
        /* non-fatal */
      }
      return next;
    });
  }, []);

  // SSR: render all open (no flash of collapsed sections).
  const effectiveClosed = mounted ? closed : new Set<string>();
  const openSet = new Set(allGroups.filter((g) => !effectiveClosed.has(g)));
  return [openSet, toggle];
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
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/60 focus-visible:ring-offset-1 focus-visible:ring-offset-sidebar",
                collapsed ? "justify-center px-2 py-2.5" : "px-3 py-2 hover:bg-sidebar-accent/60",
                // Active: premium blue glow + inset ring + left vertical accent line.
                active &&
                  !collapsed &&
                  "bg-primary/15 text-foreground border border-primary/30 shadow-[inset_0_0_0_1px_rgba(96,165,250,0.18),0_0_22px_-6px_rgba(37,99,235,0.45)]",
                active && collapsed && "bg-primary/20 text-foreground",
              )}
              aria-current={active ? "page" : undefined}
              aria-label={collapsed ? item.label : undefined}
            >
              {/* Left vertical fluorescent accent line on the active item. */}
              {active && !collapsed && (
                <span
                  aria-hidden
                  className="absolute left-0 top-1.5 bottom-1.5 w-0.5 rounded-r-full bg-primary shadow-[0_0_8px_rgba(59,130,246,0.6)]"
                />
              )}
              <Icon className={cn("size-4 shrink-0 transition-colors", active && "text-primary")} />
              {!collapsed && <span className="truncate">{item.label}</span>}
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

const NAV_SECTIONS: { group: NavItem["group"]; title: string; icon: LucideIcon }[] = [
  { group: "pipeline", title: "Pipeline", icon: WorkflowIcon },
  { group: "control", title: "Risk & Control", icon: ShieldAlertIcon },
  { group: "setup", title: "Configuration", icon: SettingsIcon },
  { group: "omega", title: "Omega S5", icon: SparklesIcon },
];

const SECTION_GROUPS = NAV_SECTIONS.map((s) => s.group);

export function AppSidebar({
  paperMode = true,
  credsNeedsAttention = 0,
}: { paperMode?: boolean; credsNeedsAttention?: number } = {}) {
  const [collapsed, toggleCollapsed] = useSidebarCollapsed();
  const [openSections, toggleSection] = useOpenSections(SECTION_GROUPS);

  return (
    <aside
      className={cn(
        "hidden lg:flex lg:flex-col lg:sticky lg:top-16 lg:self-start lg:h-[calc(100dvh-4rem)] lg:shrink-0 lg:z-30",
        "lg:border-r lg:border-sidebar-border/60",
        // Glassmorphism: semi-transparent sidebar so the gradient backdrop bleeds through.
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
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/60",
        )}
      >
        {collapsed ? <PanelLeftOpenIcon className="size-3.5" /> : <PanelLeftCloseIcon className="size-3.5" />}
      </button>
      <div className={cn("lg:py-6 overflow-y-auto", collapsed ? "lg:px-2" : "lg:px-3")}>
        <SidebarContents
          paperMode={paperMode}
          credsNeedsAttention={credsNeedsAttention}
          collapsed={collapsed}
          openSections={openSections}
          onToggleSection={toggleSection}
        />
      </div>
    </aside>
  );
}

export function SidebarContents({
  paperMode = true,
  credsNeedsAttention = 0,
  onNavigate,
  collapsed = false,
  openSections: openSectionsProp,
  onToggleSection: onToggleSectionProp,
}: {
  paperMode?: boolean;
  credsNeedsAttention?: number;
  onNavigate?: () => void;
  collapsed?: boolean;
  openSections?: Set<string>;
  onToggleSection?: (group: string) => void;
}) {
  // Mobile drawer (site-header) calls SidebarContents without the shared state —
  // fall back to an internal accordion state so both mounts work.
  const [internalOpen, internalToggle] = useOpenSections(SECTION_GROUPS);
  const openSections = openSectionsProp ?? internalOpen;
  const onToggleSection = onToggleSectionProp ?? internalToggle;
  return (
    <>
      {NAV_SECTIONS.map(({ group, title, icon: SectionIcon }, idx) => {
        const open = openSections.has(group);
        return (
          <div key={group} className={idx > 0 ? (collapsed ? "mt-2" : "mt-5") : ""}>
            {collapsed ? (
              // Collapsed: thin separator between sections (no accordion — icons only).
              idx > 0 ? (
                <div className="mx-2 mb-2 border-t border-sidebar-border/40" aria-hidden />
              ) : null
            ) : (
              // Expanded: ACCORDION header — a distinct section button (icon + title +
              // chevron), separated from child items. aria-expanded for a11y.
              <button
                type="button"
                onClick={() => onToggleSection(group)}
                aria-expanded={open}
                title={title}
                className={cn(
                  "group liquid-glass liquid-glass-interactive flex w-full items-center gap-2 rounded-md px-3 py-2 text-[11px] font-semibold uppercase tracking-wider",
                  "text-muted-foreground hover:text-foreground",
                  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/60",
                )}
              >
                <SectionIcon className="size-3.5 shrink-0 text-primary/80 group-hover:text-primary" />
                <span className="flex-1 text-left text-gradient-primary">{title}</span>
                <ChevronDownIcon
                  aria-hidden
                  className={cn(
                    "size-3.5 shrink-0 text-muted-foreground transition-transform duration-200",
                    open ? "" : "-rotate-90",
                  )}
                />
              </button>
            )}

            {/* Submenu: an inset panel that clearly belongs to the header above.
                Rounded + slightly different bg + hairline border = a child surface. */}
            {!collapsed && open && (
              <div className="liquid-glass mt-1 ml-2 space-y-0.5 rounded-lg p-1.5">
                <NavList
                  group={group}
                  credsNeedsAttention={credsNeedsAttention}
                  {...(onNavigate ? { onNavigate } : {})}
                />
              </div>
            )}
            {/* Collapsed: always show the icon list (no accordion collapse in icon mode). */}
            {collapsed && (
              <NavList
                group={group}
                credsNeedsAttention={credsNeedsAttention}
                {...(onNavigate ? { onNavigate } : {})}
                collapsed
              />
            )}
          </div>
        );
      })}
      <div
        className={cn(
          "mt-auto rounded-md border border-border/50 bg-card/40 backdrop-blur-sm p-3 text-xs text-muted-foreground",
          collapsed && "p-2",
        )}
      >
        <div className={cn("flex items-center gap-2 font-medium text-foreground", collapsed && "justify-center")}>
          <span
            className={cn(
              "size-1.5 rounded-full shrink-0",
              paperMode ? "bg-success" : "bg-destructive animate-pulse",
            )}
            aria-hidden
            title={collapsed ? (paperMode ? "paper-mode" : "LIVE TRADING") : undefined}
          />
          {!collapsed && (paperMode ? "paper-mode" : "⚠ LIVE TRADING")}
        </div>
        {!collapsed && (
          <p className="mt-1 leading-relaxed">
            {paperMode
              ? "Executions are simulated only. No capital at risk until S9."
              : "LIVE CAPITAL EXECUTION ENABLED. Kill-switch armed."}
          </p>
        )}
      </div>
    </>
  );
}
