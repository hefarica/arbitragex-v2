#!/usr/bin/env bash
# DECIMALS-CYCLE-01 (t197) — verificacion + falsificador en el VPS con la
# imagen Rust AISLADA (rust:1.91, ya presente: no se instala nada en el host).
# Uso: bash /root/dec01/run.sh fix|mutant
set -u
MODE="${1:-fix}"
T=/root/dec01
mkdir -p "$T/logs"
cp -a "$T/backend/searcher-rs/src/orchestrator.rs" "$T/orchestrator.rs.base"
# DEFECTO DE INSTRUMENTO declarado: el CARGO_TARGET_DIR es compartido entre
# corridas y cargo decide por mtime. Al cambiar de ARBOL (858b943f -> main) con
# los mismos paths, reutiliza artefactos del arbol ANTERIOR y produce errores
# fantasma (45 errores de `shared_rs::candidates::EconomicsBasis` en main, que
# SI existe en main/shared-rs/src/candidates.rs:63). Se fuerza el re-fingerprint
# de los crates del workspace; las deps del registro siguen en cache.
find "$T/backend" -type f \( -name '*.rs' -o -name 'Cargo.toml' -o -name 'Cargo.lock' \) -exec touch {} +
echo "TOUCH_OK"

if [ "$MODE" = "mutant" ]; then
  python3 "$T/mutate-decimals-cycle-01.py" "$T/backend"
  echo -n "MUT_ANCHOR="; grep -c "MUTANTE (falsificador t197)" "$T/backend/searcher-rs/src/orchestrator.rs"
fi

docker run --rm \
  -v "$T":/w -w /w/backend \
  -e CARGO_TARGET_DIR=/w/target \
  -e RUSTFLAGS=-Dwarnings \
  -e MODE="$MODE" \
  -e CARGO_TERM_COLOR=never \
  rust:1.91 \
  bash -c '
set -u
export PATH="/usr/local/cargo/bin:$PATH"
L=/w/logs
echo "MODE=$MODE"
cargo --version; rustc --version; (gcc --version | head -1)
echo "=== instalar rustfmt (la imagen rust:1.91 no lo trae) ==="
rustup component add rustfmt > /tmp/rustfmt-install.log 2>&1; echo "RUSTFMT_INSTALL_EXIT=$?"
tail -2 /tmp/rustfmt-install.log
echo
echo "=== (V1a) cargo fmt --all -- --check  [COMANDO LITERAL DEL CONTRATO] ==="
cargo fmt --all -- --check > $L/fmt_all-$MODE.log 2>&1; echo "FMT_ALL_EXIT=$?"
echo -n "FICHEROS_CON_DIFF="; grep -c "^Diff in" $L/fmt_all-$MODE.log
grep -E "^Diff in" $L/fmt_all-$MODE.log | sort -u
echo
echo "=== (V1b) fmt ACOTADO al crate tocado (searcher-rs) ==="
cargo fmt -p searcher-rs -- --check > $L/fmt_scope-$MODE.log 2>&1; echo "FMT_SCOPE_EXIT=$?"
head -8 $L/fmt_scope-$MODE.log
echo
echo "=== (V2) RUSTFLAGS=-Dwarnings cargo check --manifest-path searcher-rs/Cargo.toml ==="
RUSTFLAGS=-Dwarnings cargo check --manifest-path searcher-rs/Cargo.toml > $L/check-$MODE.log 2>&1; echo "CHECK_EXIT=$?"
tail -4 $L/check-$MODE.log
echo
echo "=== (V3f) focalizado: los 3 tests del falsificador ==="
cargo test --manifest-path searcher-rs/Cargo.toml --lib -- decimals_cycle > $L/focal-$MODE.log 2>&1; echo "FOCAL_EXIT=$?"
grep -E "^test decimals_cycle|^test result:" $L/focal-$MODE.log
echo
echo "=== (V3) cargo test --manifest-path searcher-rs/Cargo.toml --no-fail-fast ==="
cargo test --manifest-path searcher-rs/Cargo.toml --no-fail-fast > $L/test-$MODE.log 2>&1; echo "TEST_EXIT=$?"
grep -E "^test result:" $L/test-$MODE.log | tail -25
echo -n "SUITES="; grep -c "^test result:" $L/test-$MODE.log
awk "/^test result:/ {for(i=1;i<=NF;i++){if(\$i==\"passed\")p+=\$(i-1); if(\$i==\"failed\")f+=\$(i-1)}} END{printf \"PASSED_TOTAL=%d FAILED_TOTAL=%d\n\", p, f}" $L/test-$MODE.log
echo "FIN_$MODE"
'
