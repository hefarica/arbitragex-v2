"use client";
import { getApiBaseUrl } from "@/lib/api-client";

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

function PaperBadge({ active }: { active: boolean }) {
  return (
    <span
      className="hidden sm:inline-flex items-center gap-2"
      style={{
        fontFamily: 'var(--font-data)',
        fontSize: '10.5px',
        letterSpacing: '0.14em',
        textTransform: 'uppercase',
        padding: '6px 11px',
        borderRadius: '6px',
        backgroundColor: active
          ? 'color-mix(in oklab, var(--success) 16%, transparent)'
          : 'color-mix(in oklab, var(--destructive) 16%, transparent)',
        color: active ? 'var(--success)' : 'var(--destructive)',
        border: `1px solid ${active
          ? 'color-mix(in oklab, var(--success) 28%, transparent)'
          : 'color-mix(in oklab, var(--destructive) 28%, transparent)'}`,
        whiteSpace: 'nowrap',
      }}
    >
      <span
        style={{
          width: '6px',
          height: '6px',
          borderRadius: '50%',
          backgroundColor: active ? 'var(--success)' : 'var(--destructive)',
          boxShadow: active ? '0 0 10px var(--success)' : '0 0 10px var(--destructive)',
        }}
        aria-hidden
      />
      Paper · TLS Shadow
    </span>
  );
}

function KillSwitchBadge() {
  return (
    <span
      className="hidden md:inline-flex items-center gap-2"
      style={{
        fontFamily: 'var(--font-data)',
        fontSize: '10.5px',
        letterSpacing: '0.14em',
        textTransform: 'uppercase',
        padding: '6px 11px',
        borderRadius: '6px',
        backgroundColor: 'color-mix(in oklab, var(--primary) 16%, transparent)',
        color: 'var(--primary)',
        border: '1px solid color-mix(in oklab, var(--primary) 30%, transparent)',
        whiteSpace: 'nowrap',
      }}
    >
      <span
        style={{
          width: '6px',
          height: '6px',
          borderRadius: '50%',
          backgroundColor: 'var(--primary)',
          boxShadow: '0 0 10px var(--primary)',
        }}
        aria-hidden
      />
      Kill-switch &lt;10ms
    </span>
  );
}

export function SiteHeader({ paperMode = true }: { paperMode?: boolean } = {}) {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [isMounted, setIsMounted] = useState(false);

  // Hydration safety: getApiBaseUrl() returns different strings on SSR (INTERNAL_EDGE_URL) 
  // vs CSR (NEXT_PUBLIC_EDGE_URL). We must delay rendering it until the client mounts.
  useEffect(() => {
    setIsMounted(true);
  }, []);

  return (
    <header
      className="sticky top-0 z-40 w-full border-b backdrop-blur-[14px] saturate-[1.3]"
      style={{
        backgroundColor: 'var(--header-bg)',
        borderColor: 'var(--border)',
        padding: '18px 40px',
      }}
    >
      <div className="flex h-[calc(64px-36px)] items-center gap-6">
        <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
          {/*
           * NOTE: SheetTrigger WITHOUT asChild — Radix Dialog.Trigger already
           * renders a native <button>. Styling it via buttonVariants() gives
           * identical visuals with zero Slot/SlotClone composition, which was
           * the dev-time source of "Function components cannot be given refs"
           * (stack: Button <- SlotClone <- SheetTrigger) even after Button
           * was wrapped in React.forwardRef. Native button = no ref
           * forwarding path = no warning, guaranteed.
           */}
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
                Quantum<span className="text-primary">X</span>
              </SheetTitle>
            </SheetHeader>
            <div className="flex flex-col gap-6 px-3 pb-6">
              <SidebarContents paperMode={paperMode} onNavigate={() => setMobileOpen(false)} />
            </div>
          </SheetContent>
        </Sheet>

        <Link href="/" className="flex items-center gap-2.5">
          <div className="flex flex-col -space-y-0.5 leading-none">
            <span
              className="text-[18px] font-bold tracking-[-0.02em]"
              style={{ fontFamily: 'var(--font-sans)' }}
            >
              ARBITRAG<span style={{ color: 'var(--primary)', textShadow: 'var(--wordmark-glow)' }}>E</span>X
            </span>
            <span
              className="text-[9.5px] uppercase tracking-[0.18em] text-muted-foreground"
              style={{
                fontFamily: 'var(--font-data)',
                borderLeft: '1px solid var(--border)',
                paddingLeft: '1rem',
                marginLeft: '-0.25rem',
              }}
            >
              Quantum Research Terminal · Topological Yield Engine
            </span>
          </div>
        </Link>

        <div className="hidden sm:flex items-center gap-2">
          <PaperBadge active={paperMode} />
          <KillSwitchBadge />
        </div>

        <div className="ml-auto flex items-center gap-2">
          <WebSocketIndicator />
          <code className="hidden md:inline-flex rounded-md border bg-muted/60 px-2 py-1 text-[11px] text-muted-foreground">
            {isMounted ? getApiBaseUrl() : "—"}
          </code>
          <ThemeToggle />
        </div>
      </div>
    </header>
  );
}
