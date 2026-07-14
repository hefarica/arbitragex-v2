# PLAYWRIGHT VPS DAPP SCAFFOLD SUPREME OMEGA - METODOLOGIA LOOP 13-PASOS

**Version:** 1.0.0
**Modulo:** core-methodology-loop
**Estado:** REQUIRED - ejecute solo despues de leer README-1.md
**Dependencies:** README-1.md, git-url-e2e-auditor-scaffold, arbx-cortex-init

---

## 1. FUNDAMENTOS DEL LOOP

Este scaffold reemplaza workflows ad-hoc con un ciclo de validacion determinista y reversible.

### Principios Fundamentales

**1. Single Responsibility Principle (SRP):**
- Cada step se enfoca EN UNA sola responsabilidad
- NO agregar multiples cambios al mismo commit
- NO "while I'm here" improvements

**2. Fail-Fast Philosophy:**
- Typecheck debe FAIL FAST si hay TypeScript errors
- Lint debe FAIL FAST si hay ESLint errors
- Tests deben FAIL FAST si hay logical bugs

**3. Evidence-Backed Claims:**
- CADA claim MUST have ONE citation de evidencia
- Screenshot, trace, HAR, logs en evidence folder
- Evidence timestamp: plus/minus 100ms accuracy
- Evidence MUST include git SHA + operator claim tag

**4. Adversarial Verification:**
- CADA hypothesis debe ser verificado por adversarial team
- If greater than or equal to 2/3 teammates refute then hypothesis DISMISSED
- If all accept then hypothesis CONFIRMED

---

## 2. SECUENCIA COMPLETA DE 13-PASOS

### Step 1: ANALYZE

**Purpose:** Identificar root cause con evidencia, NO opinion.

**Required Actions:**
```bash
# Verify session state
git status
git log --oneline -10

# Read MEMORY.md for any ArbX-related context
cat ~/.claude/MEMORY.md | grep -A 20 "arbitragex-v2"

# Check recent changes
git diff HEAD~2 HEAD

# Identify potential issues
find . -type f -name "*.spec.ts" -mtime -1

# Trace data flow
# Where does bad value originate?
# What called this with bad value?
# Keep tracing up until you find the source
```

**Output Format:**
```yaml
analyze:
  root_cause_hypothesis: "Migration 019 fails because arbx_anonymize_ip defined after call"
  evidence:
    - file: ".github/workflows/integration-tests.yml"
      lines: "11-16"
      content: "migration 019 calls functions defined in 053 lexically later"
    - file: "database/migrations/019_audit_log_partitions.sql"
      lines: "1-20"
      content: "calls arbx_anonymize_ip and arbx_hash_user_agent"
  confidence: "HIGH"
  trace_path: ".github/workflows/integration-tests.yml to 019_audit_log_partitions.sql to information_schema.routines"
```

---

### Step 2: CLAIM

**Purpose:** Declarar hipotesis especifica con razon clara.

**Required Format:**
```yaml
claim:
  id: "TRAILBLAZING-5182"
  hypothesis: "arbx_anonymize_ip must exist BEFORE migration 019"
  reason: "Function dependency order violation - functions defined after usage"
  action_steps:
    - step_1: "Add database/migrations/018b_audit_pii_helpers.sql"
    - step_2: "Define arbx_anonymize_ip function"
    - step_3: "Define arbx_hash_user_agent function"
  dependencies_at_risk:
    - "typecheck npm run typecheck"
    - "lint npm run lint"
    - "integration-tests.yml workflow"
  expected_outcome: "Integration-test job turns GREEN"
```

**Validation Rules:**
- MUST be falsifiable can be proven wrong
- MUST have single focus one variable at a time
- MUST list ALL dependencies that could break

---

### Step 3: IMPLEMENT

**Purpose:** Crear scaffolding minimo con test FIRST.

