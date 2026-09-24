# WO-7: Hot-Path Emitter Redis Pipeline (3 RTT → 1 RTT) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse the 3 sequential Redis round trips in `HotPathEmitter::emit_detected` (XADD + HSET + EXPIRE) into one non-atomic pipeline (3 commands, 1 RTT) with byte-identical wire semantics.

**Architecture:** Extract a pure function `detected_pipeline(opp, timestamp_ms) -> redis::Pipeline` that builds the exact same three commands with `.ignore()` each, and have `emit_detected` execute it with a single `query_async`. Non-atomic by design (`Pipeline::new()` default, NOT `.atomic()`): no MULTI/EXEC means server-side execution semantics are identical to the previous sequential form — any prefix may execute if the connection drops. Unit tests assert on the packed RESP bytes and command count; no live Redis needed.

**Tech Stack:** Rust (searcher-rs crate), redis 0.25.5 (workspace dep; `Pipeline` API verified in vendored source: `cmd_iter()`, `get_packed_pipeline()`, `ignore()`; `redis::pipe()` already used at `candidate_simulation.rs:579`, `price_worker.rs:1196`).

## Global Constraints

- Mode-invariant (CLAUDE.md §34.1): no change to emitted data, keys, TTLs, or stream fields — transport-only optimization. All 4 wire values (`arbx:hot:detected` XADD fields, `arbx:hot:opp:{id}` HSET, 300s EXPIRE, MAXLEN ~10000) stay verbatim.
- NON-atomic pipeline (no `.atomic()` / no MULTI/EXEC) — agreed design with peer -61; preserves mid-connection-drop semantics of the sequential form.
- `emit_simulated` has the same 3-RTT pattern but is OUT OF SCOPE (WO-7 = lines 71-115 only). Log it as follow-up on the BOARD; do not touch it here.
- CI gate: `cargo clippy -p searcher-rs --locked --all-targets -- -D warnings` must pass with 0 warnings; full suite must show 0 failed (currently 1315 passed / 5 ignored — count may drift with main, only 0-failed is the gate).
- Branch: `perf/redis-pipeline` created from `origin/main` (NOT from the peer's `perf/dataplane-tuning`). §36: never commit on the peer's branch. Do NOT touch `backend/Cargo.toml` or any `[profile.release]`.
- Windows AppControl gotcha: `cargo test` may fail once with "os error 4551" on a fresh binary — retry the same command before diagnosing.
- Commits end with `Co-Authored-By: Claude Code <noreply@anthropic.com>`; PR description ends with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- 34 required CI checks + branch protection: after any merge to main during the PR's life, run `gh api -X PUT repos/hefarica/arbitragex-v2/pulls/<N>/update-branch`.

---

### Task 0: Coordination window + clean branch

**Files:** none (git operations only).

**Interfaces:**
- Consumes: BOARD at `audits/perf-stack-2026-09-20/GOAL-WORKORDERS.md` (WO-7 row).
- Produces: branch `perf/redis-pipeline` at `origin/main`; peer -61 informed.

- [ ] **Step 1: Confirm the shared tree is free**

The working tree is shared with peer -61 (their branch `perf/dataplane-tuning`, uncommitted audit files). Send a message to -61 announcing WO-7 execution start (file: `backend/searcher-rs/src/hot_path_emitter.rs`) and asking for a window. Their untracked audit files are safe across branch switches (untracked files survive `git checkout`), but their branch must not have uncommitted *tracked* edits when switching. Verify:

Run: `git -C "c:/Users/HFRC/Desktop/arbitragex-v2-main (17)" status --porcelain`
Expected: only `??` (untracked) lines plus known shared-modified files (`.claude/settings.json`, audits). If `-61` has in-flight tracked edits, WAIT for their ACK before switching.

- [ ] **Step 2: Create the branch from origin/main**

```bash
git fetch origin main
git checkout -b perf/redis-pipeline origin/main
git branch --show-current   # must print: perf/redis-pipeline
```

- [ ] **Step 3: Sanity — the target file matches this plan's line numbers**

Run: `grep -n "query_async" backend/searcher-rs/src/hot_path_emitter.rs | head`
Expected: 6 hits (3 in `emit_detected`, 2 in `emit_simulated`'s passed branch, 1 in `emit_gate_commit_from_state`). If the file changed on main, re-read it before editing.

---

### Task 1: Pure pipeline builder (TDD)

**Files:**
- Modify: `backend/searcher-rs/src/hot_path_emitter.rs` (add `detected_pipeline` fn + `#[cfg(test)] mod tests` at end of file)

**Interfaces:**
- Consumes: `shared_rs::contracts::Opportunity` (already imported), `redis::pipe()`.
- Produces: `fn detected_pipeline(opp: &Opportunity, timestamp_ms: u64) -> redis::Pipeline` (private, same module) — 3 commands, each `.ignore()`, non-atomic.

- [ ] **Step 1: Write the failing tests**

Append at the end of `backend/searcher-rs/src/hot_path_emitter.rs` (after the closing `}` of `impl HotPathEmitter` and the trailing comment block):

```rust
#[cfg(test)]
mod wo7_tests {
    use super::*;
    use shared_rs::contracts::StrategyKind;
    use uuid::Uuid;

    fn fixture_opp() -> Opportunity {
        Opportunity {
            id: Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: StrategyKind::triangular(),
            dex_a: "fixture_dex".into(),
            dex_b: None,
            pair_symbol: "A/B".into(),
            token_in: "0x1".into(),
            token_out: "0x2".into(),
            amount_in_wei: "1000".into(),
            expected_profit_usd: Some(10.0),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detector_id: None,
            pipeline_latency_ms: None,
            detected_at: chrono::Utc::now(),
            trace_id: Uuid::new_v4(),
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    /// WO-7: exactly 3 commands, executed as ONE round trip.
    #[test]
    fn detected_pipeline_is_three_commands() {
        let opp = fixture_opp();
        let pipe = detected_pipeline(&opp, 1_700_000_000_000u64);
        assert_eq!(pipe.cmd_iter().count(), 3, "3 commands, 1 RTT");
    }

    /// WO-7: NOT atomic — an atomic pipeline would prepend MULTI as the
    /// first RESP command. Its absence proves no MULTI/EXEC wrapping, so
    /// mid-connection-drop semantics match the old sequential form.
    #[test]
    fn detected_pipeline_is_not_atomic() {
        let opp = fixture_opp();
        let packed = detected_pipeline(&opp, 0).get_packed_pipeline();
        assert!(
            !packed.starts_with(b"*1\r\n$5\r\nMULTI\r\n"),
            "pipeline must not open with MULTI"
        );
    }

    /// WO-7: wire contract byte-identical to the sequential form (stream
    /// key, MAXLEN ~10000, opp hash key, HSET data, EXPIRE 300, strategy
    /// kind, timestamp).
    #[test]
    fn detected_pipeline_carries_exact_wire_fields() {
        let opp = fixture_opp();
        let ts = 1_700_000_000_123u64;
        let packed = detected_pipeline(&opp, ts).get_packed_pipeline();
        assert!(contains(&packed, b"XADD"));
        assert!(contains(&packed, b"arbx:hot:detected"));
        assert!(contains(&packed, b"MAXLEN"));
        assert!(contains(&packed, b"10000"));
        let key = format!("arbx:hot:opp:{}", opp.id).into_bytes();
        assert!(contains(&packed, &key));
        assert!(contains(&packed, b"HSET"));
        assert!(contains(&packed, b"EXPIRE"));
        assert!(contains(&packed, b"300"));
        let sk = opp.strategy_kind.as_str().as_bytes().to_vec();
        assert!(contains(&packed, &sk));
        assert!(contains(&packed, ts.to_string().as_bytes()));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p searcher-rs --locked wo7_tests 2>&1 | tail -15`
Expected: COMPILE ERROR — `cannot find function \`detected_pipeline\``. (A compile error is the failing state for a not-yet-existing function.)

- [ ] **Step 3: Implement the builder**

Inside `impl HotPathEmitter` in `backend/searcher-rs/src/hot_path_emitter.rs`, add this method right BEFORE `emit_detected` (i.e. after the `new` constructor):

```rust
    /// Builds the detected-emit command set: XADD stream entry + HSET opp
    /// hash + EXPIRE 300s.
    ///
    /// WO-7 (PERF-STACK-2026-09-20): the three sequential `query_async`
    /// round trips collapse into ONE non-atomic pipeline (3 commands,
    /// 1 RTT). `.atomic()` is deliberately NOT used — without MULTI/EXEC
    /// the server executes commands as they arrive, so a mid-connection
    /// drop may leave any prefix applied, exactly as the sequential form
    /// did. Wire values are byte-identical (see `wo7_tests`).
    fn detected_pipeline(opp: &Opportunity, timestamp_ms: u64) -> redis::Pipeline {
        let id = opp.id.to_string();
        let opp_key = format!("arbx:hot:opp:{}", id);
        let opp_json = serde_json::to_string(opp).unwrap_or_default();

        let mut pipe = redis::pipe();
        pipe.cmd("XADD")
            .arg("arbx:hot:detected")
            .arg("MAXLEN")
            .arg("~")
            .arg(10000)
            .arg("*")
            .arg("id")
            .arg(&id)
            .arg("chain_id")
            .arg(opp.chain_id)
            .arg("strategy_kind")
            .arg(opp.strategy_kind.as_str())
            .arg("detected_at_ms")
            .arg(timestamp_ms)
            .ignore();
        pipe.cmd("HSET")
            .arg(&opp_key)
            .arg("data")
            .arg(opp_json)
            .ignore();
        pipe.cmd("EXPIRE").arg(&opp_key).arg(300).ignore();
        pipe
    }
```

Note: `detected_pipeline` does NOT take `&self` — a pure associated function is testable without a Redis connection and makes the no-I/O property visible in the signature.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p searcher-rs --locked wo7_tests 2>&1 | tail -10`
Expected: `test result: ok. 3 passed; 0 failed`. (If "os error 4551" appears, retry the same command once — AppControl gotcha.)

---

### Task 2: Rewire `emit_detected` to one RTT

**Files:**
- Modify: `backend/searcher-rs/src/hot_path_emitter.rs:71-115` (body of `emit_detected`)

**Interfaces:**
- Consumes: `HotPathEmitter::detected_pipeline` from Task 1 (exact signature above).
- Produces: `emit_detected` with identical signature `pub async fn emit_detected(&self, opp: &Opportunity) -> Result<(), redis::RedisError>` and identical external behavior.

- [ ] **Step 1: Replace the three sequential commands**

Replace the ENTIRE body of `emit_detected` (lines 71-115: from `let id = opp.id.to_string();` through the final EXPIRE block, keeping the doc-comment and signature) with:

```rust
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // WO-7: XADD + HSET + EXPIRE in ONE pipeline round trip.
        let _: () = detected_pipeline(opp, timestamp_ms)
            .query_async(&mut self.redis.clone())
            .await?;
        Ok(())
```

Also update the doc-comment line `/// Latency budget: <5ms (measured at 1-2ms in local benchmarks).` to:

```rust
    /// Latency budget: <5ms. WO-7: one pipeline RTT (was three sequential).
```

Do NOT touch `emit_simulated` or `emit_gate_commit_from_state` (out of scope; follow-up logged on BOARD).

- [ ] **Step 2: Verify the full gate chain**

Run each in order (all from repo root):

```bash
cargo fmt -p searcher-rs
cargo clippy -p searcher-rs --locked --all-targets -- -D warnings 2>&1 | tail -3
cargo test -p searcher-rs --locked 2>&1 | tail -5
```

Expected:
- fmt: silent success.
- clippy: `finished` with no error lines (0 warnings, `-D warnings` makes any warning a failure).
- tests: `test result: ok` with **0 failed** (passed count ≈1318 now — 1315 + 3 new; drift from main merges is fine, 0 failed is the gate).

- [ ] **Step 3: Commit**

```bash
git add backend/searcher-rs/src/hot_path_emitter.rs
git commit -m "$(cat <<'EOF'
perf: collapse emit_detected 3 Redis RTTs into 1 non-atomic pipeline (WO-7)

XADD + HSET + EXPIRE now travel as one pipeline round trip; wire values
byte-identical, no MULTI/EXEC so drop-mid-flight semantics match the
sequential form. emit_simulated follow-up logged on the BOARD.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: PR + BOARD update

**Files:**
- Create: PR #N (branch `perf/redis-pipeline` → `main`).
- Modify: `audits/perf-stack-2026-09-20/GOAL-WORKORDERS.md` (WO-7 estado).

**Interfaces:**
- Consumes: GitHub CLI (`gh`), BOARD file.
- Produces: merged PR (PR-4 per BOARD agrupación), WO-7 estado ✅.

- [ ] **Step 1: Push and open the PR**

```bash
git push -u origin perf/redis-pipeline
gh pr create --base main --head perf/redis-pipeline \
  --title "perf: hot-path emitter Redis pipeline (WO-7, 3 RTT -> 1 RTT)" \
  --body "$(cat <<'EOF'
## Summary
- `HotPathEmitter::emit_detected` sent XADD, HSET and EXPIRE as three
  sequential `query_async` round trips; now one non-atomic `redis::pipe()`
  (3 commands, 1 RTT). Transport-only, mode-invariant (§34.1).
- New pure builder `detected_pipeline(opp, ts)` + 3 unit tests asserting
  command count, absence of MULTI, and byte-exact wire fields (stream key,
  MAXLEN ~10000, opp hash key, EXPIRE 300).
- `emit_simulated` has the same pattern; logged as BOARD follow-up, not
  touched here.

## Test plan
- [x] `cargo test -p searcher-rs --locked wo7_tests` — 3 passed
- [x] `cargo clippy -p searcher-rs --locked --all-targets -- -D warnings` — 0 warnings
- [x] Full suite — 0 failed (no regression)
- [ ] CI 34/34 green
- [ ] Post-merge deploy + Prometheus hot-path emit latency check (BOARD gate)

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 2: Wait for CI (34 checks)**

Run in background; on any merge to main during the wait, apply
`gh api -X PUT repos/hefarica/arbitragex-v2/pulls/<N>/update-branch` and re-wait.
Expected: 34 pass / 0 fail.

- [ ] **Step 3: Merge + update BOARD**

```bash
gh pr merge <N> --merge
```

Then edit `audits/perf-stack-2026-09-20/GOAL-WORKORDERS.md`: WO-7 estado
`🕓 tras #600 (a8)` → `✅ PR #<N> merged`, and append to the Log de eventos:
follow-up note that `emit_simulated` (lines ~139-202) still emits 3 sequential
RTTs on the passed branch (2 RTTs + 1 XADD) — candidate for a WO-7b.

- [ ] **Step 4: Deploy + verify (per-service R3, per BOARD gate)**

Only after coordinating with -61 (searcher-rs deploy also ships #600):

```bash
ssh arbx 'cd /opt/arbitragex-v2 && git pull && docker builder prune -f'
ssh arbx 'cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.prod.yml build --no-cache searcher-rs && docker compose --env-file .env -f docker/compose.prod.yml up -d searcher-rs'
ssh arbx 'cd /opt/arbitragex-v2 && git rev-parse HEAD'   # L4: == merge SHA
```

Expected: deploy SUCCESS, L4 SHA match, `docker logs searcher-rs --tail 50` shows normal emit activity (no Redis errors), Prometheus `arbx_hot_*` emit counters still increasing.

---

## Self-Review

1. **Spec coverage:** WO-7 = "XADD+HSET+EXPIRE → 1 pipeline RTT" on
   `hot_path_emitter.rs:79-112` → Tasks 1-2 implement it, Task 0 handles the
   §36/BOARD coordination constraint, Task 3 handles PR-4 + BOARD + deploy
   gate. Out-of-scope `emit_simulated` explicitly parked as follow-up. ✓
2. **Placeholder scan:** every step carries complete code or an exact
   command with expected output; no TBD/TODO. ✓
3. **Type consistency:** `detected_pipeline(&Opportunity, u64) -> redis::Pipeline`
   identical in Task 1 (definition) and Task 2 (call site);
   `query_async::<_, ()>` matches the all-ignored pattern;
   fixture field list matches `shared_rs::contracts::Opportunity`
   (verified against `candidate_simulation.rs:614` fixture). ✓
