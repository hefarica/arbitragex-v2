-- ArbitrageX v2 — Migration 127: A3 PAPER_EXECUTED
--
-- Añade el estado `paper_executed` a `opportunities.status`, SEPARADO de
-- `executed`, que queda reservado a inclusión on-chain real.
--
-- AUTORIZACIÓN DEL OPERADOR (literal):
--   "CONFIRMED: A3 status paper_executed separado de executed (paper, sin live)"
--
-- POR QUÉ (medido en Fase 1):
--   El único escritor de `status='executed'` es el relayer, y sólo tras
--   inclusión on-chain (`backend/relays-client/src/persistence.rs:47-59` +
--   `backend/relays-client/src/submit_engine.rs:854-883`). Con producción en
--   paper no hay difusión ⇒ `executed` es inalcanzable POR DISEÑO (medido:
--   `executed=0` en 515,989 filas de `opportunities` y `executions` vacía).
--   El camino paper necesita un estado honesto que NO se confunda con
--   ejecución on-chain: ése es `paper_executed`.
--
-- CRITERIO (definido, no inventado — es el que el pipeline paper ya computa):
--   una fila pasa a `paper_executed` cuando, en `backend/api-server/src/paper/executor.ts`:
--     (i)   el mensaje de simulación trae `status === "passed"` (lo decide
--           `backend/sim-ctl/src/sim_engine.rs` y lo persiste
--           `backend/sim-ctl/src/persistence.rs`),
--     (ii)  `calculateNetTopologicalYield` devuelve `net_yield_wei > 0`
--           (neto tras gas + decoherencia), y
--     (iii) `persistRun` acaba de crear la fila de evidencia en
--           `paper_trade_runs` (`paper_run_id`).
--   La evidencia asociada es `paper_trade_runs.id` + `opportunities.id`, y el
--   evento `paper_executor.paper_executed` la publica en el log.
--
-- ALCANCE (lo que esta migración NO hace):
--   · NO toca `executions.status` (su CHECK no admite 'executed' ni debe admitir
--     'paper_executed': esa tabla es de ejecución real).
--   · NO reescribe filas existentes: es ADITIVA (sólo amplía el vocabulario).
--   · NO habilita ningún camino live.
--
-- FORMA (obligatoria en este repo): ADITIVA + IDEMPOTENTE + LOCK-SAFE.
--   El runner (`database/run_migrations.sh`) NO tiene ledger de aplicadas: re-ejecuta
--   TODOS los archivos en CADA deploy. `opportunities` es HOT (searcher INSERTs +
--   purge DELETEs) y el FREEZE-01 lockguard corta a los 10 s (`lock_timeout`), así que
--   un `ALTER TABLE ... DROP/ADD CONSTRAINT` desnudo en el re-run no-op abortaría el
--   deploy (mismo patrón que 003/105, ver
--   `automation/tools/lint-migration-rerun-lock-safety.sh`).
--   Por eso el DDL vive dentro de un `DO $$` cuyo `IF` lee SÓLO catálogos
--   (`pg_constraint`): en el re-run no-op no se pide NINGÚN lock de tabla.

DO $$
DECLARE
  v_def TEXT;
  v_vocab CONSTANT TEXT := $vocab$
    'detected','validated','simulated','scored','executing',
    'executed','reconciled','rejected','failed','paper_executed'
  $vocab$;
BEGIN
  IF to_regclass('public.opportunities') IS NULL THEN
    -- Boot de un runner antes de 003: nada que ampliar todavía.
    RAISE NOTICE 'A3 paper_executed: opportunities no existe todavía (skip)';
    RETURN;
  END IF;

  SELECT pg_get_constraintdef(c.oid) INTO v_def
    FROM pg_constraint c
   WHERE c.conrelid = 'public.opportunities'::regclass
     AND c.conname = 'opportunities_status_check';

  IF v_def IS NULL THEN
    -- Sin CHECK (DB intervenida): lo creamos con el vocabulario completo. En un
    -- re-run posterior v_def ya contendrá 'paper_executed' y no se toca nada.
    EXECUTE format(
      'ALTER TABLE opportunities ADD CONSTRAINT opportunities_status_check CHECK (status IN (%s))',
      v_vocab
    );
    RAISE NOTICE 'A3 paper_executed: CHECK ausente, creado con el vocabulario completo';
  ELSIF position('paper_executed' in v_def) = 0 THEN
    EXECUTE 'ALTER TABLE opportunities DROP CONSTRAINT opportunities_status_check';
    EXECUTE format(
      'ALTER TABLE opportunities ADD CONSTRAINT opportunities_status_check CHECK (status IN (%s))',
      v_vocab
    );
    RAISE NOTICE 'A3 paper_executed: CHECK ampliado';
  ELSE
    RAISE NOTICE 'A3 paper_executed: ya presente (no-op, sin locks)';
  END IF;
END $$;

COMMENT ON COLUMN opportunities.status IS
  'Estado del pipeline. `executed` = inclusión on-chain real (escritor único: '
  'relays-client tras wait_for_inclusion). `paper_executed` = el camino paper '
  'pasó simulación (passed=true) y neto>0 con evidencia en paper_trade_runs '
  '(A3, migración 127). Son estados DISTINTOS a propósito.';
