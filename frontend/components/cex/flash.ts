// frontend/components/cex/flash.ts
//
// WO-PRICE-EXCHANGE-V1 (FE) — CEX-premium "flash on change" primitive.
//
// Binance-style one-shot background flash on a displayed value (green when it
// went up, red when it went down). Design constraints (operator order
// 2026-09-18):
//
//   - Flash ONLY on change: an identical value NEVER re-animates
//     (nextFlashState returns the SAME object when the value is unchanged, so
//     neither the CSS class nor the remount key changes).
//   - Zero global re-renders: the hook keeps its memory in a ref (NOT state),
//     so only the component that owns the value re-renders, and only because
//     its parent already re-rendered it with a new value. The animation itself
//     is pure CSS (`.arbx-flash-up/.arbx-flash-down` in globals.css).
//   - Replay on consecutive same-direction changes: callers spread the `seq`
//     onto the animated element's React `key` — a key change remounts the
//     element and restarts the one-shot animation even when the direction (and
//     therefore the class) did not change.
//   - R1 / SSR-safe: no Date, no window, no Math.random. First observation
//     (prev === null) produces dir=null → deterministic server markup with no
//     flash class; byte-identical across repeated SSR renders.
//   - Ready for `price_delta{token,prev,next,ts,source}`: when the backend
//     mirror emits deltas, flashDirection(prev, next) consumes them verbatim.
import { useRef } from "react";

export type FlashDir = "up" | "down";

/** Per-component flash memory. `seq` increments on every VALUE CHANGE. */
export interface FlashMemory {
  prev: number | null;
  seq: number;
  dir: FlashDir | null;
}

export const INITIAL_FLASH: FlashMemory = { prev: null, seq: 0, dir: null };

function norm(v: number | null | undefined): number | null {
  return v != null && Number.isFinite(v) ? v : null;
}

/**
 * Pure direction of a value transition. null when the change cannot be shown:
 * identical values, a missing side (first observation, dash transitions), or
 * non-finite input. This is the exact seam a future `price_delta`
 * {prev, next} wire event plugs into.
 */
export function flashDirection(
  prev: number | null | undefined,
  next: number | null | undefined,
): FlashDir | null {
  const p = norm(prev);
  const n = norm(next);
  if (p === null || n === null || p === n) return null;
  return n > p ? "up" : "down";
}

/**
 * Pure state step (exported for tests): identical value → the SAME object
 * (no animation), changed value → new memory with seq+1 and the transition's
 * direction.
 */
export function nextFlashState(
  m: FlashMemory,
  value: number | null | undefined,
): FlashMemory {
  const v = norm(value);
  if (v === m.prev) return m;
  return { prev: v, seq: m.seq + 1, dir: flashDirection(m.prev, v) };
}

/** Tailwind-free CSS class for the current flash state (globals.css). */
export function flashClass(m: FlashMemory): string | undefined {
  return m.dir === "up" ? "arbx-flash-up" : m.dir === "down" ? "arbx-flash-down" : undefined;
}

/**
 * Track a displayed value across renders and return its flash state.
 * Ref-backed (no state): a re-render with an UNCHANGED value costs nothing,
 * and the component only re-renders because its value-bearing props changed.
 */
export function useValueFlash(value: number | null | undefined): FlashMemory {
  const ref = useRef<FlashMemory>(INITIAL_FLASH);
  const m = nextFlashState(ref.current, value);
  ref.current = m;
  return m;
}
