#!/usr/bin/env node
/**
 * RunFullSyncCycle FASE 2 — operator macro.
 *
 * "El macro no es un camino nuevo: es la vía manual corrida en lote, con
 * homologación previa." This script reads an operator manifest and drives the
 * SAME pipeline as /settings/credentials: POST /admin/credentials/bulk
 * (dry_run first — homologation table — then, with --apply, the real write),
 * and finally RE-READS the persisted state from the server (fail-honest: the
 * state that counts is the one in Postgres, not the request we sent).
 *
 * Manifest (env ARBX_CRED_MANIFEST — path OUTSIDE this repo, never committed):
 *   {
 *     "items": [
 *       { "provider": "rpc_http", "scope": "chain:1",
 *         "secret_value": "alchemy=https://...,publicnode=https://...,drpc=https://..." },
 *       { "provider": "coingecko_pro", "scope": "global", "secret_value": "CG-..." }
 *     ]
 *   }
 * For rpc_http / rpc_ws the CSV ORDER IS the array order: first entry =
 * titular, the rest = fallbacks in rotation priority. The bulk endpoint
 * validates EVERY entry live and persists the sanitized per-provider
 * breakdown, exactly like the manual Save + validate.
 *
 * Env: ARBX_CRED_MANIFEST (required) · ARBX_ADMIN_TOKEN (required) ·
 *      ARBX_EDGE_URL (default http://localhost:8787 — run on the VPS, or
 *      point it at the public edge).
 *
 * Secrets are NEVER echoed: tables print provider/scope/action/status/message
 * and the sanitized provider breakdown only. Exit 0 = cycle completed (per-row
 * failures are reported in the table — one bad row never blocks the rest);
 * exit 1 = transport/auth/schema failure.
 */
import { readFileSync } from "node:fs";

const DEFAULT_EDGE_URL = "http://localhost:8787";
const MAX_ITEMS = 200;
const MAX_SECRET_LEN = 2048;
const SCOPE_RE = /^(global|chain:\d+)$/;

export const MANIFEST_LIMITS = { MAX_ITEMS, MAX_SECRET_LEN };

/**
 * Validate a manifest object → { ok: true, items } | { ok: false, errors }.
 * Errors carry the item index and a reason — NEVER the secret value.
 * Item order is preserved VERBATIM (for rpc_* CSVs the order is the rotation
 * priority: titular first).
 */
export function parseCredentialManifest(raw) {
  const errors = [];
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
    return { ok: false, errors: ["manifest root must be a JSON object with an `items` array"] };
  }
  const items = raw.items;
  if (!Array.isArray(items) || items.length === 0) {
    return { ok: false, errors: ["manifest.items must be a non-empty array"] };
  }
  if (items.length > MAX_ITEMS) {
    return { ok: false, errors: [`manifest.items has ${items.length} entries — cap is ${MAX_ITEMS}`] };
  }
  const clean = [];
  items.forEach((it, i) => {
    const at = `items[${i}]`;
    if (it === null || typeof it !== "object" || Array.isArray(it)) {
      errors.push(`${at}: must be an object`);
      return;
    }
    if (typeof it.provider !== "string" || it.provider.trim() === "") {
      errors.push(`${at}.provider: non-empty string required`);
    }
    if (typeof it.scope !== "string" || !SCOPE_RE.test(it.scope)) {
      errors.push(`${at}.scope: must match ^(global|chain:\\d+)$ (got scope shape invalid)`);
    }
    if (
      it.secret_value !== undefined &&
      it.secret_value !== null &&
      (typeof it.secret_value !== "string" || it.secret_value.length === 0 || it.secret_value.length > MAX_SECRET_LEN)
    ) {
      errors.push(`${at}.secret_value: must be a 1..${MAX_SECRET_LEN} char string or null/absent`);
    }
    if (it.metadata !== undefined && (it.metadata === null || typeof it.metadata !== "object" || Array.isArray(it.metadata))) {
      errors.push(`${at}.metadata: must be an object`);
    }
    if (it.display_name !== undefined && (typeof it.display_name !== "string" || it.display_name.length === 0)) {
      errors.push(`${at}.display_name: non-empty string required`);
    }
    clean.push({
      provider: it.provider,
      scope: it.scope,
      ...(it.secret_value !== undefined ? { secret_value: it.secret_value } : {}),
      ...(it.display_name !== undefined ? { display_name: it.display_name } : {}),
      metadata: it.metadata ?? {},
    });
  });
  if (errors.length > 0) return { ok: false, errors };
  return { ok: true, items: clean };
}

/** Split an rpc CSV into ordered entries (titular first) — for table display. */
export function csvEntries(secretValue) {
  if (typeof secretValue !== "string") return [];
  return secretValue
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean)
    .map((t, i) => {
      const eq = t.indexOf("=");
      return eq > 0 ? { position: i + 1, name: t.slice(0, eq).trim() } : { position: i + 1, name: `(bare #${i + 1})` };
    });
}

