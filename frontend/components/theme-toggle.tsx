"use client";

import { useEffect, useState } from "react";
import { MoonIcon, SunIcon } from "lucide-react";
import { Button } from "@/components/ui/button";

type Theme = "dark" | "light";

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>("dark");

  useEffect(() => {
    const stored = (typeof window !== "undefined" && window.localStorage.getItem("arbx_theme")) as Theme | null;
    const initial: Theme = stored ?? "dark";
    applyTheme(initial);
    setTheme(initial);
  }, []);

  function applyTheme(next: Theme) {
    const root = document.documentElement;
    if (next === "dark") root.classList.add("dark");
    else root.classList.remove("dark");
    window.localStorage.setItem("arbx_theme", next);
  }

  function toggle() {
    const next: Theme = theme === "dark" ? "light" : "dark";
    applyTheme(next);
    setTheme(next);
  }

  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      onClick={toggle}
      aria-label={`Switch to ${theme === "dark" ? "light" : "dark"} mode`}
    >
      {theme === "dark" ? <SunIcon /> : <MoonIcon />}
    </Button>
  );
}

/** Inline script that runs before React hydration to avoid flash-of-wrong-theme. */
export function ThemeScript() {
  const code = `
  (function(){
    try {
      var t = localStorage.getItem('arbx_theme');
      if (!t) t = 'dark';
      if (t === 'dark') document.documentElement.classList.add('dark');
    } catch (e) {}
  })();
  `;
  return <script dangerouslySetInnerHTML={{ __html: code }} />;
}
