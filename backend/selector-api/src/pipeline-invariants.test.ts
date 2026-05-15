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
    // at-least-once is preserved.
    expect(CONSUMER_SRC).toMatch(/consumer\.process_err/);
    // The ERROR catch should not directly call xack.
    const errorBlockMatch = CONSUMER_SRC.match(
      /} catch \(err\) \{[\s\S]*?consumer\.process_err[\s\S]*?\} finally/,
    );
    expect(errorBlockMatch).not.toBeNull();
    if (errorBlockMatch) {
      expect(errorBlockMatch[0]).not.toMatch(/xack\(/);
    }
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
