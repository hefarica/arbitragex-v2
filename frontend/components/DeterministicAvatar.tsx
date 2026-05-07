import React from "react";

/**
 * DeterministicAvatar.tsx — Pure SVG avatar derived from a seed string.
 *
 * R1 Mounted Snapshot compliance:
 *   - No Date.now(), no Math.random(), no window/document.
 *   - All values derived from the seed string via pure arithmetic.
 *   - SSR render === CSR render: zero hydration mismatch risk.
 *
 * Produces a circular gradient avatar whose hue, saturation, and lightness
 * are deterministically derived from the seed (typically a token address).
 * Same seed → identical SVG on every render, on every platform.
 */

/** Derive a non-negative integer from a seed string via FNV-1a-inspired fold. */
function seedToInt(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    // Unsigned 32-bit multiply via Math.imul to stay within safe integer range.
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h;
}

interface DeterministicAvatarProps {
  /** Seed for color derivation — typically a token address or strategy id. */
  seed: string;
  /** Tailwind or custom className. Defaults to "size-5 rounded-full". */
  className?: string;
}

export function DeterministicAvatar({
  seed,
  className = "size-5 rounded-full",
}: DeterministicAvatarProps) {
  const hash = seedToInt(seed);

  // Derive HSL components from different bit regions of the hash.
  const hue = hash % 360;
  const saturation = 55 + (hash % 30); // 55–84 — vibrant but not garish
  const lightness = 42 + ((hash >> 8) % 20); // 42–61 — visible on both light/dark bg

  // Highlight color: shift hue by 40° and increase lightness for radial pop.
  const hlHue = (hue + 40) % 360;
  const hlLightness = Math.min(lightness + 22, 85);

  // Stable gradient id derived from seed. Unique per seed value.
  // Using the full hash in hex avoids id collisions between different seeds.
  const gradientId = `da-${hash.toString(16)}`;

  return (
    <svg
      viewBox="0 0 24 24"
      className={className}
      aria-hidden="true"
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        <radialGradient
          id={gradientId}
          cx="35%"
          cy="35%"
          r="65%"
          fx="35%"
          fy="35%"
        >
          <stop
            offset="0%"
            stopColor={`hsl(${hlHue},${saturation}%,${hlLightness}%)`}
          />
          <stop
            offset="100%"
            stopColor={`hsl(${hue},${saturation}%,${lightness}%)`}
          />
        </radialGradient>
      </defs>
      <circle cx="12" cy="12" r="12" fill={`url(#${gradientId})`} />
    </svg>
  );
}
