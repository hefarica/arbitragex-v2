"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { ChevronRightIcon, HouseIcon } from "lucide-react";

import { NAV_ITEMS } from "@/components/nav-items";
import { cn } from "@/lib/utils";

/**
 * PageBreadcrumb — orientation trail for nested routes.
 *
 * Labels are sourced from NAV_ITEMS so a crumb reads identically to its sidebar
 * entry (lexicon included); unknown/dynamic segments fall back to a humanised
 * version of the path slug. Intermediate segments that are NOT real navigable
 * routes (e.g. /omega-s5, /paper) render as plain text rather than dead links.
 * Hidden on the home route.
 *
 * Client component: usePathname is request-derived on the server in the App
 * Router, so the SSR and first CSR paint agree — no hydration risk. Pure
 * render: no Date.now()/window/Math.random().
 */

const HREF_LABEL: Record<string, string> = Object.fromEntries(
  NAV_ITEMS.map((i) => [i.href, i.label]),
);

function humanise(segment: string): string {
  let s: string;
  try {
    s = decodeURIComponent(segment);
  } catch {
    s = segment;
  }
  return s
    .split("-")
    .map((w) => (w ? w.charAt(0).toUpperCase() + w.slice(1) : w))
    .join(" ");
}

export function PageBreadcrumb() {
  const pathname = usePathname();
  if (!pathname || pathname === "/") return null;

  const segments = pathname.split("/").filter(Boolean);
  const crumbs = segments.map((seg, i) => {
    const href = "/" + segments.slice(0, i + 1).join("/");
    const isLast = i === segments.length - 1;
    return {
      href,
      isLast,
      label: HREF_LABEL[href] ?? humanise(seg),
      // Only intermediate crumbs that correspond to a real nav route are links;
      // avoids breadcrumb links that would 404 (e.g. /omega-s5, /paper).
      linkable: !isLast && href in HREF_LABEL,
    };
  });

  return (
    <nav
      aria-label="Breadcrumb"
      className="mb-6 flex flex-wrap items-center gap-1 text-xs text-muted-foreground"
    >
      <Link
        href="/"
        aria-label="Home"
        className="inline-flex items-center transition-colors hover:text-foreground"
      >
        <HouseIcon className="size-3.5" />
      </Link>
      {crumbs.map((crumb) => (
        <span key={crumb.href} className="flex items-center gap-1">
          <ChevronRightIcon className="size-3 shrink-0 opacity-60" aria-hidden />
          {crumb.linkable ? (
            <Link
              href={crumb.href}
              className="transition-colors hover:text-foreground"
            >
              {crumb.label}
            </Link>
          ) : (
            <span
              className={cn(crumb.isLast && "font-medium text-foreground")}
              {...(crumb.isLast ? { "aria-current": "page" as const } : {})}
            >
              {crumb.label}
            </span>
          )}
        </span>
      ))}
    </nav>
  );
}
