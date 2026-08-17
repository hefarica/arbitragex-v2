import React from "react";

import styles from "./glass-neon.module.css";

/**
 * LedIndicator — pulsing fluorescent status LED (docs/atlas_264 prototype).
 *
 * Two states, mirroring the approved design:
 *   - "live"    → green #00ff88, pulse 2s   (evaluated card · LIVE)
 *   - "pending" → orange #ff6600, pulse 1.5s (detection diagnostic · PENDING)
 *
 * Pure CSS animation (no JS timers, no framer-motion) so hundreds of cards
 * can pulse without re-rendering — and `prefers-reduced-motion` users get a
 * static dot. Decorative by design: the adjacent text label (rendered by
 * DappBadge) carries the state for a11y, hence aria-hidden.
 */
export type LedState = "live" | "pending";

export interface LedIndicatorProps {
  state: LedState;
}

export const LedIndicator = React.memo(function LedIndicator({ state }: LedIndicatorProps) {
  return (
    <span
      aria-hidden="true"
      className={`${styles.led} ${state === "live" ? styles.ledLive : styles.ledPending}`}
    />
  );
});
