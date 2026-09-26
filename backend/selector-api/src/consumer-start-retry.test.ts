/**
 * START-RETRY-01 (2026-09-26) — consumer boot resilience + liveness invariants.
 *
 * Anomaly: deploy recreate of 2026-09-25 22:43Z left selector-api "healthy"
 * while its Redis stream consumer was permanently dead — `consumer.start()`
 * was fired once, its rejection swallowed by `.catch(logger.error)`, and the
 * `running` latch had already closed BEFORE `ensureGroup()` could throw on
 * the EAI_AGAIN boot race, poisoning any later retry into a silent no-op.
 *
 * Because consumer.ts is private to the binary and tightly coupled to a real
 * ioredis instance, this regression test mirrors the source-code-invariant
 * harness of pipeline-invariants.test.ts: it reads known control-flow markers
 * from consumer.ts and index.ts and asserts the fix cannot silently regress.
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
const INDEX_SRC = readFileSync(
  resolve(__dirname_local, "./index.ts"),
  "utf8",
);

describe("START-RETRY-01 — consumer boot resilience invariants", () => {
  it("the running latch closes AFTER ensureGroup (a boot-race throw cannot poison retries)", () => {
    const startIdx = CONSUMER_SRC.indexOf("async start(): Promise<void>");
    expect(startIdx).toBeGreaterThan(-1);
    const groupIdx = CONSUMER_SRC.indexOf("await this.ensureGroup();", startIdx);
    const latchIdx = CONSUMER_SRC.indexOf("this.running = true;", startIdx);
    expect(groupIdx).toBeGreaterThan(startIdx);
    // Latch must come after the group call inside start() — if it comes
    // before, a rejected ensureGroup leaves running=true with no loops.
    expect(latchIdx).toBeGreaterThan(groupIdx);
  });

  it("startWithRetry retries with bounded attempts and fails LOUD on exhaustion", () => {
    expect(CONSUMER_SRC).toMatch(/async startWithRetry\(\): Promise<void>/);
    // One warn line per bounded attempt (R9: aggregated, boot-bounded).
    expect(CONSUMER_SRC).toMatch(/event: "consumer\.start_retry"/);
    // Exhaustion is an ERROR that rethrows — never a silent zombie.
    expect(CONSUMER_SRC).toMatch(/event: "consumer\.start_exhausted"/);
    // Knobs are env-only (arbx-no-hardcode-doctrine).
    expect(CONSUMER_SRC).toMatch(/ARBX_CONSUMER_START_RETRIES/);
    expect(CONSUMER_SRC).toMatch(/ARBX_CONSUMER_START_BASE_MS/);
  });

  it("index wires startWithRetry (never the bare fire-once start) and exits non-zero on permanent failure", () => {
    expect(INDEX_SRC).toMatch(/consumer\.startWithRetry\(\)\.catch/);
    // The old swallow-and-continue pattern is forbidden.
    expect(INDEX_SRC).not.toMatch(/consumer\.start\(\)\.catch/);
    expect(INDEX_SRC).toMatch(/consumer\.start_failed_permanent/);
    expect(INDEX_SRC).toMatch(/process\.exit\(1\)/);
  });

  it("/livez exposes consumer liveness honestly (200 alive / 503 dead)", () => {
    expect(INDEX_SRC).toMatch(/app\.get\("\/livez"/);
    expect(INDEX_SRC).toMatch(/consumer\.isAlive\(\)/);
    expect(INDEX_SRC).toMatch(/consumer_alive/);
    expect(INDEX_SRC).toMatch(/alive \? 200 : 503/);
  });

  it("isAlive() is exported on StreamConsumer", () => {
    expect(CONSUMER_SRC).toMatch(/isAlive\(\): boolean/);
    expect(CONSUMER_SRC).toMatch(/return this\.running;/);
  });
});
