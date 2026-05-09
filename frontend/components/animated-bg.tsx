/**
 * Aurora animated background — Server Component (zero JS shipped).
 * Visible in both themes; blob colors + opacity controlled via --aurora-* CSS vars
 * defined in :root (light, pale 25%) and .dark (saturated 55%).
 * Spec: docs/superpowers/specs/2026-05-08-dark-theme-aurora-glass-design.md
 */
export function AnimatedBg() {
  return (
    <div
      aria-hidden
      className="pointer-events-none fixed inset-0 -z-10 overflow-hidden"
    >
      <div className="aurora-blob aurora-blob-1" />
      <div className="aurora-blob aurora-blob-2" />
      <div className="aurora-blob aurora-blob-3" />
    </div>
  );
}
