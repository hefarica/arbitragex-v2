export const PAGE_JUMP = 10;

/**
 * Pure keyboard navigation helper for the DataTable grid.
 *
 * Returns the next row index, or `null` if the key is not handled
 * (caller should let it propagate). When the key IS handled but
 * the index doesn't change (already at top/bottom), the function
 * still returns the current index so the caller can `preventDefault`
 * without moving focus.
 */
export function nextRowIndex(key: string, idx: number, rowCount: number): number | null {
  if (rowCount <= 0) return null;
  switch (key) {
    case "ArrowDown": return Math.min(rowCount - 1, idx + 1);
    case "ArrowUp":   return Math.max(0, idx - 1);
    case "Home":      return 0;
    case "End":       return rowCount - 1;
    case "PageDown":  return Math.min(rowCount - 1, idx + PAGE_JUMP);
    case "PageUp":    return Math.max(0, idx - PAGE_JUMP);
    default:          return null;
  }
}
