/**
 * Pure client-side CSV/JSON export utilities.
 * Called from onClick handlers only — never during render.
 * R1: no Date.now() or window access at module scope.
 */

function escapeCell(v: unknown): string {
  const s = v == null ? "" : String(v);
  // Wrap in quotes if it contains comma, newline, or quote; escape inner quotes.
  if (s.includes(",") || s.includes("\n") || s.includes('"')) {
    return `"${s.replace(/"/g, '""')}"`;
  }
  return s;
}

export function exportToCSV<T extends Record<string, unknown>>(
  rows: T[],
  filename: string,
): void {
  if (rows.length === 0) return;
  const first = rows[0];
  if (!first) return;
  const headers = Object.keys(first);
  const lines = [
    headers.join(","),
    ...rows.map((r) => headers.map((h) => escapeCell(r[h])).join(",")),
  ];
  const blob = new Blob([lines.join("\n")], { type: "text/csv;charset=utf-8;" });
  triggerDownload(blob, filename);
}

export function exportToJSON<T>(rows: T[], filename: string): void {
  if (!Array.isArray(rows) || rows.length === 0) return;
  const blob = new Blob([JSON.stringify(rows, null, 2)], {
    type: "application/json;charset=utf-8;",
  });
  triggerDownload(blob, filename);
}

function triggerDownload(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.style.display = "none";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  // Revoke after a tick to allow the download to start.
  setTimeout(() => URL.revokeObjectURL(url), 100);
}
