// Audit SEC-01 fix (2026-09-24): this file previously committed a REAL
// postgres DSN with password (`postgres://arbx_rw:...`). That credential is
// compromised by git history and MUST be rotated out-of-band. The script now
// requires DATABASE_URL from the environment (secrets.policy.md §2: never
// commit credentials; placeholders live in .env.example only).
//
// Installs the WS broadcast trigger for the opportunities table (mig 025).
// Run once per database:
//   DATABASE_URL=postgres://... node inject-trigger.mjs
import { Client } from 'pg';

const url = process.env.DATABASE_URL;
if (!url || /^(postgres|postgresql):\/\/[^:]*:REPLACE_ME@/.test(url)) {
  console.error(
    'DATABASE_URL is required (see .env.example). Refusing to run with a missing/placeholder credential.'
  );
  process.exit(1);
}

async function main() {
  const client = new Client({ connectionString: url });
  await client.connect();

  // FRONT-01 fix: amount_in_wei (NUMERIC(78,0)) must cross the WS boundary as
  // an exact STRING. row_to_json serializes numerics unquoted, and the client
  // JSON.parse turns them into float64 (silent ±loss above 2^53). jsonb_set
  // re-wraps the two int8/numeric-sensitive fields as text before pg_notify.
  // Supersedes the shape installed by migration 025 (same channel/trigger name).
  const sql = `
  CREATE OR REPLACE FUNCTION notify_new_opportunity() RETURNS trigger AS $$
  BEGIN
    PERFORM pg_notify(
      'opportunities_channel',
      jsonb_set(
        jsonb_set(
          to_jsonb(NEW),
          '{amount_in_wei}',
          to_jsonb(NEW.amount_in_wei::text)
        ),
        '{amount_in_wei,text}',
        to_jsonb(NEW.amount_in_wei::text),
        true
      )::text
    );
    RETURN NEW;
  END;
  $$ LANGUAGE plpgsql;

  DROP TRIGGER IF EXISTS trg_notify_opportunity ON opportunities;

  CREATE TRIGGER trg_notify_opportunity
  AFTER INSERT ON opportunities
  FOR EACH ROW
  EXECUTE FUNCTION notify_new_opportunity();
  `;

  try {
    await client.query(sql);
    console.log('Trigger successfully created (amount_in_wei as exact text).');
  } catch (e) {
    console.error('Error creating trigger:', e);
    process.exitCode = 1;
  } finally {
    await client.end();
  }
}

main();
