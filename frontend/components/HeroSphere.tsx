"use client";

import { useEffect, useRef } from "react";

/**
 * HeroSphere — animated particle globe inspired by the aethir.com hero, rendered
 * to a fixed, behind-content <canvas>. A point-cloud sphere (fibonacci
 * distribution) rotates slowly around its axis with a soft radial glow.
 *
 * Theme-aware (operator request):
 *   - DARK theme  → particles render in a LIGHT colour (pale blue-white).
 *   - LIGHT theme → particles render in ROYAL BLUE.
 * The colour is read from the `--sphere-rgb` CSS variable (defined per theme in
 * globals.css) and re-read when the `.dark` class on <html> toggles, so it tracks
 * the theme switch live.
 *
 * Performance: one rAF loop, devicePixelRatio capped at 2, points precomputed,
 * dots drawn as fillRect (cheaper than arc). Honors prefers-reduced-motion by
 * painting a single static frame.
 */

const POINT_COUNT = 1100;

export function HeroSphere() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    // Precompute a unit sphere (fibonacci/golden-angle distribution).
    const pts = new Array<{ x: number; y: number; z: number }>(POINT_COUNT);
    const golden = Math.PI * (3 - Math.sqrt(5));
    for (let i = 0; i < POINT_COUNT; i++) {
      const y = 1 - (i / (POINT_COUNT - 1)) * 2; // 1 → -1
      const r = Math.sqrt(Math.max(0, 1 - y * y));
      const theta = golden * i;
      pts[i] = { x: Math.cos(theta) * r, y, z: Math.sin(theta) * r };
    }

    // Theme colour, read from CSS var "--sphere-rgb" ("r g b"). Falls back to a
    // pale blue (dark-theme default) if the var is missing.
    let rgb = "206 224 255";
    const readColour = () => {
      const v = getComputedStyle(document.documentElement)
        .getPropertyValue("--sphere-rgb")
        .trim();
      if (v) rgb = v;
    };
    readColour();
    const themeObserver = new MutationObserver(readColour);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });

    let dpr = 1;
    let w = 0;
    let h = 0;
    let cx = 0;
    let cy = 0;
    let radius = 0;
    const resize = () => {
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = canvas.clientWidth;
      h = canvas.clientHeight;
      canvas.width = Math.max(1, Math.floor(w * dpr));
      canvas.height = Math.max(1, Math.floor(h * dpr));
      // Globe sits to the right and slightly high — partially off-screen, like
      // the reference composition.
      radius = Math.min(Math.max(w, h) * 0.42, Math.max(w, h));
      cx = w * 0.82;
      cy = h * 0.4;
    };
    resize();
    window.addEventListener("resize", resize);

    let angle = 0;
    let raf = 0;
    const tilt = -0.45; // fixed X-axis tilt

    const render = () => {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);

      // Soft radial glow behind the globe.
      const glow = ctx.createRadialGradient(cx, cy, 0, cx, cy, radius * 1.25);
      glow.addColorStop(0, `rgb(${rgb} / 0.18)`);
      glow.addColorStop(0.55, `rgb(${rgb} / 0.05)`);
      glow.addColorStop(1, `rgb(${rgb} / 0)`);
      ctx.fillStyle = glow;
      ctx.fillRect(0, 0, w, h);

      const cosT = Math.cos(tilt);
      const sinT = Math.sin(tilt);
      const cosA = Math.cos(angle);
      const sinA = Math.sin(angle);
      ctx.fillStyle = `rgb(${rgb})`;

      for (let i = 0; i < pts.length; i++) {
        const p = pts[i]!;
        // rotate around Y
        const x = p.x * cosA - p.z * sinA;
        const zr = p.x * sinA + p.z * cosA;
        // tilt around X
        const y = p.y * cosT - zr * sinT;
        const z = p.y * sinT + zr * cosT;

        const persp = 1 / (2.3 - z * 0.55);
        const sx = cx + x * radius * persp;
        const sy = cy + y * radius * persp;

        const depth = (z + 1) / 2; // 0 back → 1 front
        ctx.globalAlpha = 0.06 + depth * 0.5;
        const size = 0.6 + depth * 1.6;
        ctx.fillRect(sx - size / 2, sy - size / 2, size, size);
      }
      ctx.globalAlpha = 1;

      if (!reduceMotion) {
        angle += 0.0015;
        raf = requestAnimationFrame(render);
      }
    };
    render();

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
      themeObserver.disconnect();
    };
  }, []);

  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
      <canvas ref={canvasRef} className="h-full w-full opacity-70" />
    </div>
  );
}
