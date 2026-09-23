// Audit SEC-01 fix (2026-09-24): this file previously committed a REAL
// postgres DSN with password (`postgres://arbx_rw:...`). That credential is
// compromised by git history and MUST be rotated out-of-band. The script now
// requires DATABASE_URL from the environment (secrets.policy.md §2: never
// commit credentials; placeholders live in .env.example only).
//
// Installs the WS broadcast trigger for the opportunities table.
// Run once per database:
//   DATABASE_URL=postgres://... node inject-trigger.mjs
//
// FRONT-01 reconciliation (t_26eaafc3, spec §3.2 option "single source"):
// the embedded SQL duplicate is REMOVED. This script now applies the CANONICAL
// migration file `database/migrations/124_opportunities_ws_trigger_amount_text.sql`
// verbatim at runtime — one source of truth for the NOTIFY payload shape
// (amount_in_wei as exact text). Never re-embed the SQL here; change the
// migration.
import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Client } from 'pg';

const url = process.env.DATABASE_URL;
if (!url || /^(postgres|postgresql):\/\/[^:]*:REPLACE_ME@/.test(url)) {
  console.error(
    'DATABASE_URL is required (see .env.example). Refusing to run with a missing/placeholder credential.'
  );
  process.exit(1);
}

// backend/api-server/inject-trigger.mjs -> <repo>/database/migrations/124_*.sql
const MIGRATION_NAME = '124_opportunities_ws_trigger_amount_text.sql';
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const candidates = [
  path.resolve(scriptDir, '..', '..', 'database', 'migrations', MIGRATION_NAME),
  path.resolve(process.cwd(), 'database', 'migrations', MIGRATION_NAME),
];
const migrationPath = candidates.find((p) => existsSync(p));
if (!migrationPath) {
  console.error(
    `Canonical migration not found (looked in: ${candidates.join(', ')}). ` +
      'The trigger payload shape must come from the migration — do not re-embed SQL here.'
  );
  process.exit(1);
}

const sql = await readFile(migrationPath, 'utf8');
if (
  !sql.includes('CREATE OR REPLACE FUNCTION notify_new_opportunity') ||
  !sql.includes('pg_trigger')
) {
  console.error(
    `${migrationPath} does not look like the WS trigger migration (missing function body or catalog guard).`
  );
  process.exit(1);
}

const client = new Client({ connectionString: url });
try {
  await client.connect();
  await client.query(sql);
  console.log(
    `Applied ${MIGRATION_NAME} (amount_in_wei crosses WS as exact text).`
  );
} catch (e) {
  console.error('Error applying migration:', e);
  process.exitCode = 1;
} finally {
  await client.end();
}
