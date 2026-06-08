import * as React from "react";

/**
 * QuantumX brand mark.
 *
 * Two crossing quantum orbitals form an "X" around a glowing nucleus — the
 * "Quantum" (atomic orbits) and the "X" of the name in a single geometric
 * monogram. Carries its own electric royal-blue gradient (matched to the
 * --primary token), so it reads on any surface. Scalable; set size via
 * className (e.g. `size-7`).
 */
export function QuantumLogo({
  className,
  title = "QuantumX",
}: {
  className?: string;
  title?: string;
}) {
  // Unique gradient ids so multiple instances on a page don't collide.
  const uid = React.useId();
  const orbit = `qx-orbit-${uid}`;
  const core = `qx-core-${uid}`;
  return (
    <svg
      viewBox="0 0 48 48"
      role="img"
      aria-label={title}
      className={className}
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        <linearGradient id={orbit} x1="7" y1="7" x2="41" y2="41" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#9BC0FF" />
          <stop offset="0.52" stopColor="#4F7BF7" />
          <stop offset="1" stopColor="#2742E0" />
        </linearGradient>
        <radialGradient id={core} cx="0.5" cy="0.45" r="0.6">
          <stop offset="0" stopColor="#EAF2FF" />
          <stop offset="0.55" stopColor="#6E9CFF" />
          <stop offset="1" stopColor="#2C54EE" />
        </radialGradient>
      </defs>

      {/* quantum orbitals — crossing diagonals form the X */}
      <ellipse
        cx="24"
        cy="24"
        rx="20"
        ry="7.2"
        transform="rotate(45 24 24)"
        stroke={`url(#${orbit})`}
        strokeWidth="2.6"
      />
      <ellipse
        cx="24"
        cy="24"
        rx="20"
        ry="7.2"
        transform="rotate(-45 24 24)"
        stroke={`url(#${orbit})`}
        strokeWidth="2.6"
      />

      {/* nucleus */}
      <circle cx="24" cy="24" r="4.7" fill={`url(#${core})`} />

      {/* electrons on the orbital tips */}
      <circle cx="38.1" cy="9.9" r="2.15" fill="#9BC0FF" />
      <circle cx="9.9" cy="38.1" r="2.15" fill="#5B86F7" />
    </svg>
  );
}

export default QuantumLogo;
