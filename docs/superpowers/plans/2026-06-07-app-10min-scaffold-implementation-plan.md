# APP-10MIN Scaffold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a template-based scaffold system that generates enterprise-grade arbitrage applications in <10 minutes.

**Architecture:** Template-based generator (load → substitute variables → inject gates → validate via 3 phases).

**Tech Stack:** Bash orchestrator, Rust/TS/Solidity templating, existing test suites.

---

## IMPLEMENTATION TASKS (8 Tasks Total)

### Task 1: Orchestrator Script Skeleton
Create `automation/scripts/app-scaffold.sh` with argument parsing, phase orchestration, and error handling.

**Key Functions:**
- `validate_spec_json()` — check required fields
- `select_templates_by_type()` — map strategy_type to template files
- `substitute_variables()` — sed/jq to replace {{PLACEHOLDERS}}
- `inject_gates()` — append gate snippets to generated code
- `run_validation_phases()` — execute compile, unit, E2E phases

**Deliverable:** Executable script with dry-run test passing.

---

### Task 2: Template Files (Engine, Worker, Frontend)
Create 4 base template files by copying & annotating existing code with {{PLACEHOLDERS}}.

**Files to Create:**
- `automation/templates/engines/triangular_engine.rs.template`
- `automation/templates/workers/triangular_worker.rs.template`
- `automation/templates/frontend/strategy_page.tsx.template`
- `automation/templates/frontend/strategy_client.tsx.template`

**Verification:** Grep each template for ≥2 `{{STRATEGY_NAME}}` placeholders.

---

### Task 3: Gate Injection Snippets
Create 4 snippet files for mandatory gates.

**Files to Create:**
- `automation/templates/gates/net_profit_gate_snippet.rs`
- `automation/templates/gates/mev_ethics_gate_snippet.rs`
- `automation/templates/gates/risk_limits_gate_snippet.rs`
- `automation/templates/gates/pre_execute_checklist_snippet.rs`

**Verification:** All snippets are syntactically valid Rust (cargo check).

---

### Task 4: Test Harness
Create E2E fork-test runner.

**Files to Create:**
- `automation/templates/tests/fork-test.sh.template`
- `automation/templates/tests/fork-test.spec.ts.template`

**Verification:** Scripts are executable and parse without errors.

---

### Task 5: Variable Substitution
Implement full `substitute_variables()` in orchestrator.

**Logic:** Extract all variables from spec.json, sed-replace in all templates, output to $OUTPUT_DIR.

**Verification:** Test with `triangle-test.json` spec; verify output files contain substituted values (not placeholders).

---

### Task 6: Gate Injection
Implement full `inject_gates()` in orchestrator.

**Logic:** Append all 4 gate snippets to generated worker.rs and engine.rs.

**Verification:** Output files contain `arbx-net-profit-gate`, `arbx-mev-ethics-gate`, etc. comments.

---

### Task 7: Validation Phases
Implement all 3 phases (compile, unit tests, E2E fork-test).

**PHASE A (2 min):** tsc + cargo check + solc  
**PHASE B (3 min):** vitest + cargo test + forge test  
**PHASE C (4 min):** anvil fork + deploy + 5 synthetic routes + gate verification

**Verification:** All phases pass with zero errors on test spec.

---

### Task 8: Deliverables Generator
Implement `generate_deliverables()` to output docker-compose, .env, workflow, README.

**Files to Generate:**
- docker-compose.yml
- .env.example (all placeholders)
- .github/workflows/deploy-<strategy>.yml
- README.md (how to run, deploy, gates)

**Verification:** All deliverables are present and contain correct substitutions.

---

## Execution Roadmap

1. **Tasks 1-4:** Foundation (scripts, templates, gates, tests) — 1 hour
2. **Tasks 5-6:** Core logic (substitution, injection) — 30 min
3. **Task 7:** Validation (compile, unit, E2E) — 1 hour
4. **Task 8:** Deliverables (docker, env, workflow) — 30 min
5. **Integration Test:** Full scaffold run end-to-end — 30 min

**Total Estimate:** 3.5 hours implementation.

---

## Success Criteria

- ✅ `app-scaffold.sh` generates code from spec.json in <10 min
- ✅ All phases pass (compile, unit tests, E2E)
- ✅ Generated code includes all 4 mandatory gates
- ✅ Deliverables ready for git commit (no manual edits needed)
- ✅ Output is deterministic (same spec → same code)
- ✅ Zero hardcoded values (all from spec or env)

---

**PLAN READY FOR EXECUTION**

Use superpowers:subagent-driven-development to dispatch 8 parallel task agents, or superpowers:executing-plans to execute sequentially.