function renderTable(rows) {
  const head = ["#", "provider", "scope", "action", "status", "detail"].map((h) => h.padEnd(12)).join(" ");
  const lines = [head, "-".repeat(head.length)];
  rows.forEach((r, i) => {
    lines.push(
      [
        String(i + 1).padEnd(12),
        String(r.provider).padEnd(12),
        String(r.scope).padEnd(12),
        String(r.action ?? "-").padEnd(12),
        String(r.status ?? "-").padEnd(12),
        (r.error ?? r.message ?? "").slice(0, 60),
      ].join(" "),
    );
    for (const p of r.providers ?? []) {
      lines.push(`    ${p.ok ? "✓" : "✗"} ${p.name} — ${p.detail}`.slice(0, 78));
    }
    // rpc array order (rotation priority) — names only, never URLs.
    if (r.__csv?.length) {
      lines.push(`    order: ${r.__csv.map((e) => e.name).join(" → ")}`);
    }
  });
  return lines.join("\n");
}

async function postBulk(edgeUrl, token, items, dryRun) {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), 300_000);
  try {
    const res = await fetch(`${edgeUrl}/admin/credentials/bulk`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-arbx-admin-token": token,
        "x-arbx-actor": "operator:macro",
      },
      body: JSON.stringify({ items, dry_run: dryRun }),
      signal: ctrl.signal,
    });
    const text = await res.text();
    if (!res.ok) {
      throw new Error(`bulk HTTP ${res.status}: ${text.slice(0, 200)}`);
    }
    return JSON.parse(text);
  } finally {
    clearTimeout(timer);
  }
}

async function fetchPersisted(edgeUrl, token) {
  const res = await fetch(`${edgeUrl}/api/credentials`, {
    headers: { "x-arbx-admin-token": token, accept: "application/json" },
  });
  if (!res.ok) throw new Error(`list HTTP ${res.status}`);
  const data = await res.json();
  return data.items ?? [];
}

async function main() {
  const manifestPath = process.env.ARBX_CRED_MANIFEST;
  const token = process.env.ARBX_ADMIN_TOKEN;
  const edgeUrl = process.env.ARBX_EDGE_URL ?? DEFAULT_EDGE_URL;
  const apply = process.argv.includes("--apply");

  if (!manifestPath) {
    console.error("ERROR: set ARBX_CRED_MANIFEST to the manifest path (outside the repo — never commit it)");
    process.exit(1);
  }
  if (!token) {
    console.error("ERROR: set ARBX_ADMIN_TOKEN");
    process.exit(1);
  }

  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch (e) {
    console.error(`ERROR: cannot read/parse manifest: ${e.message}`);
    process.exit(1);
  }
  const parsed = parseCredentialManifest(manifest);
  if (!parsed.ok) {
    console.error(`ERROR: manifest invalid:\n  - ${parsed.errors.join("\n  - ")}`);
    process.exit(1);
  }

  // Attach the ordered CSV (rotation priority) for display — names only.
  const withCsv = parsed.items.map((it) => ({
    ...it,
    __csv: it.provider === "rpc_http" || it.provider === "rpc_ws" ? csvEntries(it.secret_value) : [],
  }));
  const wireItems = withCsv.map(({ __csv, ...rest }) => rest);

  console.log(`=== RunFullSyncCycle — ${wireItems.length} items · edge=${edgeUrl} · dry_run=true (homologación) ===`);
  const dry = await postBulk(edgeUrl, token, wireItems, true);
  console.log(renderTable(dry.items.map((r, i) => ({ ...r, __csv: withCsv[i].__csv }))));
  console.log(`summary: ${JSON.stringify(dry.summary)}`);

  if (!apply) {
    console.log("\ndry-run only — nothing persisted. Re-run with --apply to write.");
    return;
  }

  console.log("\n=== APPLY (same pipeline as manual Save + validate) ===");
  const applied = await postBulk(edgeUrl, token, wireItems, false);
  console.log(renderTable(applied.items.map((r, i) => ({ ...r, __csv: withCsv[i].__csv }))));
  console.log(`summary: ${JSON.stringify(applied.summary)}`);

  // Fail-honest: the state that counts is the SERVER's. Re-read and report.
  console.log("\n=== Persisted state (re-read from server) ===");
  const persisted = await fetchPersisted(edgeUrl, token);
  const byKey = new Map(persisted.map((r) => [`${r.provider}:${r.scope}`, r]));
  for (const it of wireItems) {
    const row = byKey.get(`${it.provider}:${it.scope}`);
    if (!row) {
      console.log(`  ${it.provider}/${it.scope}: NOT FOUND in server list`);
    } else {
      console.log(`  ${it.provider}/${it.scope}: status=${row.status} updated_at=${row.updated_at} by=${row.updated_by}`);
    }
  }
}

// CLI guard — the module is also imported by unit tests.
if (process.argv[1] && import.meta.url === new URL(`file://${process.argv[1].replace(/\\/g, "/")}`).href) {
  main().catch((e) => {
    console.error(`ERROR: ${e.message}`);
    process.exit(1);
  });
}
