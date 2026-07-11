"use client";

import { useEffect, useState } from "react";

interface StatCardProps {
  label: string;
  value: string | number;
  subtext: string;
  variant?: "default" | "success" | "accent";
  animate?: boolean;
  decimals?: number;
  prefix?: string;
  suffix?: string;
}

export function StatCard({
  label,
  value,
  subtext,
  variant = "default",
  animate = true,
  decimals = 2,
  prefix = "",
  suffix = "",
}: StatCardProps) {
  const [displayValue, setDisplayValue] = useState(animate ? 0 : value);
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);

    if (!animate || typeof value !== "number") {
      setDisplayValue(value);
      return;
    }

    const endValue = value;
    const duration = 1600;
    const startTime = performance.now() + 200;

    const step = (currentTime: number) => {
      const elapsed = currentTime - startTime;
      const progress = Math.min(1, Math.max(0, elapsed / duration));
      const eased = 1 - Math.pow(1 - progress, 4);
      const current = endValue * eased;

      setDisplayValue(decimals > 0 ? current.toFixed(decimals) : Math.round(current));

      if (progress < 1) {
        requestAnimationFrame(step);
      }
    };

    requestAnimationFrame(step);
  }, [value, animate, decimals]);

  const colorClass =
    variant === "success"
      ? "text-[var(--success)]"
      : variant === "accent"
      ? "text-[var(--primary-2)]"
      : "";

  if (!mounted) {
    return (
      <div className="liquid-glass p-5">
        <span className="block font-mono text-[10.5px] tracking-widest uppercase text-[var(--muted)] mb-3">
          {label}
        </span>
        <div className={`text-[38px] font-semibold tracking-tight leading-none ${colorClass}`}>
          {prefix}0{suffix}
        </div>
        <div className="font-mono text-[11px] text-[var(--muted)] mt-2 tracking-wide">{subtext}</div>
      </div>
    );
  }

  return (
    <div className="liquid-glass p-5 relative overflow-hidden">
      <span className="block font-mono text-[10.5px] tracking-widest uppercase text-[var(--muted)] mb-3">
        {label}
      </span>
      <div className={`text-[38px] font-semibold tracking-tight leading-none ${colorClass}`}>
        {prefix}{displayValue}{suffix}
      </div>
      <div className="font-mono text-[11px] text-[var(--muted)] mt-2 tracking-wide">{subtext}</div>
      {animate && <div className="shimmer"></div>}
    </div>
  );
}
