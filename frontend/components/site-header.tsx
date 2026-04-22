"use client";

import { useState } from "react";
import Link from "next/link";
import { MenuIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from "@/components/ui/sheet";
import { SidebarContents } from "@/components/app-sidebar";
import { ThemeToggle } from "@/components/theme-toggle";

export function SiteHeader() {
  const [mobileOpen, setMobileOpen] = useState(false);

  return (
    <header className="sticky top-0 z-40 w-full border-b bg-background/80 backdrop-blur-md supports-[backdrop-filter]:bg-background/60">
      <div className="flex h-16 items-center gap-3 px-4 lg:px-6">
        <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
          <SheetTrigger asChild>
            <Button variant="ghost" size="icon" className="lg:hidden" aria-label="Open navigation">
              <MenuIcon />
            </Button>
          </SheetTrigger>
          <SheetContent side="left" className="p-0">
            <SheetHeader>
              <SheetTitle>ArbitrageX v2</SheetTitle>
            </SheetHeader>
            <div className="flex flex-col gap-6 px-3 pb-6">
              <SidebarContents onNavigate={() => setMobileOpen(false)} />
            </div>
          </SheetContent>
        </Sheet>

        <Link href="/" className="flex items-center gap-2.5">
          <span
            aria-hidden
            className="size-7 rounded-md shadow-sm ring-1 ring-border"
            style={{
              background:
                "conic-gradient(from 210deg, var(--primary), var(--info) 55%, var(--success) 85%, var(--primary))",
            }}
          />
          <div className="flex flex-col -space-y-0.5 leading-none">
            <span className="text-sm font-semibold">ArbitrageX</span>
            <span className="text-[10px] font-medium uppercase tracking-widest text-muted-foreground">
              v2 · operator
            </span>
          </div>
        </Link>

        <Badge variant="info" className="hidden sm:inline-flex">
          <span className="size-1.5 rounded-full bg-info" aria-hidden /> paper-mode
        </Badge>

        <div className="ml-auto flex items-center gap-2">
          <code className="hidden md:inline-flex rounded-md border bg-muted/60 px-2 py-1 text-[11px] text-muted-foreground">
            {process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787"}
          </code>
          <ThemeToggle />
        </div>
      </div>
    </header>
  );
}
