#!/usr/bin/env node
/**
 * RDO-ORPHAN-AUDIT-01 (2026-09-26) — read-only audit (optional safe reclaim)
 * for the arbx:route_discovery:outcomes consumer group PEL.
 *
 * Context (2026-09-26 survey): rd-outcome-sink-g0 had ~200k pending entries,
 * oldest 13 days, while PG kept receiving rows — classic ORPHANED-ACK
 * hypothesis (consumers named by HOSTNAME die on every restart; WO-D8 in
 * selector-api documented the same pattern). Blind draining would be wrong;
 * this script classifies with EVIDENCE and only then reclaims.
 *
 * Classification (exact, thanks to the sink's idempotency contract —
 * route-discovery-outcome-sink.ts: ON CONFLICT (stream_id, ts_ms) DO NOTHING):
 *   - PENDING + row EXISTS in PG  → orphan-acked: safe to XACK (a redelivery
 *     could never have duplicated the row; acking just clears stale PEL state).
 *   - PENDING + row MISSING in PG → unpersisted backlog: DO NOT ack (the live
 *     sink must persist it; acking would LOSE the outcome).
 *
 * Usage (run where node >= 20 can reach Redis + PG — VPS host or any checkout
 * with root node_modules after `npm install`):
 *   RDO_DATABASE_URL=postgres://... node scripts/rdo_orphan_audit.mjs            # audit only
 *   RDO_DATABASE_URL=... node scripts/rdo_orphan_audit.mjs --reclaim             # XACK confirmed orphans
 *   node scripts/rdo_orphan_audit.mjs --help
 *
 * Env (all optional, no operator values hardcoded):
 *   RDO_REDIS_URL     default redis://127.0.0.1:6379
 *   RDO_DATABASE_URL  or DATABASE_URL — required (fail-fast, never guessed)
 *   RDO_STREAM        default arbx:route_discovery:outcomes
 *   RDO_GROUP         default rd-outcome-sink-g0
 *   RDO_SAMPLE        default 200 oldest pending entries to classify
 *
 * Exit codes: 0 ok · 1 usage · 2 unreachable deps.
 * READ-ONLY by default. --reclaim issues XACK ONLY for ids proven present in
 * PG, in the same run, after the audit (never blind — R7/R8 discipline).
 */

const args = new Set(process.argv.slice(2));
if (args.has("--help") || args.has("-h")) {
  console.log("rdo_orphan_audit.mjs — classify PEL orphans vs real backlog. See header. Flags: --reclaim");
  process.exit(0);
}
const RECLAIM = args.has("--reclaim");

const REDIS_URL = process.env["RDO_REDIS_URL"] ?? "redis://127.0.0.1:6379";
const DATABASE_URL = process.env["RDO_DATABASE_URL"] ?? process.env["DATABASE_URL"];
const STREAM = process.env["RDO_STREAM"] ?? "arbx:route_discovery:outcomes";
const GROUP = process.env["RDO_GROUP"] ?? "rd-outcome-sink-g0";
const SAMPLE = Math.max(1, Number(process.env["RDO_SAMPLE"] ?? 200) || 200);

if (!DATABASE_URL) {
  console.error("[usage] RDO_DATABASE_URL (or DATABASE_URL) is required — refusing to guess (fail-fast).");
  process.exit(1);
}

const { default: Redis } = await import("ioredis");
const { default: pg } = await import("pg");

const redis = new Redis(REDIS_URL, { maxRetriesPerRequest: 2, connectTimeout: 5000 });
const pool = new pg.Pool({ connectionString: DATABASE_URL, max: 2, connectionTimeoutMillis: 5000 });

// Without these, an unreachable dependency emits an unhandled 'error' event
// that CRASHES the process (exit 1) before the command rejection reaches
// fail2 — caught by the local verification run (2026-09-26).
redis.on("error", () => {});
pool.on("error", () => {});

function fail2(msg) {
  console.error(`[unreachable] ${msg}`);
  process.exitCode = 2;
}

async function main() {
  // 1. PEL summary — the survey numbers, re-read live (fail-honest if empty).
  const summary = await redis.xpending(STREAM, GROUP).catch((e) => {
    fail2(`XPENDING ${STREAM} ${GROUP}: ${e.message}`);
    return null;
  });
  if (!summary) return;
  const [totalPending, minId, maxId] = summary;
  console.log(`stream=${STREAM} group=${GROUP}`);
  console.log(`pending_total=${totalPending} oldest=${minId ?? "null"} newest=${maxId ?? "null"}`);
  if (Number(totalPending) === 0) {
    console.log("verdict=clean nothing_pending");
    return;
  }

  // 2. Oldest N pending ids.
  const rows = await redis.xpendingRange(STREAM, GROUP, "-", "+", SAMPLE).catch((e) => {
    fail2(`XPENDING RANGE: ${e.message}`);
    return null;
  });
  if (!rows) return;
  const ids = rows.map((r) => String(r["id"] ?? r[0]));
  console.log(`sampled=${ids.length}`);

  // 3. Classify against PG in chunks (stream_id is the idempotency key).
  const orphanAcked = [];
  const unpersisted = [];
  const CHUNK = 50;
  for (let i = 0; i < ids.length; i += CHUNK) {
    const chunk = ids.slice(i, i + CHUNK);
    const placeholders = chunk.map((_, j) => `$${j + 1}`).join(",");
    const res = await pool
      .query(`SELECT stream_id FROM route_discovery_outcomes WHERE stream_id IN (${placeholders})`, chunk)
      .catch((e) => {
        fail2(`PG query: ${e.message}`);
        return null;
      });
    if (res === null) return;
    const present = new Set(res.rows.map((r) => r.stream_id));
    for (const id of chunk) (present.has(id) ? orphanAcked : unpersisted).push(id);
  }

  // 4. Verdict + evidence.
  const verdict =
    unpersisted.length === 0 && orphanAcked.length > 0
      ? "orphan_acks_only — reclaim safe (ON CONFLICT DO NOTHING makes redelivery a no-op)"
      : unpersisted.length > 0
        ? `mixed — ${unpersisted.length} unpersisted entries MUST go through the live sink (never blind-ack)`
        : "clean_sample";
  console.log(`orphan_acked=${orphanAcked.length} unpersisted=${unpersisted.length}`);
  console.log(`verdict=${verdict}`);

  // 5. Optional reclaim — only ids PROVEN present in PG, same run.
  if (RECLAIM) {
    if (orphanAcked.length === 0) {
      console.log("reclaim=skipped nothing_to_reclaim");
      return;
    }
    let acked = 0;
    for (const id of orphanAcked) {
      const n = await redis.xack(STREAM, GROUP, id).catch(() => 0);
      acked += Number(n) || 0;
    }
    console.log(`reclaim=applied acked=${acked} of=${orphanAcked.length}`);
  } else if (orphanAcked.length > 0) {
    console.log("reclaim=dry_run rerun with --reclaim to XACK the orphan_acked ids");
  }
}

main()
  .catch((e) => fail2(e.message))
  .finally(async () => {
    try { redis.disconnect(); } catch {}
    try { await pool.end(); } catch {}
  });
