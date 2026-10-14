/**
 * mandatory-cases-guard.test.cjs — VERIFIER-MANDATORY-CASES-01
 *
 * Pruebas del VALIDADOR (`assert-mandatory-cases.mjs`). No prueban el producto:
 * prueban la CAPACIDAD DE DETECCION del cierre. Hermeticas: cada escenario crea
 * sus fixtures y su manifiesto en un directorio temporal y ejecuta el validador
 * como proceso hijo, afirmando el exit code y el NOMBRE del caso en el mensaje.
 *
 * Motivo (brecha medida, no supuesta): el validador anterior aprobaba una suite
 * con todos los casos en `test.skip` (con `TOTAL_PASS=0`) y aprobaba un archivo
 * SIN ninguna llamada a `test`, que Node contabiliza como subtest aprobado. Y
 * `pass > 0` no alcanza: ese archivo produce `# pass 1` con cero pruebas.
 */
const { test } = require('node:test');
const assert = require('node:assert');
const { execFileSync, spawnSync } = require('node:child_process');
const { mkdtempSync, writeFileSync, readFileSync, mkdirSync, existsSync } = require('node:fs');
const { tmpdir } = require('node:os');
const path = require('node:path');

const VALIDATOR = path.join(__dirname, 'assert-mandatory-cases.mjs');

/**
 * Entorno SANEADO para lanzar `node --test` desde dentro de otro `node --test`.
 *
 * TRAMPA MEDIDA (no teorica): Node propaga `NODE_TEST_CONTEXT=child-v8` a los
 * procesos hijos. Si el hijo es otro `node --test`, Node se niega a correrlo
 * ("node:test run() is being called recursively within a test file. skipping
 * running files.") y **no emite TAP**: stdout vacio, exit 0. Sin sanear, todos
 * los escenarios de este archivo verian TAP vacio y el validador rechazaria por
 * "0 casos reales" — enmascarando la prueba. El caso `TRAMPA` de abajo lo fija.
 */
const CLEAN_ENV = (() => {
  const e = { ...process.env };
  delete e.NODE_TEST_CONTEXT;
  return e;
})();

/** Ejecuta `node --test --test-reporter=tap` sobre un fixture y devuelve su TAP. */
function tapOf(fixturePath) {
  try {
    return execFileSync(process.execPath, ['--test', '--test-reporter=tap', fixturePath], {
      encoding: 'utf8',
      env: CLEAN_ENV,
    });
  } catch (e) {
    return (e.stdout || '') + (e.stderr || '');
  }
}

/**
 * Escenario: crea un evidenceDir con los TAP de los fixtures dados y un
 * manifiesto que exige `required` para el archivo `key`.
 */
function scenario({ fixtures, key = 'scripts/verification/fixture.test.cjs', required, noManifest = false, omitTap = false }) {
  const root = mkdtempSync(path.join(tmpdir(), 'mandatory-'));
  const evidenceDir = path.join(root, 'verifier-evidence');
  mkdirSync(evidenceDir, { recursive: true });
  const manifestPath = path.join(root, 'mandatory-cases.json');
  // `omitTap` se compara por BASENAME: la clave del manifiesto es una ruta
  // (`scripts/verification/x.test.cjs`) y el fixture vive suelto en el temp.
  const skipTapFor = omitTap ? path.basename(key) : null;
  for (const [name, src] of Object.entries(fixtures)) {
    const p = path.join(root, name);
    writeFileSync(p, src);
    if (!(skipTapFor && path.basename(name) === skipTapFor)) {
      writeFileSync(path.join(evidenceDir, `${path.basename(name)}.tap`), tapOf(p));
    }
  }
  if (!noManifest) writeFileSync(manifestPath, JSON.stringify({ files: { [key]: required } }, null, 2));
  const run = spawnSync(process.execPath, [VALIDATOR, evidenceDir, manifestPath], { encoding: 'utf8', env: CLEAN_ENV });
  return { root, evidenceDir, manifestPath, code: run.status, out: (run.stdout || '') + (run.stderr || '') };
}

