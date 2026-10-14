#!/usr/bin/env node
/**
 * assert-mandatory-cases.mjs — cierre POR NOMBRE para `verifier-policy-tests.yml`.
 *
 * Que cierra (VERIFIER-MANDATORY-CASES-01, brecha medida): el validador anterior
 * sumaba `# tests`, `# pass`, `# fail` del TAP y solo rechazaba con `tests === 0`
 * o `fail !== 0`. Con eso:
 *   - una suite con TODOS los casos en `test.skip` APROBABA con `TOTAL_PASS=0`;
 *   - una suite SIN ninguna llamada a `test` APROBABA, porque Node emite el
 *     ARCHIVO como subtest aprobado (`ok 1 - /ruta/archivo.test.cjs`) y con eso
 *     `# tests 1`, `# pass 1`;
 *   - un caso obligatorio en `test.todo` APROBABA (el resumen trae `# todo 1`,
 *     que nadie leia).
 * Exigir `pass > 0` NO alcanza: el caso "archivo sin casos" sube `pass` sin que
 * exista una sola prueba (medido: `# tests 1 / # pass 1` con cero `test()`).
 *
 * Que hace ahora: cada caso obligatorio esta ENUMERADO POR NOMBRE en
 * `scripts/verification/mandatory-cases.json`, y se verifica su resultado
 * INDIVIDUAL en el TAP. Un caso obligatorio AUSENTE, `skipped`, `todo`,
 * cancelado o fallido es un RECHAZO, con el nombre exacto en el mensaje.
 *
 * Lo que NO hace: no exige un CONTEO (el manifiesto es una cota inferior: se
 * pueden ANADIR casos libremente; los extra se reportan y no rompen nada). No
 * congela cifras: congela NOMBRES, que es lo que el contrato pide.
 *
 * Uso:
 *   node scripts/verification/assert-mandatory-cases.mjs [evidenceDir] [manifestPath]
 *     evidenceDir   por defecto `verifier-evidence`
 *     manifestPath  por defecto `./mandatory-cases.json` (junto a este script)
 *
 * Salida: exit 0 solo si TODOS los casos obligatorios existen y pasan.
 * Escribe `mandatory-cases.txt` en evidenceDir como evidencia.
 */
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const evidenceDir = process.argv[2] || 'verifier-evidence';
const manifestPath = process.argv[3] || path.join(HERE, 'mandatory-cases.json');

/** Estados que NO cuentan como cobertura aprobada. */
const NOT_COVERAGE = new Set(['SKIPPED', 'TODO', 'CANCELLED', 'FAILED']);

/**
 * Parsea el TAP de `node --test --test-reporter=tap`.
 * Devuelve los resultados POR NOMBRE y aparte los pseudo-tests de archivo
 * (que Node emite cuando el archivo no llama a `test()`).
 */
