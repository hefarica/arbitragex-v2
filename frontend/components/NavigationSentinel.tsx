"use client";
// BROWSE-FIX-4.6 (2026-09-17) — navigation sentinel.
//
// Context (audits/first-understand-20260917, BROWSE Operador de mesa §4.6):
// after the FIRST load of /opportunities the client router navigated to "/"
// by itself after ~8 s, with no interaction and no console error (1x, not
// reproducible). Exhaustive sweep of this repo (grep over app/ components/
// lib/ features/ hooks/, next.config.js, no middleware.ts) found ZERO code
// paths that navigate to "/" — the only router calls are click-gated
// router.push("/admin/...") and three server-side redirect()s on unrelated
// routes. The navigation therefore originated in the framework's internal
// recovery (App Router 14.2.x prefetch/router-state class), an upstream HTTP
// redirect on a hard document fetch, or the browser/automation layer — all
// UNKNOWN until observed again WITH attribution.
//
// This sentinel makes a recurrence diagnosable instead of anecdotal: it logs
// every same-document navigation with (a) the mechanism (Navigation API
// navigationType, or popstate where the API is absent) and (b) the time since
// the last real user input (pointerdown/keydown). A spontaneous navigation
// will now print e.g.
//   [arbx] nav-sentinel: navigation-api:push → https://host/ (4120ms since last user input)
// which immediately separates "framework did a soft push" from "hard redirect"
// from "user/automation actually interacted".
//
// Rules honored:
// - R1 (Mounted Snapshot): null-render; ALL window access lives inside
//   useEffect — server render is identical to pre-mount client render.
// - RULE 00 / R8: reports ONLY observed mechanism + timing deltas, never
//   invents a cause. Absence of the Navigation API is reported by its absence
//   (popstate fallback), not fabricated.
// - R9 volume: one console.info line per navigation event — navigations are
//   user-rate, this is not a hot loop.
// - Surgical: renders null, touches no store, no layout, no styles.

import { useEffect } from "react";

export interface NavSentinelEntry {
  /** Where the observation came from, e.g. "navigation-api:push" or "popstate". */
  mechanism: string;
  /** Destination as reported by the mechanism (absolute URL or pathname). */
  destination: string;
  /** ms between the last user input (pointerdown/keydown) and the navigation,
   *  or null when no user input was recorded in this page's lifetime. */
  sinceInputMs: number | null;
}

export function formatNavSentinelLine(entry: NavSentinelEntry): string {
  const input =
    entry.sinceInputMs === null
      ? "no user input recorded this page"
      : `${entry.sinceInputMs}ms since last user input`;
  return `[arbx] nav-sentinel: ${entry.mechanism} → ${entry.destination} (${input})`;
}

interface NavigationApiLike {
  addEventListener(type: "navigate", listener: (e: Event) => void): void;
  removeEventListener(type: "navigate", listener: (e: Event) => void): void;
}

/** SSR-safe probe kept separate so tests can exercise the guard logic. */
export function getNavigationApi(w: Window): NavigationApiLike | null {
  const candidate = (w as Window & { navigation?: NavigationApiLike }).navigation;
  return candidate && typeof candidate.addEventListener === "function" ? candidate : null;
}

export function NavigationSentinel(): null {
  useEffect(() => {
    let lastUserInputAt: number | null = null;
    const markInput = () => {
      lastUserInputAt = Date.now();
    };

    const report = (mechanism: string, destination: string) => {
      // eslint-disable-next-line no-console -- diagnostic by design (see header)
      console.info(
        formatNavSentinelLine({
          mechanism,
          destination,
          sinceInputMs: lastUserInputAt === null ? null : Date.now() - lastUserInputAt,
        }),
      );
    };

    // Capture-phase so synthetic/bubbled input still marks "a user was here".
    window.addEventListener("pointerdown", markInput, true);
    window.addEventListener("keydown", markInput, true);

    const navApi = getNavigationApi(window);
    const onNavigate = (e: Event) => {
      // NavigationNavigateEvent shape accessed defensively — the API is
      // Chromium-only and its TS types are not in this repo's lib.
      const ev = e as { destination?: { url?: string }; navigationType?: string };
      report(
        `navigation-api:${ev.navigationType ?? "unknown"}`,
        ev.destination?.url ?? "(destination not reported)",
      );
    };
    if (navApi) navApi.addEventListener("navigate", onNavigate);

    // Fallback + back/forward attribution where the Navigation API is absent.
    const onPop = () => report("popstate", window.location.pathname);
    window.addEventListener("popstate", onPop);

    return () => {
      window.removeEventListener("pointerdown", markInput, true);
      window.removeEventListener("keydown", markInput, true);
      if (navApi) navApi.removeEventListener("navigate", onNavigate);
      window.removeEventListener("popstate", onPop);
    };
  }, []);
  return null;
}

export default NavigationSentinel;
