/**
 * User preferences — persisted in localStorage under "arbx_prefs".
 *
 * R1 (Mounted Snapshot Pattern): This module MUST NOT be imported at render
 * time by Server Components. All reads from localStorage happen exclusively
 * inside useEffect in the consuming components.
 *
 * R8 (Fail-Honest): defaults are the authoritative fallback. We never
 * fabricate user intent from an absent or corrupt localStorage entry.
 */

"use client";

import { useEffect, useState, useCallback } from "react";

// ─── Schema ──────────────────────────────────────────────────────────────────

export interface UserPrefs {
  /** Minimum net profit (USD) that triggers a toast notification. */
  notification_threshold_usd: number;
  /** Polling interval in milliseconds for live feeds that support it. */
  polling_interval_ms: number;
  /** Default chain filter (null = all chains). */
  default_chain_id: number | null;
  /** Theme override — 'system' defers to ThemeToggle/localStorage arbx_theme. */
  theme_override: "system" | "light" | "dark";
  /** Compact density for tables (less padding, smaller text). */
  compact_density: boolean;
}

export const DEFAULT_PREFS: UserPrefs = {
  notification_threshold_usd: 50,
  polling_interval_ms: 5_000,
  default_chain_id: null,
  theme_override: "system",
  compact_density: false,
};

const STORAGE_KEY = "arbx_prefs";

// ─── Storage helpers (pure, no side-effects) ─────────────────────────────────

function readPrefs(): UserPrefs {
  // Guard: localStorage does not exist in SSR — callers must only invoke
  // this from inside a useEffect or event handler.
  if (typeof window === "undefined") return DEFAULT_PREFS;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_PREFS;
    const parsed = JSON.parse(raw) as Partial<UserPrefs>;
    // Merge with defaults so new keys added in future releases don't produce
    // undefined when the user has an older version persisted.
    return { ...DEFAULT_PREFS, ...parsed };
  } catch {
    return DEFAULT_PREFS;
  }
}

function writePrefs(prefs: UserPrefs): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    // Private mode / quota exceeded — in-memory state remains correct.
  }
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

/**
 * useUserPrefs — reads arbx_prefs from localStorage on mount (R1 compliant).
 *
 * Returns the current prefs and a stable `savePrefs` function.
 *
 * Server Components: do NOT call this hook. It is marked "use client" at the
 * module boundary via the consuming component's directive.
 */
export function useUserPrefs(): {
  prefs: UserPrefs;
  savePrefs: (next: UserPrefs) => void;
} {
  // R1: initialise with deterministic DEFAULT_PREFS (SSR-safe).
  // localStorage read is deferred to the useEffect below.
  const [prefs, setPrefs] = useState<UserPrefs>(DEFAULT_PREFS);

  useEffect(() => {
    setPrefs(readPrefs());
  }, []);

  const savePrefs = useCallback((next: UserPrefs) => {
    setPrefs(next);
    writePrefs(next);
  }, []);

  return { prefs, savePrefs };
}
