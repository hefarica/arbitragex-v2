#!/usr/bin/env bash
# DECIMALS-CYCLE-01 (t197) — corridas encadenadas: BASE (sin parche) y MUTANTE.
# Comparten el CARGO_TARGET_DIR (los crates del workspace se re-fingerprintan
# por el touch de run.sh; las deps del registro siguen en cache).
set -u
cd /root/dec01

echo "=== [1/2] BASE: arbol 858b943f SIN parche ==="
rm -rf backend
tar -xzf backend-head.tar.gz
bash run.sh base > host-base.log 2>&1
echo "BASE_EXIT=$?"
tail -4 host-base.log

echo "=== [2/2] MUTANTE: arbol con parche + default 18 restaurado ==="
rm -rf backend
tar -xzf dec01-858.tgz
bash run.sh mutant > host-mutant.log 2>&1
echo "MUTANT_EXIT=$?"
tail -4 host-mutant.log

echo "=== disco ==="
df -h / | tail -1
echo "FIN_CHAIN"
