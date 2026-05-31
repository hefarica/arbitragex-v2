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
import { cn } from "@/lib/utils";

export function SiteHeader() {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [isMounted, setIsMounted] = useState(false);

  // Hydration safety: getApiBaseUrl() returns different strings on SSR (INTERNAL_EDGE_URL) 
  // vs CSR (NEXT_PUBLIC_EDGE_URL). We must delay rendering it until the client mounts.
  useEffect(() => {
    setIsMounted(true);
  }, []);

  return (
    <header className="sticky top-0 z-40 w-full border-b bg-background/80 backdrop-blur-md supports-[backdrop-filter]:bg-background/60">
      <div className="flex h-16 items-center gap-3 px-4 lg:px-6">
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
              <SheetTitle>QuantumX</SheetTitle>
            </SheetHeader>
            <div className="flex flex-col gap-6 px-3 pb-6">
              <SidebarContents onNavigate={() => setMobileOpen(false)} />
            </div>
          </SheetContent>
        </Sheet>

        <Link href="/" className="flex items-center gap-2.5">
          <span
            aria-hidden
            className="size-7 rounded-md shadow-sm ring-1 ring-border bg-primary"
          />
          <div className="flex flex-col -space-y-0.5 leading-none">
            <span className="text-sm font-semibold">QuantumX</span>
            <span className="text-[10px] font-medium uppercase tracking-widest text-muted-foreground">
              control plane
            </span>
          </div>
        </Link>

        <Badge variant="info" className="hidden sm:inline-flex">
          <span className="size-1.5 rounded-full bg-info" aria-hidden /> ghost-protocol
        </Badge>

        <div className="ml-auto flex items-center gap-2">
          <code className="hidden md:inline-flex rounded-md border bg-muted/60 px-2 py-1 text-[11px] text-muted-foreground">
            {isMounted ? getApiBaseUrl() : "—"}
          </code>
          <ThemeToggle />
        </div>
      </div>
    </header>
  );
}
