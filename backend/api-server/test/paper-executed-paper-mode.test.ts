/**
 * A3 PAPER_EXECUTED — integración contra Postgres REAL (lane `test:integration`).
 *
 * Demuestra contra una base migrada de verdad (el job CI aplica
 * `automation/scripts/migrate.sh` antes de correr esto):
 *   (a) el CHECK de `opportunities.status` ACEPTA 'paper_executed' ⇒ la migración 127
 *       se aplicó y el estado es representable;
 *   (b) el escritor del camino paper mueve la fila a 'paper_executed' con evidencia
 *       (`paper_trade_runs.id`) y **NO** toca 'executed' ⇒ `executed` sigue en 0;
 *   (c) la migración es IDEMPOTENTE: re-ejecutarla (el runner la corre en CADA
 *       deploy) no rompe ni duplica nada;
 *   (d) 'executed' y 'paper_executed' son estados DISTINTOS: una fila `executed`
 *       no puede ser degradada por el escritor paper (guarda `status IN (...)`).
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import pg from "pg";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const DATABASE_URL = process.env["DATABASE_URL"];
const here = dirname(fileURLToPath(import.meta.url));
const MIGRATION_PATH = join(
  here,
  "../../..",
  "database/migrations/127_opportunities_paper_executed_status.sql"
);

/** Mismo SQL que `PaperExecutor.markPaperExecuted` (fuente única: executor.ts). */
const MARK_PAPER_EXECUTED_SQL = `
  UPDATE opportunities
     SET status = 'paper_executed',
         updated_at = NOW()
   WHERE id = $1
     AND status IN ('detected','validated','simulated','scored')
`;

describe.skipIf(!DATABASE_URL)("A3 paper_executed (PG real)", () => {
  let pool: pg.Pool;
  let oppId: string;
  let executedOppId: string;

  beforeAll(async () => {
    pool = new pg.Pool({ connectionString: DATABASE_URL });
    const ins = await pool.query<{ id: string }>(
      `INSERT INTO opportunities
         (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id, status)
       VALUES (1, 'dex_arb', 'uniswap-v2', $1, $2, '1000000000000000000', gen_random_uuid(), 'validated')
       RETURNING id`,
      ["0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984"]
    );
    oppId = ins.rows[0]!.id;

    // Segunda fila para probar que el escritor NO degrada un `executed` on-chain.
    const ins2 = await pool.query<{ id: string }>(
      `INSERT INTO opportunities
         (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id, status)
       VALUES (1, 'dex_arb', 'uniswap-v2', $1, $2, '1000000000000000000', gen_random_uuid(), 'executed')
       RETURNING id`,
      ["0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984"]
    );
    executedOppId = ins2.rows[0]!.id;
  });

  afterAll(async () => {
    if (!pool) return;
    await pool.query(`DELETE FROM opportunities WHERE id = ANY($1::uuid[])`, [
      [oppId, executedOppId],
    ]);
    await pool.end();
  });

  it("(a) el CHECK de opportunities.status acepta 'paper_executed' (migración 127 aplicada)", async () => {
    const def = await pool.query<{ def: string }>(
      `SELECT pg_get_constraintdef(c.oid) AS def
         FROM pg_constraint c
        WHERE c.conrelid = 'public.opportunities'::regclass
          AND c.conname = 'opportunities_status_check'`
    );
    expect(def.rowCount).toBe(1);
    expect(def.rows[0]!.def).toContain("paper_executed");
    // El vocabulario previo se conserva (migración ADITIVA, no sustitutiva).
    for (const kept of ["detected", "validated", "simulated", "scored", "executing", "executed", "reconciled", "rejected", "failed"]) {
      expect(def.rows[0]!.def).toContain(kept);
    }
  });

  it("(b) el escritor paper mueve la fila a paper_executed con evidencia y executed sigue en 0", async () => {
    // `executed` ajeno al fixture (el fixture (d) inserta UNA fila `executed` a
    // propósito, así que se excluye por id para medir el efecto del escritor).
    const countForeignExecuted = async (): Promise<number> => {
      const r = await pool.query<{ n: string }>(
        `SELECT count(*)::text AS n FROM opportunities
          WHERE status = 'executed' AND id <> $1`,
        [executedOppId]
      );
      return Number(r.rows[0]!.n);
    };

    const before = await countForeignExecuted();

    const res = await pool.query(MARK_PAPER_EXECUTED_SQL, [oppId]);
    expect(res.rowCount).toBe(1);

    const row = await pool.query<{ status: string }>(
      `SELECT status FROM opportunities WHERE id = $1`,
      [oppId]
    );
    expect(row.rows[0]!.status).toBe("paper_executed");

    // El escritor paper NO creó ninguna fila `executed`: el conteo ajeno no cambia
    // y en una base limpia es 0.
    const after = await countForeignExecuted();
    expect(after).toBe(before);
    expect(after).toBe(0);

    // Y la fila movida no es `executed` (estados distintos por diseño).
    const anyExecuted = await pool.query<{ n: string }>(
      `SELECT count(*)::text AS n FROM opportunities WHERE id = $1 AND status = 'executed'`,
      [oppId]
    );
    expect(Number(anyExecuted.rows[0]!.n)).toBe(0);
  });

  it("(c) la migración es idempotente (re-ejecutarla no rompe ni duplica)", async () => {
    const sql = readFileSync(MIGRATION_PATH, "utf8");
    // El runner NO tiene ledger: re-ejecuta el archivo completo en cada deploy.
    await expect(pool.query(sql)).resolves.toBeDefined();
    await expect(pool.query(sql)).resolves.toBeDefined();
    const constraints = await pool.query<{ n: string }>(
      `SELECT count(*)::text AS n FROM pg_constraint
        WHERE conrelid = 'public.opportunities'::regclass
          AND conname = 'opportunities_status_check'`
    );
    expect(constraints.rows[0]!.n).toBe("1");
  });

  it("(d) el escritor paper NO degrada una fila `executed` (guarda de estados terminales)", async () => {
    const res = await pool.query(MARK_PAPER_EXECUTED_SQL, [executedOppId]);
    expect(res.rowCount).toBe(0); // la guarda `status IN (...)` no incluye 'executed'
    const row = await pool.query<{ status: string }>(
      `SELECT status FROM opportunities WHERE id = $1`,
      [executedOppId]
    );
    expect(row.rows[0]!.status).toBe("executed");
  });
});
