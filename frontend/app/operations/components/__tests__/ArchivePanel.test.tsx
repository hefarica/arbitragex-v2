// frontend/app/operations/components/__tests__/ArchivePanel.test.tsx
//
// WO-ARCHIVE-401 (2026-09-17) — fail-honest state transition tests.
//
// Repo pattern (RejectionBreakdownPanel.test.tsx): the frontend test env is
// `node` (no jsdom) — the presentational view renders to static HTML via
// react-dom/server and the deterministic branches assert. The pure state
// labels are unit-tested directly.
//
//   - GAP fixed (BROWSE 2026-09-17 §4.3): a failed status poll (edge 401
//     {"error":"missing_admin_token"}) must render an ERROR state, never a
//     perpetual "Cargando estado de archivo…" row.
//   - unknown ≠ empty: with no status the file list must NOT claim
//     "Sin archivos aún." (RULE 00 / R8).
//   - healthy path unchanged: tables render, honest "—" for
//     rows_beyond_window === null, empty-list message only with real status.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { ArchivePanelView, filesStateLabel, retentionStateLabel } from "../ArchivePanel";
import type { ArchiveStatus } from "@/lib/api-client";

const ERR_401 = 'edge HTTP 401: {"error":"missing_admin_token"}';

const STATUS: ArchiveStatus = {
  ok: true,
  kind: "archive_status",
  archive_dir: "/var/lib/arbitragex/archives",
  disk: { total_bytes: 20 * 1024 ** 3, free_bytes: 12 * 1024 ** 3, used_pct: 40 },
  auto_mode: { enabled: true, source: "retention_settings", updated_at: "2026-09-16T04:20:00.000Z", effect: "nightly" },
  export_running: null,
  tables: [
    { table: "opportunities", window_days: 30, rows_beyond_window: 158_322 },
    { table: "simulations", window_days: 14, rows_beyond_window: null },
  ],
  archives: { files: [], total_bytes: 0 },
  min_free_bytes: 15 * 1024 ** 3,
  ts: "2026-09-17T06:20:00.000Z",
};

const noop = () => {};

function renderView(p: Partial<Parameters<typeof ArchivePanelView>[0]>) {
  return renderToStaticMarkup(
    <ArchivePanelView
      status={null}
      error={null}
      busy={false}
      actionMsg={null}
      onToggleAuto={noop}
      onExport={noop}
      {...p}
    />,
  );
}

describe("WO-ARCHIVE-401 — retention table state transition", () => {
  it("401 without admin session → ERROR row with the verbatim reason, never 'Cargando…'", () => {
    const html = renderView({ error: ERR_401 });
    expect(html).toContain("Estado de archivo no disponible");
    expect(html).toContain("missing_admin_token");
    expect(html).not.toContain("Cargando estado de archivo");
    // the failure is announced, not silently styled
    expect(html).toContain('role="alert"');
  });

  it("no status, no error yet → loading label (transient, honest)", () => {
    const html = renderView({});
    expect(html).toContain("Cargando estado de archivo");
    expect(html).not.toContain("Estado de archivo no disponible");
    expect(html).not.toContain('role="alert"');
  });

  it("401 → file list does NOT claim 'Sin archivos aún.' (unknown ≠ empty)", () => {
    const html = renderView({ error: ERR_401 });
    expect(html).not.toContain("Sin archivos aún.");
    expect(html).toContain("Listado de archivos no disponible");
  });

  it("no status, no error → file list says loading, not empty", () => {
    const html = renderView({});
    expect(html).not.toContain("Sin archivos aún.");
    expect(html).toContain("Cargando listado de archivos");
  });
});

describe("WO-ARCHIVE-401 — healthy path unchanged", () => {
  it("renders tables with counts and honest '—' for null rows_beyond_window", () => {
    const html = renderView({ status: STATUS });
    expect(html).toContain("opportunities");
    expect(html).toContain("158,322");
    expect(html).toContain("simulations");
    // R8: null is a dash, never 0
    expect(html).toContain("—");
    expect(html).not.toContain("Cargando estado de archivo");
  });

  it("real status with zero files → 'Sin archivos aún.' (genuine empty)", () => {
    const html = renderView({ status: STATUS });
    expect(html).toContain("Sin archivos aún.");
  });

  it("last good status is kept while a later poll fails (error verbatim at bottom)", () => {
    const html = renderView({ status: STATUS, error: ERR_401 });
    // table rows still render from the stale good status…
    expect(html).toContain("opportunities");
    expect(html).not.toContain("Cargando estado de archivo");
    // …and the failure is surfaced verbatim
    expect(html).toContain("missing_admin_token");
  });
});

describe("state label pure functions", () => {
  it("retentionStateLabel", () => {
    expect(retentionStateLabel(ERR_401)).toBe(
      `Estado de archivo no disponible: ${ERR_401}`,
    );
    expect(retentionStateLabel(null)).toBe("Cargando estado de archivo…");
  });

  it("filesStateLabel", () => {
    expect(filesStateLabel(null, ERR_401)).toBe(
      "Listado de archivos no disponible: estado de archivo inaccesible.",
    );
    expect(filesStateLabel(null, null)).toBe("Cargando listado de archivos…");
    expect(filesStateLabel(STATUS, null)).toBe("Sin archivos aún.");
  });
});
