"use client";
import { getApiBaseUrl } from "@/lib/api-client";
import { usePaperModeState } from "@/hooks/usePaperModeState";

import { useState, useEffect } from "react";
import Link from "next/link";
import { MenuIcon } from "lucide-react";

import { buttonVariants } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from "@/components/ui/sheet";
import { SidebarContents } from "@/components/app-sidebar";
import { ThemeToggle } from "@/components/theme-toggle";
import { QuantumLogo } from "@/components/quantum-logo";
import { WebSocketIndicator } from "@/components/WebSocketIndicator";
import { cn } from "@/lib/utils";

export function SiteHeader({ paperMode = true }: { paperMode?: boolean } = {}) {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [isMounted, setIsMounted] = useState(false);
  const paper = usePaperModeState();

  // Hydration safety: getApiBaseUrl() returns different strings on SSR (INTERNAL_EDGE_URL)
  // vs CSR (NEXT_PUBLIC_EDGE_URL). We must delay rendering it until the client mounts.
  useEffect(() => {
    setIsMounted(true);
  }, []);

  const isEnabled = paper.isLoading ? paperMode : paper.data.enabled;
  const hasConflict = paper.data.conflict;
  const isWarning = !isEnabled || hasConflict;

  // Dynamic badge styling
  const badgeBorder = isWarning
    ? "border-[oklch(0.55_0.18_55)]"
    : "border-[oklch(0.55_0.18_145)]";
  const badgeBg = isWarning
    ? "bg-[color-mix(in_oklab,oklch(0.55_0.18_55)_15%,transparent)]"
    : "bg-[color-mix(in_oklab,oklch(0.55_0.18_145)_15%,transparent)]";
  const badgeText = isWarning
    ? "text-[oklch(0.75_0.15_55)]"
    : "text-[oklch(0.75_0.15_145)]";
  const badgeHover = isWarning
    ? "hover:bg-[color-mix(in_oklab,oklch(0.55_0.18_55)_20%,transparent)]"
    : "hover:bg-[color-mix(in_oklab,oklch(0.55_0.18_145)_20%,transparent)]";
  const dotColor = isWarning
    ? "bg-[oklch(0.65_0.16_55)]"
    : "bg-[oklch(0.65_0.16_145)]";

  const badgeLabel = isEnabled ? "PAPER · TLS SHADOW" : "LIVE · CHECK STATE";

  return (
    <header className="sticky top-0 z-40 w-full border-b border-[color-mix(in_oklab,oklch(0.62_0.22_263)_20%,transparent)] bg-[color-mix(in_oklab,oklch(0.18_0.05_264)_85%,transparent)] backdrop-blur-xl">
      <div className="flex h-14 items-center gap-4 px-4 lg:px-6">
        <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
          <SheetTrigger
            className={cn(buttonVariants({ variant: "ghost", size: "icon" }), "lg:hidden")}
            aria-label="Open navigation"
          >
            <MenuIcon />
          </SheetTrigger>
          <SheetContent side="left" className="p-0">
            <SheetHeader>
              <SheetTitle className="flex items-center gap-2">
                <QuantumLogo className="size-5" />
                ARBITRAG<span className="text-primary">E</span>X
              </SheetTitle>
            </SheetHeader>
            <div className="flex flex-col gap-6 px-3 pb-6">
              <SidebarContents paperMode={paperMode} onNavigate={() => setMobileOpen(false)} />
            </div>
          </SheetContent>
        </Sheet>

        {/* Logo */}
        <Link href="/" className="flex items-center gap-2">
          <span
            aria-hidden
            className="grid size-7 place-items-center rounded-full bg-[oklch(0.62_0.22_263)] shadow-[0_0_12px_oklch(0.62_0.22_263/0.5)]"
          >
            <QuantumLogo className="size-4 text-white" />
          </span>
          <span className="text-[13px] font-bold tracking-[0.12em] text-white">
            ARBITRAG<span className="text-[oklch(0.62_0.22_263)]">E</span>X
          </span>
        </Link>

        {/* Center tagline */}
        <div className="hidden lg:flex items-center gap-2 text-[11px] font-medium tracking-[0.15em] text-[color-mix(in_oklab,white_60%,transparent)] uppercase">
          <span>Quantum Research Terminal</span>
          <span className="text-[color-mix(in_oklab,white_30%,transparent)]">·</span>
          <span>Topological Yield Engine</span>
        </div>

        {/* Right side badges */}
        <div className="ml-auto flex items-center gap-2">
          {/* Paper · TLS Shadow Badge */}
          <Badge
            variant="outline"
            className={cn(
              "hidden sm:inline-flex items-center gap-1.5 px-2.5 py-1 h-7 text-[11px] font-medium tracking-wide",
              badgeBorder,
              badgeBg,
              badgeText,
              badgeHover,
            )}
          >
            <span className={cn("size-1.5 rounded-full animate-pulse", dotColor)} aria-hidden />
            {badgeLabel}
          </Badge>

          {/* Kill-Switch Badge */}
          <Badge
            variant="outline"
            className="hidden sm:inline-flex items-center gap-1.5 px-2.5 py-1 h-7 text-[11px] font-medium tracking-wide border-[oklch(0.55_0.15_250)] bg-[color-mix(in_oklab,oklch(0.55_0.15_250)_15%,transparent)] text-[oklch(0.75_0.12_250)] hover:bg-[color-mix(in_oklab,oklch(0.55_0.15_250)_20%,transparent)]"
          >
            <span className="size-1.5 rounded-full bg-[oklch(0.65_0.13_250)]" aria-hidden />
            KILL-SWITCH &lt;10MS
          </Badge>

          <div className="w-px h-5 bg-[color-mix(in_oklab,white_15%,transparent)] mx-1" />

          <WebSocketIndicator />
          <code className="hidden md:inline-flex rounded-md border border-[color-mix(in_oklab,white_15%,transparent)] bg-[color-mix(in_oklab,black_30%,transparent)] px-2 py-1 text-[10px] text-[color-mix(in_oklab,white_50%,transparent)]">
            {isMounted ? getApiBaseUrl() : "—"}
          </code>
          <ThemeToggle />
        </div>
      </div>
    </header>
  );
}
