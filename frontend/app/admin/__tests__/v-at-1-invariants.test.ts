/**
 * V-AT-1 invariants — no localStorage / sessionStorage in admin paths.
 *
 * The admin token MUST live only in the arbx_admin_session httpOnly cookie.
 * Any localStorage/sessionStorage write under frontend/app/admin/** or in
 * the AdminSessionBadge component is a V-AT-1 regression (XSS exposure).
 *
 * The companion verifier under backend/api-server/src/readiness/verifiers/
 * v-at-1.ts already scans frontend/lib/admin-token.ts; this test extends
 * coverage to the routes and the badge introduced in this PR.
 */
import { promises as fs } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const ROOT = path.resolve(__dirname, '..', '..', '..');

// Forbids any read/write of localStorage/sessionStorage keyed by a literal
// containing "token" or "admin" — i.e. the V-AT-1 anti-pattern. Mirrors the
// regex in backend/api-server/src/readiness/verifiers/v-at-1.ts so the
// frontend test enforces the same contract as the readiness gate.
const FORBIDDEN = /\b(?:localStorage|sessionStorage)\s*\.\s*(?:setItem|getItem|removeItem)\s*\(\s*['"][^'"]*(?:token|admin)[^'"]*['"]/i;

async function walk(dir: string): Promise<string[]> {
  const out: string[] = [];
  const entries = await fs.readdir(dir, { withFileTypes: true });
  for (const e of entries) {
    const full = path.join(dir, e.name);
    if (e.isDirectory()) {
      out.push(...(await walk(full)));
    } else if (/\.(ts|tsx)$/.test(e.name) && !/\.test\.(ts|tsx)$/.test(e.name)) {
      out.push(full);
    }
  }
  return out;
}

describe('V-AT-1 invariants — admin paths', () => {
  it('no localStorage/sessionStorage usage under frontend/app/admin/**', async () => {
    const adminDir = path.join(ROOT, 'app', 'admin');
    const files = await walk(adminDir);
    const offenders: string[] = [];
    for (const f of files) {
      const c = await fs.readFile(f, 'utf8');
      if (FORBIDDEN.test(c)) offenders.push(path.relative(ROOT, f));
    }
    expect(offenders).toEqual([]);
  });

  it('no localStorage/sessionStorage usage in AdminSessionBadge', async () => {
    const badge = path.join(ROOT, 'components', 'AdminSessionBadge.tsx');
    const c = await fs.readFile(badge, 'utf8');
    expect(FORBIDDEN.test(c)).toBe(false);
  });
});
