"""Schema drift diff: repo migrations (expected) vs deployed PG (observed).

RULE 00: deterministic parse, no invented columns. Dynamic-SQL migrations are
flagged MANUAL-REVIEW instead of guessed.
"""
import os, re, sys, json
from collections import defaultdict

MIG_DIR = os.path.join(os.path.dirname(__file__), "..", "..", "database", "migrations")
DEPLOYED = os.path.join(os.path.dirname(__file__), "deployed_schema.tsv")

COL = re.compile(r"^\s{2,}([a-z_][a-z0-9_]*)\s+(?!PRIMARY|FOREIGN|UNIQUE|CHECK|CONSTRAINT|EXCLUDE)([A-Za-z]+.*)$")
CREATE = re.compile(r"CREATE TABLE\s+(?:IF NOT EXISTS\s+)?(?:public\.)?([a-z_][a-z0-9_]*)\s*\(", re.I)
ALTER_ADD = re.compile(r"ALTER TABLE\s+(?:IF EXISTS\s+)?(?:public\.)?([a-z_][a-z0-9_]*)\s+(?:ADD COLUMN\s+)(?:IF NOT EXISTS\s+)?([a-z_][a-z0-9_]*)", re.I)
DROP_COL = re.compile(r"ALTER TABLE\s+(?:IF EXISTS\s+)?(?:public\.)?([a-z_][a-z0-9_]*)\s+DROP COLUMN\s+(?:IF EXISTS\s+)?([a-z_][a-z0-9_]*)", re.I)
DROP_TABLE = re.compile(r"DROP TABLE\s+(?:IF EXISTS\s+)?(?:public\.)?([a-z_][a-z0-9_]*)", re.I)
RENAME = re.compile(r"ALTER TABLE\s+(?:IF EXISTS\s+)?(?:public\.)?([a-z_][a-z0-9_]*)\s+RENAME(?: COLUMN)?\s+([a-z_][a-z0-9_]*)\s+TO\s+([a-z_][a-z0-9_]*)", re.I)

expected = defaultdict(set)
manual_review = []

files = sorted(os.listdir(MIG_DIR), key=lambda f: (len(f.split('_')[0]), f))
for fn in files:
    if not fn.endswith(".sql"):
        continue
    src = open(os.path.join(MIG_DIR, fn), encoding="utf-8", errors="replace").read()
    # strip line comments to reduce noise
    src_nc = "\n".join(l.split("--")[0] for l in src.splitlines())
    if "EXECUTE" in src_nc or "DO $$" in src_nc:
        manual_review.append(fn)
    # naive paren-depth split of CREATE TABLE bodies
    for m in CREATE.finditer(src_nc):
        table = m.group(1)
        depth, i = 1, m.end() - 1
        body = []
        while i < len(src_nc) and depth > 0:
            ch = src_nc[i]
            if ch == "(": depth += 1
            elif ch == ")": depth -= 1
            if depth > 0: body.append(ch)
            i += 1
        for line in "".join(body).split("\n"):
            cm = COL.match(line)
            if cm:
                expected[table].add(cm.group(1))
    for m in ALTER_ADD.finditer(src_nc):
        expected[m.group(1)].add(m.group(2))
    for m in DROP_COL.finditer(src_nc):
        expected[m.group(1)].discard(m.group(2))
    for m in DROP_TABLE.finditer(src_nc):
        expected.pop(m.group(1), None)
    for m in RENAME.finditer(src_nc):
        expected[m.group(1)].discard(m.group(2))
        expected[m.group(1)].add(m.group(3))

deployed = defaultdict(set)
for line in open(DEPLOYED, encoding="utf-8"):
    parts = line.rstrip("\n").split("\t")
    if len(parts) >= 2:
        deployed[parts[0]].add(parts[1])

missing_on_db = {}   # expected columns not in deployed table
orphan_cols = {}     # deployed columns not derivable from migrations
missing_tables = set(expected) - set(deployed)
orphan_tables = set(deployed) - set(expected)

for t in sorted(set(expected) & set(deployed)):
    miss = expected[t] - deployed[t]
    if miss:
        missing_on_db[t] = sorted(miss)
    orph = deployed[t] - expected[t]
    if orph:
        orphan_cols[t] = sorted(orph)

print("== TABLES expected-but-missing-on-DB ==")
for t in sorted(missing_tables): print(f"  {t}  ({len(expected[t])} cols in migrations)")
print("== TABLES on-DB-but-not-in-migrations ==")
for t in sorted(orphan_tables): print(f"  {t}  ({len(deployed[t])} cols deployed)")
print("== COLUMNS expected-but-missing-on-DB (drift tipo A) ==")
for t, cols in missing_on_db.items(): print(f"  {t}: {cols}")
print("== COLUMNS deployed-but-not-derivable-from-migrations (tipo B) ==")
for t, cols in orphan_cols.items(): print(f"  {t}: {cols}")
print("== Migrations with dynamic SQL (MANUAL-REVIEW) ==")
print(" ", len(manual_review), "files:", ", ".join(manual_review[:40]))