const MANIFEST_KEY = 'scripts/verification/fixture.test.cjs';

// ─── CONTROL POSITIVO ────────────────────────────────────────────────────────
test('POSITIVO: el juego completo de casos obligatorios pasa', () => {
  const s = scenario({
    fixtures: {
      'fixture.test.cjs': `
        const { test } = require('node:test');
        const assert = require('node:assert');
        test('obligatorio A', () => assert.equal(1, 1));
        test('obligatorio B', () => assert.equal(2, 2));
      `,
    },
    key: MANIFEST_KEY,
    required: ['obligatorio A', 'obligatorio B'],
  });
  assert.equal(s.code, 0, `el validador debio aprobar. salida:\n${s.out}`);
  assert.match(s.out, /OK: todos los casos obligatorios existen y pasan/);
});

// ─── NEGATIVOS ───────────────────────────────────────────────────────────────
test('NEGATIVO: un caso obligatorio AUSENTE rompe el check, y lo nombra', () => {
  const s = scenario({
    fixtures: {
      'fixture.test.cjs': `
        const { test } = require('node:test');
        const assert = require('node:assert');
        test('obligatorio A', () => assert.equal(1, 1));
      `,
    },
    key: MANIFEST_KEY,
    required: ['obligatorio A', 'obligatorio B'],
  });
  assert.equal(s.code, 1, `el validador debio rechazar. salida:\n${s.out}`);
  assert.match(s.out, /MANDATORY_CASE_ABSENT/);
  assert.match(s.out, /obligatorio B/, 'el mensaje debe NOMBRAR el caso ausente');
});

test('NEGATIVO: un caso obligatorio SKIPPED rompe el check, y lo nombra', () => {
  const s = scenario({
    fixtures: {
      'fixture.test.cjs': `
        const { test } = require('node:test');
        const assert = require('node:assert');
        test('obligatorio A', () => assert.equal(1, 1));
        test.skip('obligatorio B', () => assert.equal(2, 2));
      `,
    },
    key: MANIFEST_KEY,
    required: ['obligatorio A', 'obligatorio B'],
  });
  assert.equal(s.code, 1, `el validador debio rechazar. salida:\n${s.out}`);
  assert.match(s.out, /MANDATORY_CASE_SKIPPED/);
  assert.match(s.out, /obligatorio B/);
});

test('NEGATIVO: un caso obligatorio TODO rompe el check, y lo nombra', () => {
  const s = scenario({
    fixtures: {
      'fixture.test.cjs': `
        const { test } = require('node:test');
        const assert = require('node:assert');
        test('obligatorio A', () => assert.equal(1, 1));
        test.todo('obligatorio B');
      `,
    },
    key: MANIFEST_KEY,
    required: ['obligatorio A', 'obligatorio B'],
  });
  assert.equal(s.code, 1, `el validador debio rechazar. salida:\n${s.out}`);
  assert.match(s.out, /MANDATORY_CASE_TODO/);
  assert.match(s.out, /obligatorio B/);
});

test('NEGATIVO: un caso obligatorio FALLIDO rompe el check, y lo nombra', () => {
  const s = scenario({
    fixtures: {
      'fixture.test.cjs': `
        const { test } = require('node:test');
        const assert = require('node:assert');
        test('obligatorio A', () => assert.equal(1, 1));
        test('obligatorio B', () => assert.equal(1, 2));
      `,
    },
    key: MANIFEST_KEY,
    required: ['obligatorio A', 'obligatorio B'],
  });
  assert.equal(s.code, 1, `el validador debio rechazar. salida:\n${s.out}`);
  assert.match(s.out, /MANDATORY_CASE_FAILED/);
  assert.match(s.out, /obligatorio B/);
});

