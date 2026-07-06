"use client";

import { useEffect, useState } from "react";
import { MoonIcon, SunIcon } from "lucide-react";
import { Button } from "@/components/ui/button";

type Theme = "dark" | "light";

const STORAGE_KEY = "arbx_theme";

function readStoredTheme(): Theme {
  if (typeof window === "undefined") return "dark";
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return stored === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

function applyTheme(next: Theme) {
  const root = document.documentElement;
  if (next === "dark") {
    root.classList.add("dark");
  } else {
    root.classList.remove("dark");
  }
  try {
    window.localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // private mode / quota exceeded — class change still applies in-session
  }
}

export function ThemeToggle() {
  // `mounted` gates icon render to avoid SSR/CSR mismatch flash.
  const [mounted, setMounted] = useState(false);
  const [theme, setTheme] = useState<Theme>("dark");

  useEffect(() => {
    const initial = readStoredTheme();
    setTheme(initial);
    // Re-apply on mount to defend against any DOM state drift between
    // the inline ThemeScript and React hydration.
    applyTheme(initial);
    setMounted(true);
  }, []);

  function toggle() {
    setTheme((prev) => {
      const next: Theme = prev === "dark" ? "light" : "dark";
      applyTheme(next);
      return next;
    });
  }

  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      onClick={toggle}
      aria-label={mounted ? `Switch to ${theme === "dark" ? "light" : "dark"} mode` : "Switch theme"}
      suppressHydrationWarning
    >
      {/* Render placeholder before mount so SSR (always "dark") doesn't paint
          the wrong icon when the user's stored theme is "light". */}
      {mounted ? (
        theme === "dark" ? <SunIcon /> : <MoonIcon />
      ) : (
        <SunIcon className="opacity-0" />
      )}
    </Button>
  );
}

/**
 * Inline script that runs before React hydration to avoid flash-of-wrong-theme.
 * Defensive: explicitly adds OR removes `.dark` so any prior DOM state (from
 * caching, hot reload, or React hydration drift) is normalised to whatever
 * localStorage says. The previous version only added — never removed — which
 * meant `light` could leak through as `dark` if `.dark` was already present.
 */
export function ThemeScript() {
  const code = `
  (function(){
    try {
      var t = localStorage.getItem('arbx_theme');
      var html = document.documentElement;
      if (t === 'light') {
        html.classList.remove('dark');
      } else {
        html.classList.add('dark');
      }
    } catch (e) {}
  })();
  `;
  return <script dangerouslySetInnerHTML={{ __html: code }} />;
}
