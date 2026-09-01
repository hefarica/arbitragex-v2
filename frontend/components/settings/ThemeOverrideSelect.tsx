"use client";

import * as React from "react"; // SSR-test classic JSX runtime (house convention, see RuntimePostureBar)

import type { UserPrefs } from "@/lib/user-prefs";

/**
 * DAPP-SURFACE (2026-09-01B): native <select> for the theme override —
 * deliberately NOT Radix Select. Radix renders a hidden native-select proxy
 * (aria-hidden, tabindex=-1, no accessible name, no API to name it) whenever
 * its trigger sits inside a native <form> and by default during SSR, so the
 * audit's control census counted an unlabeled control on /settings. A real
 * <select> with id + <Label htmlFor> and SSR-rendered <option> children is
 * labeled at every lifecycle instant (aria/id/label and non-empty text).
 * Presentational + controlled; state stays in SettingsClient (R1).
 */
export function ThemeOverrideSelect({
  id = "theme_override",
  value,
  disabled,
  onChange,
  className,
}: {
  id?: string;
  value: UserPrefs["theme_override"];
  disabled?: boolean;
  onChange: (value: UserPrefs["theme_override"]) => void;
  className?: string;
}) {
  return (
    <select
      id={id}
      disabled={disabled}
      value={value}
      onChange={(e) => onChange(e.target.value as UserPrefs["theme_override"])}
      className={
        className ??
        "flex h-9 max-w-xs items-center rounded-md border border-input bg-transparent px-3 py-2 text-sm whitespace-nowrap shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-input/30"
      }
    >
      <option value="system">System (follow OS / arbx_theme)</option>
      <option value="dark">Dark</option>
      <option value="light">Light</option>
    </select>
  );
}