test('NEGATIVO: un tope sin `pass > 0` — una suite SIN ninguna llamada a test() es rechazada', () => {
  const s = scenario({
    fixtures: {
      'fixture.test.cjs': `
        // Ninguna llamada a test(): solo codigo de modulo (el caso del operador).
        const helper = (x) => x + 1;
        module.exports = { helper };
      `,
    },
    key: MANIFEST_KEY,
    required: ['obligatorio A'],
  });
  assert.equal(s.code, 1, `el validador debio rechazar. salida:\n${s.out}`);
  assert.match(s.out, /no ejecuto NINGUN caso real/);
  // Y la medicion que refuta `pass > 0`: el TAP del archivo sin casos trae pass=1.
  const tap = readFileSync(path.join(s.evidenceDir, 'fixture.test.cjs.tap'), 'utf8');
  const pass = Number([...tap.matchAll(/^# pass (\d+)$/gm)].pop()[1]);
  assert.ok(pass > 0, `el archivo sin casos debe traer # pass > 0 para refutar pass>0 (medido: ${pass})`);
  assert.match(tap, /^ok 1 - .*fixture\.test\.cjs$/m, 'Node emite el ARCHIVO como subtest aprobado');
});

test('NEGATIVO: falta la evidencia TAP de un archivo obligatorio', () => {
  const s = scenario({
    fixtures: {
      'fixture.test.cjs': `
        const { test } = require('node:test');
        const assert = require('node:assert');
        test('obligatorio A', () => assert.equal(1, 1));
      `,
    },
    key: MANIFEST_KEY,
    required: ['obligatorio A'],
    omitTap: true,
  });
  assert.equal(s.code, 1, `el validador debio rechazar. salida:\n${s.out}`);
  assert.match(s.out, /falta la evidencia TAP/);
});

test('NEGATIVO: falta el manifiesto de casos obligatorios', () => {
  const s = scenario({
    fixtures: {
      'fixture.test.cjs': `
        const { test } = require('node:test');
        const assert = require('node:assert');
        test('obligatorio A', () => assert.equal(1, 1));
      `,
    },
    key: MANIFEST_KEY,
    required: ['obligatorio A'],
    noManifest: true,
  });
  assert.equal(s.code, 1, `el validador debio rechazar. salida:\n${s.out}`);
  assert.match(s.out, /falta el manifiesto/);
});

// ─── CONSERVAR, NO CONGELAR ──────────────────────────────────────────────────
test('NO CONGELADO: anadir casos NUEVOS no rompe el check (el manifiesto es cota inferior)', () => {
  const s = scenario({
    fixtures: {
      'fixture.test.cjs': `
        const { test } = require('node:test');
        const assert = require('node:assert');
        test('obligatorio A', () => assert.equal(1, 1));
        test('caso NUEVO 1', () => assert.equal(3, 3));
        test('caso NUEVO 2', () => assert.equal(4, 4));
      `,
    },
    key: MANIFEST_KEY,
    required: ['obligatorio A'],
  });
  assert.equal(s.code, 0, `anadir casos debe seguir aprobando. salida:\n${s.out}`);
  assert.match(s.out, /MANDATORY_EXTRA=2/, 'los casos nuevos se reportan como extra, no como fallo');
});

// ─── TRAMPA MEDIDA (harness) ─────────────────────────────────────────────────
test('TRAMPA: `node --test` anidado SIN sanear el env produce TAP vacio (y no debe)', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'trap-'));
  const p = path.join(root, 'fixture.test.cjs');
  writeFileSync(p, `
    const { test } = require('node:test');
    const assert = require('node:assert');
    test('obligatorio A', () => assert.equal(1, 1));
  `);
  // Con NODE_TEST_CONTEXT heredado (como lo recibe un hijo del runner): NO hay TAP.
  const dirty = spawnSync(process.execPath, ['--test', '--test-reporter=tap', p], { encoding: 'utf8' });
  assert.equal((dirty.stdout || '').length, 0, 'Node no emite TAP en una corrida anidada sin sanear');
  assert.match(dirty.stderr || '', /recursively within a test file/, 'lo dice explicitamente por stderr');
  // Con el env saneado: TAP real, con el caso.
  const clean = tapOf(p);
  assert.match(clean, /^ok 1 - obligatorio A$/m, 'el harness DEBE sanear el env para obtener TAP real');
});
