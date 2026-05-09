/**
 * Aurora animated background — Server Component (zero JS shipped).
 * Visible only in dark theme. See globals.css `.aurora-blob-*` for animation.
 * Spec: docs/superpowers/specs/2026-05-08-dark-theme-aurora-glass-design.md
 */
export function AnimatedBg() {
  return (
    <div
      aria-hidden
      className="pointer-events-none fixed inset-0 -z-10 hidden overflow-hidden dark:block"
    >
      <div className="aurora-blob aurora-blob-1" />
      <div className="aurora-blob aurora-blob-2" />
      <div className="aurora-blob aurora-blob-3" />
    </div>
  );
}
