# HP-08 (2026-09-08) — verify: espejo SKILL.md **Secondary** vs kg op_32 (read-only)
import json, re, glob, os

edges = [json.loads(l) for l in open('skills/arbitragex-ultra/knowledge_graph.jsonl', encoding='utf-8') if l.strip()]
op32_kg = set(e['from'] for e in edges if e['to'] == 'op_32')
bad_format, missing, extra = [], [], []
for p in glob.glob('skills/arbitragex-ultra/strategies/*/SKILL.md'):
    sid = os.path.basename(os.path.dirname(p))
    txt = open(p, encoding='utf-8').read()
    m = re.search(r'\*\*Secondary\*\*:?\s*(.+)', txt)
    has32 = bool(m and 'op_32' in m.group(1))
    if sid in op32_kg and not has32:
        missing.append(sid)
    if sid not in op32_kg and has32:
        extra.append(sid)
    if m:
        toks = re.findall(r'op_\d+', m.group(1))
        nums = [int(t.split('_')[1]) for t in toks]
        if nums != sorted(nums):
            bad_format.append(sid)
print('kg-op32 estrategias:', len(op32_kg))
print('SKILL.md Secondary SIN op_32 (debe []):', missing)
print('SKILL.md Secondary CON op_32 no-justificado (debe []):', extra)
print('SKILL.md Secondary no-ascendente (debe []):', bad_format)