export function parseTap(tap) {
  const byName = new Map(); // name -> array de estados, en orden de aparicion
  const pseudo = [];
  for (const raw of tap.split('\n')) {
    const m = raw.match(/^\s*(not ok|ok) \d+ - (.*)$/);
    if (!m) continue;
    const ok = m[1] === 'ok';
    let rest = m[2];
    let directive = null;
    const dm = rest.match(/ # (SKIP|TODO|CANCELLED)\s*$/i);
    if (dm) {
      directive = dm[1].toUpperCase();
      rest = rest.slice(0, dm.index);
    }
    const name = rest.trim();
    // Un "caso" cuyo nombre es una RUTA de archivo es el pseudo-test que Node
    // sintetiza para el archivo: NUNCA cuenta como prueba.
    const looksLikeFile = /\.(c|m)?js$/.test(name) && /[\\/]/.test(name);
    if (looksLikeFile) {
      pseudo.push(name);
      continue;
    }
    let status;
    if (!ok) status = 'FAILED';
    else if (directive === 'SKIP') status = 'SKIPPED';
    else if (directive === 'TODO') status = 'TODO';
    else if (directive === 'CANCELLED') status = 'CANCELLED';
    else status = 'PASSED';
    if (!byName.has(name)) byName.set(name, []);
    byName.get(name).push(status);
  }
  return { byName, pseudo };
}

function main() {
  const refusals = [];
  const lines = [];

  if (!existsSync(manifestPath)) {
    console.error(`REFUSED: falta el manifiesto de casos obligatorios: ${manifestPath}`);
    console.error('Sin manifiesto no hay casos obligatorios que exigir: eso NO es un pase.');
    process.exit(1);
  }
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  const files = manifest.files || {};

  let totalRequired = 0;
  let totalVerified = 0;
  let totalExtra = 0;
  let filesChecked = 0;

  for (const [file, required] of Object.entries(files)) {
    const tapPath = path.join(evidenceDir, `${path.basename(file)}.tap`);
    if (!existsSync(tapPath)) {
      refusals.push(`REFUSED: falta la evidencia TAP del archivo obligatorio ${file} (esperada en ${tapPath}).`);
      continue;
    }
    filesChecked += 1;
    const { byName, pseudo } = parseTap(readFileSync(tapPath, 'utf8'));

    // Guarda dura: un archivo que solo produce el pseudo-test de archivo es el
    // caso "suite sin ninguna llamada a test()", que el validador viejo aprobaba.
    if (byName.size === 0) {
      refusals.push(
        `REFUSED: ${file} no ejecuto NINGUN caso real (0 nombres de caso en el TAP).` +
          (pseudo.length ? ` Solo aparecio el pseudo-test de archivo: ${pseudo.map((p) => path.basename(p)).join(', ')}.` : '') +
          ' Un archivo sin llamadas a test() NO es cobertura.',
      );
      continue;
    }

    for (const name of required) {
      totalRequired += 1;
      const statuses = byName.get(name);
      if (!statuses) {
        refusals.push(`REFUSED: MANDATORY_CASE_ABSENT: ${file} :: "${name}"`);
        continue;
      }
      const bad = statuses.filter((s) => NOT_COVERAGE.has(s));
      if (bad.length) {
        refusals.push(
          `REFUSED: MANDATORY_CASE_${bad[0]}: ${file} :: "${name}"` +
            (statuses.length > 1 ? ` (estados observados: ${statuses.join(',')})` : ''),
        );
        continue;
      }
      totalVerified += 1;
    }

    // Cualquier caso fallido, aunque no sea obligatorio, sigue siendo un rechazo
    // (conserva la propiedad del validador anterior: `fail !== 0`).
    for (const [name, statuses] of byName) {
      if (statuses.includes('FAILED') && !required.includes(name)) {
        refusals.push(`REFUSED: CASE_FAILED: ${file} :: "${name}"`);
      }
    }

    const extra = [...byName.keys()].filter((n) => !required.includes(n));
    totalExtra += extra.length;
    lines.push(
      `${file} :: casos_reales=${byName.size} obligatorios=${required.length} ` +
        `verificados=${required.filter((n) => (byName.get(n) || []).every((s) => s === 'PASSED')).length} extra=${extra.length}`,
    );
  }

  for (const l of lines) console.log(l);
  const totals = `MANDATORY_FILES=${filesChecked}\nMANDATORY_REQUIRED=${totalRequired}\nMANDATORY_VERIFIED=${totalVerified}\nMANDATORY_EXTRA=${totalExtra}\n`;
  try {
    writeFileSync(path.join(evidenceDir, 'mandatory-cases.txt'), totals);
  } catch {
    /* la evidencia es deseable, no bloqueante */
  }
  process.stdout.write(`MANDATORY_REQUIRED=${totalRequired} MANDATORY_VERIFIED=${totalVerified} MANDATORY_EXTRA=${totalExtra}\n`);

  if (refusals.length) {
    for (const r of refusals) console.error(r);
    console.error(`\nREFUSED: ${refusals.length} problema(s) de casos obligatorios. Esto es un RECHAZO, no un pase.`);
    process.exit(1);
  }
  console.log('OK: todos los casos obligatorios existen y pasan (por nombre, no por conteo).');
}

main();
