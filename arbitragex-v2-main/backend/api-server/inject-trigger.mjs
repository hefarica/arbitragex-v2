import { Client } from 'pg';

const url = "postgres://arbx_rw:RRwDFjgqH61uDwSRYiNIkEFt@localhost:5432/arbitragex_v2";

async function main() {
  const client = new Client({ connectionString: url });
  await client.connect();

  const sql = `
  CREATE OR REPLACE FUNCTION notify_new_opportunity() RETURNS trigger AS $$
  BEGIN
    PERFORM pg_notify('opportunities_channel', row_to_json(NEW)::text);
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
    console.log("Trigger successfully created!");
  } catch (e) {
    console.error("Error creating trigger:", e);
  } finally {
    await client.end();
  }
}

main();