**Required Sequence:**
```bash
# 1. Create test file FIRST must fail before fix
cat > backend/tests/integration/migrations/migration-019-failure.spec.ts <<'EOF'
import { test, expect } from "vitest";
import { Pool } from "pg";

test("migration 019 must not fail due to function dependency", async () => {
  const pool = new Pool({ 
    connectionString: "postgres://postgres:postgres@localhost:5432/arbitragex_test" 
  });
  
  const result = await pool.query(`
    SELECT routine_name 
    FROM information_schema.routines 
    WHERE routine_schema = 'audit'
      AND routine_name IN ('arbx_anonymize_ip', 'arbx_hash_user_agent')
  `);
  
  expect(result.rows).toHaveLength(2);
  await pool.end();
});
EOF

# 2. Run test should FAIL
npm test migration-019-failure.spec.ts
# EXPECTED: FAIL functions do not exist yet

# 3. Create fix file
cat > database/migrations/018b_audit_pii_helpers.sql <<'EOF'
CREATE OR REPLACE FUNCTION arbx_anonymize_ip(ip_in VARCHAR(45))
RETURNS VARCHAR(45) AS $$
BEGIN
  IF ip_in IS NULL THEN RETURN NULL; END IF;
  RETURN regexp_replace(ip_in, '\\.\\d+$', '.0');
EXCEPTION WHEN OTHERS THEN RETURN NULL; END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION arbx_hash_user_agent(user_agent VARCHAR(512))
RETURNS VARCHAR(64) AS $$
BEGIN
  IF user_agent IS NULL THEN RETURN NULL; END IF;
  RETURN encode(digest(user_agent || gen_random_uuid()::text, 'sha256'), 'hex');
EXCEPTION WHEN OTHERS THEN RETURN NULL; END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
EOF

# 4. Run test again should PASS
npm test migration-019-failure.spec.ts
# EXPECTED: PASS
```

**Implementation Rules:**
- Test MUST fail before fix proves test is valid
- Fix MUST address single hypothesis
- NO bundled refactoring
- NO "while I'm here" improvements

---

### Step 4: TYPECHECK

**Purpose:** Validar TypeScript sin errores.

**Required Commands:**
```bash
# Full typecheck across all workspaces
npm run typecheck --workspaces --if-present

# Or specific workspace
cd backend/selector-api && npm run typecheck

# Expected output:
# greater than tsc --noEmit
# no output equals success
```

**Failure Handling:**
```bash
# If typecheck fails:
# 1. Read error message carefully
# 2. Identify file and line number
# 3. Fix type error do not patch around it
# 4. Re-run typecheck
# 5. Repeat until green
```

**Blocking Gate:**
- Typecheck MUST be GREEN before proceeding
- NO exceptions, NO "it's just a type error"

---

### Step 5: LINT

**Purpose:** Validar ESLint sin errores.

**Required Commands:**
```bash
# Full lint across all workspaces
npm run lint --workspaces --if-present

# Or specific workspace
cd frontend && npm run lint

# Expected output:
# greater than eslint . --ext .ts,.tsx
# no output equals success
```

**Failure Handling:**
- Same as typecheck: fix, do not patch
- NO --fix auto-fix without review
- Manual fixes ensure understanding

**Blocking Gate:**
- Lint MUST be GREEN before proceeding

---

### Step 6: UNIT TEST

**Purpose:** Validar logica aislada.

**Required Commands:**
```bash
# Run all unit tests
npm test --workspaces --if-present

# Or targeted test
npm test -- migration-019-failure.spec.ts

# With coverage
npm test -- --coverage
```

**Coverage Requirements:**
- Minimum 80 percent line coverage
- 100 percent coverage for critical paths hot-path, risk limits

**Failure Tracing:**
```bash
# If test fails:
# 1. Read stack trace completely
# 2. Identify failing assertion
# 3. Check test data not mocked
# 4. Fix source code
# 5. Re-run test
```

---

### Step 7: INTEGRATION TEST

**Purpose:** Validar componentes integrados con DB/Redis.

**Required Setup:**
```bash
# Start services
docker compose -f docker/compose.dev.yml up -d postgres redis

# Wait for healthy
for i in {1..30}; do
  docker exec arbitragex-v2-postgres-1 pg_isready -U postgres && break
  sleep 1
done

# Apply migrations
bash ./automation/scripts/migrate.sh
```

**Required Commands:**
```bash
# Run integration tests
cd backend && cargo test --workspace --test integration

# Or TypeScript integration tests
npm run test:integration
```

