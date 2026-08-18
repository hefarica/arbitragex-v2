import React from "react";

import styles from "./glass-neon.module.css";
import { LedIndicator, type LedState } from "./LedIndicator";

/**
 * QUANTUMX_LOGO_SVG_SOURCE — verbatim copy of `frontend/app/icon.svg`.
 *
 * The badge renders it as a data-URI `<img>` exactly like the approved
 * prototype. Stock Next.js client components cannot read the file at build
 * time, so this constant IS the sync point: `__tests__/DappBadge.test.tsx`
 * asserts (against the real file, via node fs) that this string still equals
 * `frontend/app/icon.svg` — if the icon ever changes without updating this
 * constant, CI fails, so every card's logo provably follows icon.svg.
 */
export const QUANTUMX_LOGO_SVG_SOURCE = `<svg width="48" height="48" viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="QuantumX">
  <rect width="48" height="48" rx="11" fill="#0C1230"/>
  <defs>
    <linearGradient id="qx-o" x1="10" y1="10" x2="38" y2="38" gradientUnits="userSpaceOnUse">
      <stop stop-color="#9BC0FF"/>
      <stop offset="0.52" stop-color="#4F7BF7"/>
      <stop offset="1" stop-color="#2742E0"/>
    </linearGradient>
    <radialGradient id="qx-n" cx="0.5" cy="0.45" r="0.6">
      <stop stop-color="#EAF2FF"/>
      <stop offset="0.55" stop-color="#6E9CFF"/>
      <stop offset="1" stop-color="#2C54EE"/>
    </radialGradient>
  </defs>
  <ellipse cx="24" cy="24" rx="15.5" ry="5.6" transform="rotate(45 24 24)" stroke="url(#qx-o)" stroke-width="2.4"/>
  <ellipse cx="24" cy="24" rx="15.5" ry="5.6" transform="rotate(-45 24 24)" stroke="url(#qx-o)" stroke-width="2.4"/>
  <circle cx="24" cy="24" r="3.8" fill="url(#qx-n)"/>
  <circle cx="34.9" cy="13.1" r="1.8" fill="#9BC0FF"/>
  <circle cx="13.1" cy="34.9" r="1.8" fill="#5B86F7"/>
</svg>
`;

/** Data-URI form used by the `<img>` (mirrors the prototype's encoding). */
export const QUANTUMX_LOGO_DATA_URI = `data:image/svg+xml,${encodeURIComponent(
  QUANTUMX_LOGO_SVG_SOURCE,
)}`;

/**
 * DappBadge — full-width QuantumX header badge (docs/atlas_264 prototype).
 *
 * Structure, verbatim from the approved design:
 *   [QuantumX logo] LABEL · STRATEGY NAME          [● LED  LIVE/PENDING]
 *
 *  - Royal-blue neon text (#4169E1) with double text-shadow.
 *  - LED: "live" → green #00ff88 pulse 2s (LIVE) · "pending" → orange
 *    #ff6600 pulse 1.5s (PENDING).
 *  - variant "warn" tints the badge amber/orange for the detection face.
 *
 * Presentational only — no hooks, no time, no window: SSR === CSR (R1).
 */
export interface DappBadgeProps {
  /** Leading state word, e.g. "Evaluada" | "Detección". */
  label: string;
  /** Strategy display name after the separator (may truncate). */
  strategyName: string;
  /** LED state + default label text (LIVE / PENDING). */
  led: LedState;
  /** Override the LED text (defaults to LIVE / PENDING). */
  ledLabel?: string;
  /** "warn" = detection-diagnostic amber variant. */
  variant?: "default" | "warn";
  /** Optional trailing element rendered at the far right (after the LED). */
  trailing?: React.ReactNode;
  /** Extra classes for the badge root (e.g. spacing from the card). */
  className?: string;
}

export const DappBadge = React.memo(function DappBadge({
  label,
  strategyName,
  led,
  ledLabel,
  variant = "default",
  trailing,
  className = "",
}: DappBadgeProps) {
  const isLive = led === "live";
  return (
    <div
      className={`${styles.badge} ${variant === "warn" ? styles.badgeWarn : ""} ${className}`}
    >
      {/* eslint-disable-next-line @next/next/no-img-element -- data-URI asset, no optimization needed */}
      <img
        src={QUANTUMX_LOGO_DATA_URI}
        alt="QuantumX"
        width={22}
        height={22}
        className={styles.badgeLogo}
        draggable={false}
      />
      <span>{label}</span>
      <span className={styles.badgeSep} aria-hidden="true">
        ·
      </span>
      <span className="truncate">{strategyName}</span>
      <span className={styles.ledGroup}>
        <LedIndicator state={led} />
        <span className={isLive ? styles.ledLabelLive : styles.ledLabelPending}>
          {ledLabel ?? (isLive ? "LIVE" : "PENDING")}
        </span>
      </span>
      {trailing}
    </div>
  );
});
