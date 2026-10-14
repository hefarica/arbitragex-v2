#!/usr/bin/env node
// =============================================================================
// CONTRACTS-PATCHGEN-01 — normalise a raw `git diff --no-index` into an appliable patch
// =============================================================================
// Why this exists: piping git's diff through PowerShell (ForEach-Object | Set-Content)
// rewrites every line ending to CRLF, and `git apply` then rejects the patch with
// "patch does not apply" because the target file uses LF. That failure was MEASURED
// (the first generated patch failed `git apply --check` for exactly this reason), not
// assumed.
//
// This step takes the RAW diff bytes (produced via `cmd /c ... > file`, which does not
// translate line endings), substitutes the two raw header paths for the repository path
// the patch must target, forces LF, and writes the result.
//
// Usage:
//   node CONTRACTS-PATCHGEN-01.mjs --raw <raw.diff> --out <patch> \
//        --rawA <path-as-it-appears-after-a/> --rawB <path-as-it-appears-after-b/> \
//        --path <repo-relative target path>
// =============================================================================

import { readFileSync, writeFileSync } from 'node:fs';

const arg = (k) => { const i = process.argv.indexOf(k); return i >= 0 ? process.argv[i + 1] : null; };
const rawPath = arg('--raw'), outPath = arg('--out'), rawA = arg('--rawA'), rawB = arg('--rawB'), target = arg('--path');
if (!rawPath || !outPath || !rawA || !rawB || !target) {
  console.error('usage: --raw <file> --out <file> --rawA <a/-path> --rawB <b/-path> --path <repo path>');
  process.exit(1);
}

let diff = readFileSync(rawPath, 'utf8');
const crlf = (diff.match(/\r\n/g) || []).length;
diff = diff.replace(/\r\n/g, '\n');
if (!diff.endsWith('\n')) diff += '\n';

const before = diff;
// Exactly two substitutions fix every header form at once: `diff --git a/X b/Y`,
// `--- a/X` and `+++ b/Y`.
diff = diff.split(`a/${rawA}`).join(`a/${target}`);
diff = diff.split(`b/${rawB}`).join(`b/${target}`);

const guards = [
  [/^diff --git a\S+ b\S+$/m, 'diff --git header'],
  [/^--- a\S+$/m, '--- header'],
  [/^\+\+\+ b\S+$/m, '+++ header'],
  [/^@@ -\d+(,\d+)? \+\d+(,\d+)? @@/m, 'at least one hunk header'],
];
for (const [re, what] of guards) {
  if (!re.test(diff)) { console.error(`ABORT: rewritten diff is missing ${what}. No output written.`); process.exit(1); }
}
if (diff === before && rawA !== target) { console.error('ABORT: no substitution happened — check --rawA/--rawB.'); process.exit(1); }

writeFileSync(outPath, diff, 'utf8');
const hunks = (diff.match(/^@@ /gm) || []).length;
console.log('CONTRACTS-PATCHGEN-01');
console.log(`  CRLF lines normalised : ${crlf}`);
console.log(`  hunks                 : ${hunks}`);
console.log(`  output                : ${outPath}`);
console.log(`  header                : ${diff.split('\n')[0]}`);
console.log(`  target header         : ${(diff.match(/^\+\+\+ .*/m) || [''])[0]}`);