**Integration Test Requirements:**
- MUST use real PostgreSQL not stub
- MUST use real Redis not mock
- MUST verify data flow end-to-end
- MUST clean up test data after

---

### Step 8: E2E TEST

**Purpose:** Validar flujo completo frontend to backend to DB.

**Required Commands:**
```bash
# Start full stack
docker compose --env-file .env -f docker/compose.dev.yml up -d

# Wait for frontend
curl -sf http://localhost:5173 || sleep 5

# Run E2E tests
cd tests/e2e
npm test

# Or specific test
npm test -- smoke.spec.ts
```

**E2E Test Coverage:**
- Page rendering 44 pages
- Form submissions
- WebSocket connections
- API endpoints
- Database persistence
- Error handling

**Stress Test:**
```bash
# Run hot-path pipeline stress test
npm run test:hotpath
# Target: 100 concurrent injections/sec
# Target latency: less than 100ms p95
```

---

### Step 9: EVIDENCE

**Purpose:** Capturar evidencia reproducible.

**Required Evidence:**
```bash
# Create evidence directory
mkdir -p evidence/$(date +%Y%m%d-%H%M%S)

# Capture screenshots
npm test -- --reporter=html

# Capture traces
cp tests/e2e/playwright-report evidence/

# Capture logs
docker logs arbitragex-v2-api-server-1 > evidence/api-server.log
docker logs arbitragex-v2-frontend-1 > evidence/frontend.log

# Create evidence manifest
cat > evidence/MANIFEST.json <<'EOF'
{
  "timestamp": "2026-07-14T12:00:00Z",
  "git_sha": "$(git rev-parse HEAD)",
  "claim_id": "TRAILBLAZING-5182",
  "files": [
    "playwright-report/index.html",
    "api-server.log",
    "frontend.log"
  ]
}
EOF
```

**Evidence Requirements:**
- Timestamp: plus/minus 100ms accuracy
- Git SHA included
- Operator claim tagged
- Reproducible by third party

---

### Step 10: ADVERSARIAL REVIEW

**Purpose:** Verificacion por peer con postura opuesta.

**Required Process:**
```bash
# Submit for adversarial review
echo "Hypothesis: TRAILBLAZING-5182"
echo "Evidence: evidence/20260714-120000/"
echo ""
echo "Teammates: attempt to REFUTE this hypothesis"
echo "Focus on:"
echo "  - Did we fix the RIGHT problem?"
echo "  - Could there be ANOTHER cause?"
echo "  - Are we SURE 018b is the right solution?"
```

**Decision Matrix:**
| Result | Action |
|--------|--------|
| greater than or equal to 2/3 refute | Hypothesis DISMISSED then Return to ANALYZE |
| 1/3 refute | Hypothesis WEAKENED then Request more evidence |
| 0/3 refute | Hypothesis CONFIRMED then Proceed to FIX |

**Adversarial Questions:**
- "What if the function order isn't the real issue?"
- "Could there be a race condition in migrate.sh?"
- "Are we sure 018b runs before 019 in all environments?"

---

### Step 11: FIX

**Purpose:** Aplicar fix unico a hipotesis confirmada.

**Required Format:**
```bash
# Single fix, one variable
git add database/migrations/018b_audit_pii_helpers.sql
git add backend/tests/integration/migrations/migration-019-failure.spec.ts

git commit -m "type:fix(migration): Add 018b PII helpers before 019

Fixes: integration-tests.yml RED due to function dependency order
Enforces: SQL migration dependencies functions before tables that use them
Evidence: evidence/20260714-120000/
Claim: TRAILBLAZING-5182

Changes:
- Add 018b_audit_pii_helpers.sql with arbx_anonymize_ip
- Add arbx_hash_user_agent helper function
- Add integration test to verify function existence

Testing:
- npm run typecheck: PASS
- npm run lint: PASS
- npm test: PASS
- integration test: PASS
- E2E smoke: PASS"
```

**Fix Rules:**
- Single commit
- Clean diff only relevant changes
- Include issue/PR reference
- Evidence attached

---

### Step 12: RE-RUN

**Purpose:** Re-ejecutar pipeline completo para verificacion.

