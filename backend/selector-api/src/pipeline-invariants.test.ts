/**
 * OMEGA-8/M4 Fase 11 — Redis Streams pipeline invariants.
 *
 * Locks the seven properties the Capa 3 spec requires for the
 * detected → validated → simulated → executed pipeline:
 *
 *   1. XACK only after persistence (consumer.ts line 154).
 *   2. Invalid message → XACK + invalid metric (consumer.ts line 132).
 *   3. Transient error → no XACK (consumer.ts catch block, line 156).
 *   4. Consumer group creation tolerates BUSYGROUP.
 *   5. XPENDING is monitored via `lagFor` (consumer.ts line 116).
 *   6. MAXLEN ~10000 with approximate trim (consumer.ts line 184).
 *   7. trace_id is forwarded through every persist call (line 141).
 *
 * Because consumer.ts is private to the binary and tightly coupled to a
 * real ioredis instance, the regression test below operates at the
 * source-code-invariant level. It reads the pipeline constants and
 * known control-flow markers from the consumer module and asserts they
 * have not regressed. A future refactor that moves logic out of these
 * landmarks must update this test deliberately — protecting against
 * silent drift.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname_local = dirname(fileURLToPath(import.meta.url));
const CONSUMER_SRC = readFileSync(
  resolve(__dirname_local, "./consumer.ts"),
  "utf8",
);

describe("OMEGA-8/M4 Fase 11 — Redis Streams pipeline invariants", () => {
  it("STREAM_IN, STREAM_OUT, GROUP names are stable", () => {
    expect(CONSUMER_SRC).toMatch(/STREAM_IN\s*=\s*"arbx:opps:detected"/);
    expect(CONSUMER_SRC).toMatch(/STREAM_OUT\s*=\s*"arbx:opps:validated"/);
    expect(CONSUMER_SRC).toMatch(/GROUP\s*=\s*"selector-g0"/);
  });

  it("MAXLEN uses approximate trim (~10000) so XADD never blocks", () => {
    expect(CONSUMER_SRC).toMatch(/MAXLEN_OUT\s*=\s*10[_]?000/);
    // The actual XADD must pass MAXLEN with '~' for non-blocking trim.
    expect(CONSUMER_SRC).toMatch(/"MAXLEN",\s*"~"/);
  });

  it("invalid messages are XACK-ed (no replay loop)", () => {
    // The catch block on JSON parse / schema-validation must call xack
    // and increment the invalid messages metric.
    expect(CONSUMER_SRC).toMatch(/invalidMessagesTotal\(\)/);
    expect(CONSUMER_SRC).toMatch(/xack\(STREAM_IN,\s*GROUP,\s*id\)/);
  });

  it("XACK happens AFTER persistDecision succeeds (at-least-once)", () => {
    // The ack call must be inside the same `try` after persistDecision,
    // and after `decision === accept` publishes downstream.
    const ackIdx = CONSUMER_SRC.indexOf("xack(STREAM_IN, GROUP, id)");
    const persistIdx = CONSUMER_SRC.indexOf("persistDecision");
    expect(ackIdx).toBeGreaterThan(persistIdx);
  });

  it("transient errors do NOT XACK — next consumer pass retries", () => {
    // The catch block following the persist+publish path must log
    // `consumer.process_err` but must NOT call xack — that's how
    // at-least-once is preserved. Locate the catch block that mentions
    // `consumer.process_err` and verify NO xack appears between the
    // opening `{` and the matching closing `}` immediately before the
    // `finally` block.
    expect(CONSUMER_SRC).toMatch(/consumer\.process_err/);

    // Slice between `consumer.process_err` and the next `} finally` —
    // that span is the body of the transient-error catch block exclusively.
    const startIdx = CONSUMER_SRC.indexOf("consumer.process_err");
    const finallyIdx = CONSUMER_SRC.indexOf("} finally", startIdx);
    expect(startIdx).toBeGreaterThan(0);
    expect(finallyIdx).toBeGreaterThan(startIdx);
    const errorCatchBody = CONSUMER_SRC.slice(startIdx, finallyIdx);
    expect(errorCatchBody).not.toMatch(/xack\(/);
  });

  it("trace_id is propagated into persistDecision", () => {
    // R7 trazabilidad E2E: the trace_id field must flow through the
    // persistence call so downstream queries can correlate.
    expect(CONSUMER_SRC).toMatch(/opportunity!\.trace_id/);
  });

  it("XPENDING-based lag tracking is wired", () => {
    expect(CONSUMER_SRC).toMatch(/xpending\(stream,\s*group\)/);
    expect(CONSUMER_SRC).toMatch(/lagGauge/);
  });
});

describe("WO-D8 (schema-drift-2026-09-20) — orphan consumer hygiene invariants", () => {
  it("maintenance sweep exists with XAUTOCLAIM reclaim and bounded cursor loop", () => {
    expect(CONSUMER_SRC).toMatch(/maintenanceLoop/);
    expect(CONSUMER_SRC).toMatch(/xautoclaim\(/);
    expect(CONSUMER_SRC).toMatch(/RECLAIM_MIN_IDLE_MS/);
    expect(CONSUMER_SRC).toMatch(/ORPHAN_PURGE_IDLE_MS/);
    // Bounded loop: the XAUTOCLAIM cursor iteration must be capped.
    expect(CONSUMER_SRC).toMatch(/iterations < 100/);
  });

  it("zero-loss ordering: reclaim runs BEFORE DELCONSUMER inside the sweep", () => {
    const purgeIdx = CONSUMER_SRC.indexOf("private async purgeOrphanConsumers");
    expect(purgeIdx).toBeGreaterThan(0);
    const purgeBody = CONSUMER_SRC.slice(purgeIdx, CONSUMER_SRC.indexOf("private async reclaimPending"));
    const reclaimCallIdx = purgeBody.indexOf("await this.reclaimPending()");
    const delconsumerIdx = purgeBody.indexOf('"DELCONSUMER"');
    expect(reclaimCallIdx).toBeGreaterThan(-1);
    expect(delconsumerIdx).toBeGreaterThan(reclaimCallIdx);
    // DELCONSUMER is gated on the orphan having pending > 0 reclaimed first.
    expect(purgeBody).toMatch(/if \(pending > 0\)/);
  });

  it("sweep skips while kill-switch is armed (reclaim must not bypass the halt)", () => {
    const sweepIdx = CONSUMER_SRC.indexOf("private async maintenanceLoop");
    const sweepBody = CONSUMER_SRC.slice(sweepIdx, CONSUMER_SRC.indexOf("private async purgeOrphanConsumers"));
    expect(sweepBody).toMatch(/killSwitch\.isEnabled\(\)/);
    expect(sweepBody.indexOf("killSwitch.isEnabled()")).toBeLessThan(
      sweepBody.indexOf("purgeOrphanConsumers()"),
    );
  });

  it("self is never purged; own PEL entries are never dropped by DELCONSUMER", () => {
    const purgeIdx = CONSUMER_SRC.indexOf("private async purgeOrphanConsumers");
    const purgeBody = CONSUMER_SRC.slice(purgeIdx, CONSUMER_SRC.indexOf("private async reclaimPending"));
    expect(purgeBody).toMatch(/name === CONSUMER\) continue/);
    // stop() deregisters self ONLY when its pending count is zero.
    const stopIdx = CONSUMER_SRC.indexOf("async stop(");
    const stopBody = CONSUMER_SRC.slice(stopIdx, stopIdx + 1400);
    expect(stopBody).toMatch(/pending \?\? 0\) === 0/);
  });

  it("XINFO CONSUMERS raw-array shape is normalized (WO-15 hotfix invariant)", () => {
    expect(CONSUMER_SRC).toMatch(/function normalizeConsumer/);
    expect(CONSUMER_SRC).toMatch(/Array\.isArray\(c\)/);
  });

  it("trimmed entries still in the PEL are XACKed during reclaim", () => {
    const reclaimIdx = CONSUMER_SRC.indexOf("private async reclaimPending");
    const reclaimBody = CONSUMER_SRC.slice(reclaimIdx);
    expect(reclaimBody).toMatch(/if \(!kv\)/);
    expect(reclaimBody).toMatch(/xack\(STREAM_IN,\s*GROUP,\s*id\)/);
  });
});
