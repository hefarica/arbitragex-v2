"use client";

import { useEffect, useRef, type ReactNode } from "react";

/**
 * Focus the wrapped content on mount.
 *
 * Purpose: when a destructive alert renders because the edge failed, screen
 * readers should announce it immediately. We also move keyboard focus onto
 * the alert so a sighted keyboard user doesn't miss it.
 *
 * The wrapper is `tabIndex=-1` (programmatically focusable, not in tab order)
 * and has `outline-none` — the visible focus indicator is provided by the
 * alert itself (destructive border + color).
 */
export function FocusOnMount({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.focus({ preventScroll: false });
  }, []);
  return (
    <div ref={ref} tabIndex={-1} className="outline-none">
      {children}
    </div>
  );
}