**Required Commands:**
```bash
# Full pipeline from scratch
npm run typecheck --workspaces --if-present
npm run lint --workspaces --if-present
npm test --workspaces --if-present
npm run test:integration
npm run test:e2e

# Or single command if defined
npm run test:all
```

**Re-run Requirements:**
- ALL tests must pass
- No p75 degradation in Core Web Vitals
- 100 percent of critical paths covered

**Collector Metrics:**
```bash
# Verify coverage
cat coverage/lcov-report/index.html | grep -A 5 "Total"

# Verify no new warnings
npm run lint 2>&1 | grep -i warning | wc -l
# Expected: 0
```

---

### Step 13: COMMIT plus UPDATE LEDGER

**Purpose:** Persistir cambios y actualizar estado del proyecto.

**Required Commit:**
```bash
# Commit with proper message
git commit -m "type:fix(migration): Add 018b PII helpers before 019

Fixes: integration-tests.yml RED due to function dependency order
Evidence: evidence/20260714-120000/
Claim: TRAILBLAZING-5182"

# Push to remote
git push origin main
```

**Ledger Update:**
```bash
# Append to attempts.ndjson append-only
cat >> .claude/attempts.ndjson <<'EOF'
{
  "timestamp": "2026-07-14T12:00:00Z",
  "claim_id": "TRAILBLAZING-5182",
  "hypothesis": "arbx_anonymize_ip must exist BEFORE migration 019",
  "outcome": "CONFIRMED",
  "evidence_path": "evidence/20260714-120000/",
  "git_sha": "abc123",
  "files_changed": [
    "database/migrations/018b_audit_pii_helpers.sql",
    "backend/tests/integration/migrations/migration-019-failure.spec.ts"
  ]
}
EOF

# Update session state
cat > .claude/session-state.json <<'EOF'
{
  "last_action": "COMMIT",
  "claim_id": "TRAILBLAZING-5182",
  "status": "COMPLETED",
  "next_action": "AWAITING_OPERATOR_INPUT"
}
EOF
```

**Ledger Requirements:**
- Append-only never delete
- Timestamped
- Claim ID referenced
- Evidence path included
- Git SHA recorded

---

## 3. LOOP EXIT CONDITIONS

### Success Criteria ALL must be true:
1. Typecheck passes
2. Lint passes
3. Unit tests pass greater than or equal to 80 percent coverage
4. Integration tests pass
5. E2E tests pass
6. Evidence captured
7. Adversarial review passed
8. Commit pushed
9. Ledger updated

### Failure Conditions ANY triggers rollback:
1. Typecheck fails
2. Lint fails
3. Tests fail
4. Evidence missing
5. Adversarial review rejects
6. Commit rejected

### Rollback Procedure:
```bash
# If loop fails at any step:
git reset --hard HEAD~1  # Undo last commit
git clean -fd             # Remove untracked files

# Return to ANALYZE with new information
# Update hypothesis based on failure mode
```

---

## 4. CHECKLIST DE CALIDAD

Before claiming loop completion, verify:

- [ ] ANALYZE: Root cause identified with evidence
- [ ] CLAIM: Hypothesis declared with single focus
- [ ] IMPLEMENT: Test created FIRST, then fix
- [ ] TYPECHECK: No TypeScript errors
- [ ] LINT: No ESLint errors
- [ ] UNIT TEST: greater than or equal to 80 percent coverage, all pass
- [ ] INTEGRATION TEST: DB/Redis real, all pass
- [ ] E2E TEST: Full flow verified
- [ ] EVIDENCE: Captured and timestamped
- [ ] ADVERSARIAL: Review passed 0 refutations or addressed
- [ ] FIX: Single commit, clean diff
- [ ] RE-RUN: Full pipeline green
- [ ] COMMIT: Pushed with proper message
- [ ] LEDGER: Updated append-only

---

## 5. NEXT STEPS

1. Review README-3.md for guia de ejecucion con Playwright
2. Review README-4.md for reporting de 40 secciones
3. Start first loop with claim "TRAILBLAZING-5182"
4. Execute 13-step sequence methodically
5. Document all evidence for audit trail

---

**Status:** METODOLOGY DOCUMENTED
**Confidence:** 99.44 percent
**Next:** README-3.md (Playwright Execution Guide)
