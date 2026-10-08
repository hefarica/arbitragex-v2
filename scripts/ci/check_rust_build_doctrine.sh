#!/usr/bin/env bash
# GATE: doctrina de compilacion Rust en Windows (AppControl 4551).
#
# POR QUE EXISTE (2026-09-29): la regla 4 de la seccion 36 de CLAUDE.md afirmaba
# que "un worktree fresco tiene target/ frio -> cargo check falla por Windows
# AppControl (os error 4551)" y recomendaba compilar en el arbol principal con
# target/ caliente. La causa es FALSA y costo tiempo real: el bloqueo es Smart App
# Control en enforcement, cuyo criterio es FIRMA + REPUTACION, no la temperatura
# del target/. El binario bloqueado es el build-script que cargo compila nuevo en
# CADA build, en un target/ caliente o frio, y el arbol principal falla igual.
#
# La creencia se replico en planes y notas ("CARGO_TARGET_DIR al target caliente
# del arbol principal"), y ese consejo no arregla nada. Este gate impide que el
# texto viejo vuelva a entrar. La correccion vive en CLAUDE.md seccion 36 regla 4
# y el procedimiento correcto en docs/development/WSL2-RUST.md.
#
# Contrato: el texto viejo NO debe existir en el arbol (fuera de audits/ y de las
# notas historicas), SALVO la linea que lo cita para corregirlo, que se reconoce
# porque contiene su propio marcador de correccion.
set -uo pipefail

# Fragmento prohibido: la atribucion causal falsa. Se arma por concatenacion para
# que este mismo archivo no sea la coincidencia que busca.
NEEDLE_HEAD='worktree fresco tiene'
NEEDLE_TAIL='fr'
NEEDLE="${NEEDLE_HEAD} \`target/\` ${NEEDLE_TAIL}"

echo "gate: buscando la atribucion causal falsa en el arbol rastreado"
echo "      fragmento: ${NEEDLE}"

# FAIL-CLOSED: si algo falla, el gate falla (no queremos un verde por error del gate).
set -e

# Un `git grep` que falla (arbol no-git, indice roto, ruta ilegible) NO debe
# producir un verde. Se exige que el repo sea valido ANTES de buscar.
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "FALLO DEL GATE: no estamos dentro de un arbol git valido; no se puede verificar."
  echo "  cwd: $(pwd)"
  exit 1
fi

hits=""
# git grep sale con 1 cuando NO hay coincidencias: eso es el caso bueno, no un error.
if git grep -q -F -- "$NEEDLE" -- '*.md' '*.yml' '*.yaml' '*.sh'; then
  hits="$(git grep -n -F -- "$NEEDLE" -- '*.md' '*.yml' '*.yaml' '*.sh')"
fi

if [ -z "$hits" ]; then
  echo "OK: la atribucion falsa no aparece en el arbol."
  exit 0
fi

# Permitido: la linea que cita el texto viejo para corregirlo. Se identifica por
# llevar el marcador de correccion en la MISMA linea.
#
# `grep -v` sale con 1 cuando TODAS las lineas quedan filtradas: ese es el caso
# bueno. Se evalua en un `if` para que ese 1 no aborte el script (con `set -e` un
# `|| true` en la asignacion tambien mataba la salida con violaciones: el gate
# reportaba pero terminaba en 0, que es un gate inutil).
violations=""
if printf '%s\n' "$hits" | grep -q -v -F 'CORREGIDO 2026-09-29'; then
  violations="$(printf '%s\n' "$hits" | grep -v -F 'CORREGIDO 2026-09-29')"
fi

if [ -z "$violations" ]; then
  echo "OK: la unica aparicion es la linea de correccion (lleva su marcador)."
  exit 0
fi

echo
echo "FALLO: la atribucion causal falsa reaparecio sin su correccion."
echo
printf '%s\n' "$violations"
echo
echo "Por que importa: la causa real es Smart App Control (firma + reputacion),"
echo "NO el target/ frio. El binario bloqueado es el build-script que cargo compila"
echo "nuevo en cada build, en un target/ caliente o frio; el arbol principal falla"
echo "igual. Compilar en el arbol principal NO arregla nada."
echo
echo "La via correcta es WSL2. Ver:"
echo "  - CLAUDE.md seccion 36, regla 4 (texto corregido)"
echo "  - docs/development/WSL2-RUST.md (procedimiento)"
echo "  - docs/audits/E2E-AUDIT-2026-09-29.md (evidencia medida)"
echo
echo "Si estas citando el texto viejo para corregirlo, agrega el marcador"
echo "'CORREGIDO 2026-09-29' en esa misma linea."
exit 1
