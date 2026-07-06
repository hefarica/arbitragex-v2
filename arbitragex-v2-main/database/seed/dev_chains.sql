-- ArbitrageX v2 — DEV-ONLY seed
-- Do not execute in staging or prod. This only pre-loads trivially public constants.
-- bootstrap.sh refuses to run this unless $ENV == 'development'.

-- No table inserts here yet in S1: chains and relays are configured via configs/app.toml.
-- This file exists so automation/scripts/seed-dev.sh has a target; S2 will populate when
-- lookup tables are added (chains_catalog, relays_catalog) as needed.

SELECT 'dev-seed placeholder: no rows inserted in S1' AS note;
