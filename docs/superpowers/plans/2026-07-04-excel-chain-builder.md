# Excel Chain Builder — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the ArbitrageX_Unified_Config.xlsm the single source of truth for adding/removing chains — activating a chain from the Excel drives 5 dapp layers (.env RPC vars, ARBX_ENABLED_CHAINS, PG chains seed, PG factories seed, tokens reference) with multi-provider privacy.

**Architecture:** A new "Chain Builder" sheet reads the existing 3 source sheets (RPC Providers, _RED_lookup, RPC Parser). A Python script (`gen_chain_env.py`, extended from the existing `gen_rpc_env_from_xlsx.py`) generates the 5 artifacts. The existing VBA macro `RunFullSyncCycle` uploads them to the VPS (shred, no-print, paper_mode untouched). No VBA code is modified — the existing macro handles the upload; the new generation step is Python.

**Tech Stack:** openpyxl (Python, preserves VBA via `keep_vba=True`), oletools (VBA read/verify), Excel dark-mode styling (`FF141C30` fill / `FFF3F9FF` font), existing repo script `gen_rpc_env_from_xlsx.py`.

---

## File Structure

| File | Responsibility |
|------|----------------|
| `C:\Users\HFRC\Downloads\ArbitrageX_Unified_Config.xlsm` | The workbook being extended (chain builder sheet + corrected styling) |
| `scripts/arbx-env-deploy/gen_chain_env.py` | NEW — reads "Chain Builder" sheet, emits the 5 artifacts (RPC env, ENABLED_CHAINS, chains.sql, factories.sql, tokens ref) |
| `scripts/arbx-env-deploy/gen_rpc_env_from_xlsx.py` | EXISTING — the multi-provider CSV generator we extend/import from |
| `docs/gsim1-simulator-v2-ready-flip-checklist.md` | existing — referenced |

## Constraints (doctrinal, non-negotiable)

1. **`paper_mode` is never touched** by any step.
2. **Existing VBA macros are preserved byte-for-byte** (verified via oletools before/after every save).
3. **No secret values are printed** — only presence + length + structurally-derived content.
4. **Dark-mode styling** (`FF141C30` fill, `FFF3F9FF` font) overrides the skill `xlsx` industry color convention for this file (established pattern).
5. **The 3 source sheets (`RPC Providers`, `_RED_lookup`, `RPC Parser`) are read-only** — Chain Builder consumes them; never mutates.

---

### Task 0: Backup + forensic baseline (excel-vba-auditor PhD method)

**Files:**
- Read: `C:\Users\HFRC\Downloads\ArbitrageX_Unified_Config.xlsm`
- Create: `C:\tmp\xlsx_audit\` working dir

- [ ] **Step 1: Backup the workbook**

```bash
cp "C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm" \
   "C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm.bak.$(date -u +%Y%m%dT%H%M%SZ)"
```

- [ ] **Step 2: Forensic extraction (skill excel-vba-auditor)**

```bash
mkdir -p /c/tmp/xlsx_audit
cd /c/tmp/xlsx_audit
unzip -o "C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm" -d unzipped/
# sharedStrings = the DNA
cp unzipped/xl/sharedStrings.xml sharedStrings.xml
# VBA = the brain
python3 -c "from oletools.olevba import VBA_Parser; vp=VBA_Parser('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm'); [open(f'{n}.bas','w').write(c) for (_,_,n,c) in vp.extract_macros()]"
```

- [ ] **Step 3: Verify `RunFullSyncCycle` handles section headers starting with `#`**

```bash
grep -A5 "Sub RunFullSyncCycle" ArbxEnvDeploy.bas | head -20
grep -E "Left\(.*,\s*1\)\s*=\s*\"#\"|InStr.*\"#\"" ArbxEnvDeploy.bas
```

If the macro does NOT skip `#`-prefixed rows, we must use a different sentinel (e.g. a dedicated "skip" column) before inserting section headers. **Document the exact skip mechanism here before proceeding.**

- [ ] **Step 4: Capture dark-mode style baseline (exact fill/font)**

```bash
PYTHONIOENCODING=utf-8 python3 -c "
import openpyxl
wb = openpyxl.load_workbook('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm', keep_vba=True)
ws = wb['.env Production']
for r in [3, 10, 63]:
    a = ws.cell(r,1); b = ws.cell(r,2); c = ws.cell(r,3)
    print(f'r{r}: A.fill={a.fill.fgColor.rgb} A.font={a.font.color.rgb} bold={a.font.bold} size={a.font.size} | B.fill={b.fill.fgColor.rgb} | C.font={c.font.color.rgb}')
print('row height:', ws.row_dimensions[3].height)
print('col A width:', ws.column_dimensions['A'].width)
" | tee /c/tmp/xlsx_audit/style_baseline.txt
```

**Expected:** fill `FF141C30` (or `FF091123`), font `FFF3F9FF`, size 11. Save these exact values — every new cell we write uses them.

---

### Task 1: Correct prior additions to dark mode (Mainnet Deploy sheet + r139-142)

**Files:**
- Modify: `C:\Users\HFRC\Downloads\ArbitrageX_Unified_Config.xlsm` (Mainnet Deploy, .env Production r139-142)

- [ ] **Step 1: Write the restyle script**

Create `/c/tmp/restyle_additions.py`:

```python
import openpyxl
from openpyxl.styles import Font, PatternFill, Alignment

# Baseline captured in Task 0 Step 4 — use EXACT values
DARK_FILL = PatternFill('solid', fgColor='FF141C30')   # adjust to baseline
LIGHT_FONT = Font(color='FFF3F9FF', size=11)            # adjust to baseline
HEADER_FONT = Font(color='FFF3F9FF', size=11, bold=True)
WARN_FILL = PatternFill('solid', fgColor='FF3A2E1A')    # subtle amber tint for "value to fill" — dark-compatible

wb = openpyxl.load_workbook('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm', keep_vba=True)

# 1. Mainnet Deploy sheet — restyle all cells to dark
ws = wb['Mainnet Deploy']
for row in ws.iter_rows(min_row=1, max_row=ws.max_row, max_col=ws.max_column):
    for cell in row:
        if cell.row <= 3:  # headers
            cell.font = HEADER_FONT
        else:
            cell.font = LIGHT_FONT
        cell.fill = DARK_FILL
    # col B (value to fill) gets the subtle warn tint instead
    b = ws.cell(row[0].row, 2)
    if b.row > 3 and not b.value:
        b.fill = WARN_FILL

# 2. .env Production r139-142 — restyle the green outputs to dark
env = wb['.env Production']
for r in range(139, 143):
    for col in range(1, 5):
        c = env.cell(r, col)
        c.fill = DARK_FILL
        if col == 1:
            c.font = LIGHT_FONT
        else:
            c.font = LIGHT_FONT

wb.save('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm')
print('restyle OK')
```

- [ ] **Step 2: Run + verify macros + data preserved**

```bash
PYTHONIOENCODING=utf-8 python3 /c/tmp/restyle_additions.py
PYTHONIOENCODING=utf-8 python3 -c "
from oletools.olevba import VBA_Parser
vp = VBA_Parser('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm')
print('macros:', len(list(vp.extract_macros())))  # expect 16
import openpyxl
wb = openpyxl.load_workbook('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm', data_only=True, read_only=True)
env = wb['.env Production']
print('SIM_BACKEND r63 valor LLENO:', bool(env.cell(63,2).value))
print('ARBITRAGE_EXECUTOR r139 [DEPLOY-OUTPUT]:', str(env.cell(139,3).value or '')[:25])
print('Mainnet Deploy MULTISIG_ADDRESS r4 estado:', str(wb['Mainnet Deploy'].cell(4,3).value or '')[:25])
"
```

**Expected:** 16 macros, SIM_BACKEND still populated, ARBITRAGE_EXECUTOR still `[DEPLOY-OUTPUT]`, MULTISIG_ADDRESS still `[DEPLOY-INPUT]`. If any fails, restore from backup.

---

### Task 2: Catalog source data for Chain Builder (read-only)

**Files:**
- Read: `RPC Providers`, `_RED_lookup` sheets
- Create: `/c/tmp/chain_catalog.json`

- [ ] **Step 1: Extract the canonical chain catalog**

```bash
PYTHONIOENCODING=utf-8 python3 << 'PYEOF'
import openpyxl, json
wb = openpyxl.load_workbook('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm', data_only=True, read_only=True)

# 1. _RED_lookup: token_RED -> canonical Chain name
ws = wb['_RED_lookup']
red_to_chain = {}
for row in ws.iter_rows(min_row=2, values_only=True):
    if row[3] and row[4]:
        red_to_chain[str(row[3]).strip()] = str(row[4]).strip()

# 2. RPC Providers: group URLs by Chain + Protocolo + Proveedor
ws = wb['RPC Providers']
rpcs = {}  # chain -> {HTTP: {provider: url}, WSS: {provider: url}}
for row in ws.iter_rows(min_row=2, values_only=True):
    chain, proto, prov, url = (list(row)+[None]*4)[:4]
    if not all([chain, proto, prov, url]): continue
    chain = str(chain).strip(); proto = str(proto).strip()
    prov = str(prov).strip(); url = str(url).strip()
    rpcs.setdefault(chain, {'HTTP': {}, 'WSS': {}})
    rpcs[chain][proto][prov] = url

# 3. chain_id map (from gen_rpc_env_from_xlsx.py CHAIN_IDS — copy the exact dict)
CHAIN_IDS = {
    'Ethereum Mainnet':1,'Optimism':10,'BSC Mainnet':56,'Gnosis':100,
    'Polygon Mainnet':137,'Base':8453,'Arbitrum One':42161,'Avalanche':43114,
    'Linea':59144,'Scroll':534352,'Blast':81457,
}
# native_currency + explorer from dapp seed 043 + canonical
CHAIN_META = {
    1:('ETH','https://etherscan.io',12000),10:('ETH','https://optimistic.etherscan.io',2000),
    56:('BNB','https://bscscan.com',3000),100:('xDAI','https://gnosisscan.io',5000),
    137:('MATIC','https://polygonscan.com',2000),8453:('ETH','https://basescan.org',2000),
    42161:('ETH','https://arbiscan.io',250),43114:('AVAX','https://snowtrace.io',2000),
    59144:('ETH','https://lineascan.build',12000),534352:('ETH','https://scrollscan.com',3000),
    81457:('ETH','https://blastscan.io',2000),
}

catalog = {}
for chain_name, cid in CHAIN_IDS.items():
    if chain_name in rpcs:
        cur, explorer, block_ms = CHAIN_META.get(cid, ('?','?',0))
        catalog[chain_name] = {
            'chain_id': cid, 'native': cur, 'explorer': explorer, 'block_ms': block_ms,
            'rpc_http': rpcs[chain_name].get('HTTP', {}),
            'rpc_ws': rpcs[chain_name].get('WSS', {}),
        }
json.dump(catalog, open('/c/tmp/chain_catalog.json','w'), indent=2)
print(f'catalog: {len(catalog)} chains with both CHAIN_IDS + RPC Providers data')
for c,info in catalog.items(): print(f'  {c} ({info["chain_id"]}): HTTP={len(info["rpc_http"])} WSS={len(info["rpc_ws"])}')
PYEOF
```

- [ ] **Step 2: Cross-check against dapp seed 043 (which chains have factories)**

```bash
cd "c:/Users/HFRC/Desktop/arbitragex-v2-main (17)/arbitragex-v2-main"
grep -E "chain_id.*[0-9]|VALUES.*\([0-9]" database/migrations/043_seed_multichain_dexes_factories.sql | grep -oE "\([0-9]{1,6}," | tr -d '(,' | sort -un
# expected: 10, 56, 137, 8453, 42161 (the 5 chains with factories seeded)
```

Annotate `/c/tmp/chain_catalog.json` mentally: chains `10, 56, 137, 8453, 42161` = `factories_seeded=true`; all others = `false`.

---

### Task 3: Build the "Chain Builder" sheet

**Files:**
- Modify: `C:\Users\HFRC\Downloads\ArbitrageX_Unified_Config.xlsm` (new sheet "Chain Builder")

- [ ] **Step 1: Write the sheet-builder script**

Create `/c/tmp/build_chain_builder.py`:

```python
import openpyxl, json
from openpyxl.styles import Font, PatternFill, Alignment
from openpyxl.worksheet.datavalidation import DataValidation

DARK_FILL = PatternFill('solid', fgColor='FF141C30')
LIGHT_FONT = Font(color='FFF3F9FF', size=11)
HEADER_FONT = Font(color='FFF3F9FF', size=11, bold=True)
AMBER_FONT = Font(color='FFFFC000', size=11)   # "value to fill" cue (dark-compatible)
GREEN_FONT = Font(color='FF52C41A', size=11)   # "ready/active" cue
RED_FONT = Font(color='FFFF4D4F', size=11)     # "missing/blocker" cue

wb = openpyxl.load_workbook('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm', keep_vba=True)
if 'Chain Builder' in wb.sheetnames: del wb['Chain Builder']
# Position: right after "_RED_lookup" (logical — it consumes that sheet)
idx = wb.sheetnames.index('_RED_lookup') + 1
ws = wb.create_sheet('Chain Builder', index=idx)

# Title
ws['A1'] = 'Chain Builder — activation panel (mark ✓ in col B)'
ws['A1'].font = Font(color='FFF3F9FF', size=14, bold=True)
ws.merge_cells('A1:J1')

# Headers (row 3)
headers = ['Chain','✓ ACTIVE','chain_id','native','explorer','block_ms',
           'RPC_HTTP (multi-provider)','RPC_WS','factories in dapp?','Estado']
for i, h in enumerate(headers, 1):
    c = ws.cell(3, i, h); c.font = HEADER_FONT; c.fill = DARK_FILL
    c.alignment = Alignment(horizontal='center')

catalog = json.load(open('/c/tmp/chain_catalog.json'))
FACTORIES_SEEDED = {10,56,137,8453,42161}  # from Task 2 Step 2

for i, (chain, info) in enumerate(sorted(catalog.items(), key=lambda x: x[1]['chain_id']), start=4):
    cid = info['chain_id']
    seeded = cid in FACTORIES_SEEDED
    # col A chain name
    ws.cell(i, 1, chain).font = LIGHT_FONT
    # col B ACTIVE (operator marks ✓ manually — leave blank, amber cue)
    ws.cell(i, 2, '').font = AMBER_FONT
    # col C chain_id
    ws.cell(i, 3, cid).font = LIGHT_FONT
    # col D native
    ws.cell(i, 4, info['native']).font = LIGHT_FONT
    # col E explorer
    ws.cell(i, 5, info['explorer']).font = LIGHT_FONT
    # col F block_ms
    ws.cell(i, 6, info['block_ms']).font = LIGHT_FONT
    # col G RPC_HTTP multi-provider CSV — FORMULA referencing RPC Providers sheet would be ideal;
    # for now the gen_chain_env.py script builds it. Store count + first provider as preview.
    http = info['rpc_http']
    preview = ','.join(f'{k}=…' for k in list(http)[:3])
    ws.cell(i, 7, f'{len(http)} providers: {preview}').font = LIGHT_FONT
    # col H RPC_WS count
    ws.cell(i, 8, f'{len(info["rpc_ws"])} providers').font = LIGHT_FONT
    # col I factories in dapp?
    cell_i = ws.cell(i, 9, '✓ seeded' if seeded else '✗ missing')
    cell_i.font = GREEN_FONT if seeded else RED_FONT
    # col J Estado (formula: ready iff ACTIVE marked AND factories seeded)
    ws.cell(i, 10, f'=IF(B{i}="✓",IF(I{i}="✓ seeded","LISTO","PENDIENTE factories"),"inactiva")').font = LIGHT_FONT
    # apply dark fill to all cells in row
    for col in range(1, 11):
        ws.cell(i, col).fill = DARK_FILL
    ws.row_dimensions[i].height = 22

# Data validation: col B only accepts "✓" or empty
dv = DataValidation(type='list', formula1='"✓"', allow_blank=True)
dv.add(f'B4:B{3+len(catalog)}')
ws.add_data_validation(dv)

# Column widths
widths = {'A':22,'B':12,'C':10,'D':10,'E':32,'F':10,'G':50,'H':18,'I':18,'J':22}
for col, w in widths.items(): ws.column_dimensions[col].width = w

# Instructions at bottom
inst_row = 5 + len(catalog)
ws.cell(inst_row, 1, 'INSTRUCCIONES: marca ✓ en col B → corre python scripts/arbx-env-deploy/gen_chain_env.py → revisa fragment → RunFullSyncCycle (sube al VPS).').font = Font(color='FFF3F9FF', italic=True, size=10)
ws.merge_cells(start_row=inst_row, start_column=1, end_row=inst_row, end_column=10)

wb.save('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm')
print(f'Chain Builder sheet created with {len(catalog)} chains')
```

- [ ] **Step 2: Run + verify**

```bash
PYTHONIOENCODING=utf-8 python3 /c/tmp/build_chain_builder.py
PYTHONIOENCODING=utf-8 python3 -c "
from oletools.olevba import VBA_Parser
vp = VBA_Parser('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm')
print('macros still:', len(list(vp.extract_macros())))  # 16
import openpyxl
wb = openpyxl.load_workbook('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm', data_only=True, read_only=True)
print('Chain Builder in sheets:', 'Chain Builder' in wb.sheetnames)
ws = wb['Chain Builder']
print('rows:', ws.max_row, 'cols:', ws.max_column)
"
```

---

### Task 4: Write `gen_chain_env.py` (the generator, 5 artifacts)

**Files:**
- Create: `scripts/arbx-env-deploy/gen_chain_env.py`

- [ ] **Step 1: Implement the generator (imports logic from existing `gen_rpc_env_from_xlsx.py`)**

Create `scripts/arbx-env-deploy/gen_chain_env.py`:

```python
#!/usr/bin/env python3
"""
Generate the 5 chain artifacts from the Excel "Chain Builder" sheet.

Reads:  Chain Builder col B (✓ ACTIVE) + the 3 source sheets
Emits (to stdout + gitignored fragments for review):
  1. rpc_chains_fragment.env      — RPC_HTTP_<id> + RPC_WS_<id> (multi-provider, SHUFFLED)
  2. enabled_chains_fragment.env  — ARBX_ENABLED_CHAINS=<comma list of active chain_ids>
  3. chains_seed.sql              — INSERT INTO chains (chain_id, name, ...)
  4. factories_seed.sql           — INSERT INTO factories (only for seeded DEXes)
  5. tokens_reference.md          — top tokens per chain (WETH/WNative + USDC equivalent)

Direction: Excel → fragments. Applying fragments to .env Production / VPS / PG migrations
is a separate operator-gated step (RunFullSyncCycle + psql).

Privacy: multi-provider CSV + provider ORDER IS SHUFFLED per run, so the first
provider in the value does not fingerprint the operator's infra.

Read-only on the workbook. No secret is printed to stdout (only lengths).
"""
import argparse, json, random, sys
from collections import OrderedDict
from pathlib import Path

try:
    import openpyxl
except ImportError:
    sys.exit("openpyxl required: pip install openpyxl")

# CHAIN_IDS + CHAIN_META canonical (mirror of gen_rpc_env_from_xlsx.py + seed 043)
CHAIN_IDS = {
    'Ethereum Mainnet':1,'Optimism':10,'BSC Mainnet':56,'Gnosis':100,
    'Polygon Mainnet':137,'Base':8453,'Arbitrum One':42161,'Avalanche':43114,
    'Linea':59144,'Scroll':534352,'Blast':81457,
}
FACTORIES_SEEDED = {10,56,137,8453,42161}  # chains with factories in seed 043
# Per-chain factories from seed 043 (extend as dapp adds more)
FACTORIES = {
    1:  {'UniswapV2':'0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f','UniswapV3':'0x1F98431c8aD98523631AE4a59f267346ea31F984','SushiSwap':'0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac'},
    10: {'UniswapV3':'0x1F98431c8aD98523631AE4a59f267346ea31F984','SushiSwap':'0xc35DADB65012eC412f5fe79F3667b22B3A32B795'},
    56: {'PancakeV2':'0x1097053Fd5911a4863cA7D0e6F3C73a8B2CDA8b9','PancakeV3':'0x0BFbCF9fa4f9C56B0F40a671Ad90E3DC94D20d4e','BiSwap':'0x3a6d8cA21D1CF76F653A67577FA0FB271661792C'},
    137:{'UniswapV3':'0x1F98431c8aD98523631AE4a59f267346ea31F984','SushiSwap':'0xc35DADB65012eC412f5fe79F3667b22B3A32B795'},
    8453:{'UniswapV3':'0x33128a8fC17869897dcEA68d25cD9Ec44D11BbfA','Aerodrome':'0x33360F37492Ea44090b89FF2cFF92Bc399938E1'},
    42161:{'UniswapV3':'0x1F98431c8aD98523631AE4a59f267346ea31F984','SushiSwap':'0xc35DADB65012eC412f5fe79F3667b22B3A32B795'},
}
TOKENS_REF = {  # WNative + USDC-equivalent per chain (for tokens_reference.md)
    1:('WETH','0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2','USDC','0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48'),
    10:('WETH','0x4200000000000000000000000000000000000006','USDC','0x7F5c764cBc14f9669B88837ca1490cCa17c31607'),
    137:('WMATIC','0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270','USDC','0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174'),
    8453:('WETH','0x4200000000000000000000000000000000000006','USDC','0x833589fCD6eDb6E08f4c7C32D4f71b54bdA20D63'),
    42161:('WETH','0x82aF49447D8a07e3bd95BD0d56f35241523fBab1','USDC','0xaf88d065e77c8cC2239327C5EDb3A432268e5831'),
    56:('WBNB','0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c','BUSD-USD','0xe9e7CEA3DedC5394299B3f8f10F1Bb15Bb1b7b15'),
}

def load_catalog(wb):
    """Read RPC Providers + _RED_lookup into chain->{HTTP,WSS}->{provider:url}."""
    ws = wb['_RED_lookup']
    red_chain = {}
    for row in ws.iter_rows(min_row=2, values_only=True):
        if row[3] and row[4]: red_chain[str(row[3]).strip()] = str(row[4]).strip()
    ws = wb['RPC Providers']
    out = {}
    for row in ws.iter_rows(min_row=2, values_only=True):
        chain,proto,prov,url = (list(row)+[None]*4)[:4]
        if not all([chain,proto,prov,url]): continue
        out.setdefault(str(chain).strip(), {'HTTP':{},'WSS':{}})
        out[str(chain).strip()][str(proto).strip()][str(prov).strip()] = str(url).strip()
    return out

def shuffle_csv(providers: dict, seed=None):
    """Build multi-provider CSV with SHUFFLED order (privacy: no fixed first provider)."""
    items = list(providers.items())
    rng = random.Random(seed)
    rng.shuffle(items)
    return ','.join(f'{prov}={url}' for prov, url in items)

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--xlsx', default='C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm')
    ap.add_argument('--out', default='.', help='dir for fragments')
    ap.add_argument('--seed', type=int, default=None, help='shuffle seed (None=random per run)')
    ap.add_argument('--include-keyed', action='store_true', help='include key-requiring URLs (off by default)')
    args = ap.parse_args()

    wb = openpyxl.load_workbook(args.xlsx, data_only=True, read_only=True, keep_vba=True)
    if 'Chain Builder' not in wb.sheetnames:
        sys.exit("ERROR: 'Chain Builder' sheet not found")
    ws = wb['Chain Builder']
    active = []  # [(chain_name, chain_id)]
    for row in ws.iter_rows(min_row=4, values_only=True):
        chain, mark, cid = (list(row)+[None]*3)[:3]
        if str(mark or '').strip() == '✓' and chain and cid:
            active.append((str(chain).strip(), int(cid)))
    if not active:
        sys.exit("No chains marked ✓ ACTIVE in Chain Builder. Mark col B then re-run.")

    catalog = load_catalog(wb)
    out = Path(args.out)
    rpc_lines, enabled_ids, chains_sql, factories_sql, tokens_md = [], [], [], [], []
    enabled_ids = [str(cid) for _, cid in active]

    for chain, cid in active:
        rpcs = catalog.get(chain)
        if not rpcs:
            print(f'WARN: {chain} ({cid}) not in RPC Providers — skipped', file=sys.stderr); continue
        http, wss = rpcs.get('HTTP', {}), rpcs.get('WSS', {})
        if not http:
            print(f'WARN: {chain} ({cid}) has no HTTP RPCs — skipped', file=sys.stderr); continue
        rpc_lines.append(f'RPC_HTTP_{cid}={shuffle_csv(http, args.seed)}')
        if wss: rpc_lines.append(f'RPC_WS_{cid}={shuffle_csv(wss, args.seed)}')
        # chains.sql
        chains_sql.append(f"  ({cid}, '{chain.lower().replace(' mainnet','').replace(' one','').replace(' mainnet','')}', 'ETH', 'https://etherscan.io', true)")  # name/explorer simplified — adjust via CHAIN_META in production
        # factories.sql (only if seeded)
        if cid in FACTORIES and cid in FACTORIES_SEEDED:
            for dex_name, addr in FACTORIES[cid].items():
                factories_sql.append(f"  ('{dex_name}', {cid}, '{addr}')")
        # tokens ref
        if cid in TOKENS_REF:
            sym, addr, sym2, addr2 = TOKENS_REF[cid]
            tokens_md.append(f'| {cid} | {chain} | {sym} `{addr}` | {sym2} `{addr2}` |')

    (out/'rpc_chains_fragment.env').write_text('\n'.join(rpc_lines)+'\n')
    (out/'enabled_chains_fragment.env').write_text(f'ARBX_ENABLED_CHAINS={",".join(enabled_ids)}\n')
    (out/'chains_seed.sql').write_text('-- Generated by gen_chain_env.py — review before applying\n'
        'INSERT INTO chains (chain_id, name, native_currency, explorer_url, is_active) VALUES\n'
        + ',\n'.join(chains_sql) + '\nON CONFLICT (chain_id) DO NOTHING;\n')
    (out/'factories_seed.sql').write_text('-- Generated by gen_chain_env.py\n'
        'INSERT INTO factories (dex_id, chain_id, address) VALUES\n'
        + ',\n'.join(factories_sql) + '\nON CONFLICT (chain_id, address) DO NOTHING;\n')
    (out/'tokens_reference.md').write_text('# Tokens reference per active chain\n\n| chain_id | chain | WNative | USDC-equiv |\n|---|---|---|---|\n' + '\n'.join(tokens_md) + '\n')

    print(f'OK — {len(active)} chains active: {enabled_ids}')
    print(f'fragments in {out.resolve()}: rpc_chains_fragment.env, enabled_chains_fragment.env, chains_seed.sql, factories_seed.sql, tokens_reference.md')
    print('(review fragments, then apply via RunFullSyncCycle + psql — operator gated)')

if __name__ == '__main__':
    main()
```

- [ ] **Step 2: Mark a test chain (Base 8453) + run**

```bash
# Mark Base ACTIVE in Chain Builder (col B row of Base = "✓") via openpyxl
PYTHONIOENCODING=utf-8 python3 -c "
import openpyxl
wb = openpyxl.load_workbook('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm', keep_vba=True)
ws = wb['Chain Builder']
for r in range(4, ws.max_row+1):
    if ws.cell(r,1).value == 'Base':
        ws.cell(r,2,'✓'); print(f'marked Base active at r{r}'); break
wb.save('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm')
"
# Run the generator
python3 scripts/arbx-env-deploy/gen_chain_env.py --out /c/tmp/chain_out/
cat /c/tmp/chain_out/enabled_chains_fragment.env   # expect: ARBX_ENABLED_CHAINS=8453
head -2 /c/tmp/chain_out/rpc_chains_fragment.env    # expect: RPC_HTTP_8453=<shuffled providers>
cat /c/tmp/chain_out/factories_seed.sql | head -4   # expect: UniswapV3 + Aerodrome for 8453
```

- [ ] **Step 3: Verify shuffle privacy (run twice, confirm order changes)**

```bash
python3 scripts/arbx-env-deploy/gen_chain_env.py --out /c/tmp/run1/ >/dev/null
python3 scripts/arbx-env-deploy/gen_chain_env.py --out /c/tmp/run2/ >/dev/null
diff <(head -1 /c/tmp/run1/rpc_chains_fragment.env) <(head -1 /c/tmp/run2/rpc_chains_fragment.env) \
  && echo "❌ order did NOT change (privacy weak)" \
  || echo "✅ order shuffled (privacy OK)"
```

- [ ] **Step 4: Unmark Base (cleanup, leave the operator to decide)**

```bash
PYTHONIOENCODING=utf-8 python3 -c "
import openpyxl
wb = openpyxl.load_workbook('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm', keep_vba=True)
ws = wb['Chain Builder']
for r in range(4, ws.max_row+1):
    if ws.cell(r,1).value == 'Base': ws.cell(r,2,None); break
wb.save('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm')
print('Base unmarked — Chain Builder ready for operator use')
"
```

---

### Task 5: Final verification + documentation in the workbook

**Files:**
- Read: `C:\Users\HFRC\Downloads\ArbitrageX_Unified_Config.xlsm`
- Create: a "Leyenda" note row at top of Chain Builder

- [ ] **Step 1: Full workbook integrity check**

```bash
PYTHONIOENCODING=utf-8 python3 -c "
from oletools.olevba import VBA_Parser
vp = VBA_Parser('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm')
macros = list(vp.extract_macros())
print(f'macros: {len(macros)} (expect 16)')
import openpyxl
wb = openpyxl.load_workbook('C:/Users/HFRC/Downloads/ArbitrageX_Unified_Config.xlsm', data_only=True, read_only=True)
print(f'sheets: {len(wb.sheetnames)}')
# spot-check 5 pre-existing values still populated
env = wb['.env Production']
for k_row in [('SIM_BACKEND',63),('REVM_RPC_URL',64),('DATABASE_URL',15),('RPC_HTTP_1',46),('SIM_SIGNER_ADDRESS',61)]:
    v = env.cell(k_row[1],2).value
    print(f'  {k_row[0]}: {\"LLENO\" if v else \"VACIO\"}')
print('Chain Builder sheet:', 'Chain Builder' in wb.sheetnames)
print('Mainnet Deploy sheet:', 'Mainnet Deploy' in wb.sheetnames)
"
```

**Expected:** 16 macros, 15 sheets (13 original + Mainnet Deploy + Chain Builder), 5/5 spot-check LLENO, both new sheets present.

- [ ] **Step 2: Add a "Leyenda" row to Chain Builder explaining colors**

Already in the build script (instructions row). Confirm it reads:
"marca ✓ en col B → corre python scripts/arbx-env-deploy/gen_chain_env.py → revisa fragment → RunFullSyncCycle (sube al VPS)."

- [ ] **Step 3: Commit the new repo script + plan**

```bash
cd "c:/Users/HFRC/Desktop/arbitragex-v2-main (17)/arbitragex-v2-main"
git add scripts/arbx-env-deploy/gen_chain_env.py docs/superpowers/plans/2026-07-04-excel-chain-builder.md
git commit -m "feat(excel-bridge): gen_chain_env.py — Excel Chain Builder → 5 dapp artifacts

Adds the generator that reads the new 'Chain Builder' sheet (mark ✓ col B)
and emits: RPC_HTTP/WS multi-provider CSV (shuffled for privacy),
ARBX_ENABLED_CHAINS, chains_seed.sql, factories_seed.sql, tokens_reference.md.

Direction: Excel → fragments (read-only on workbook). Applying fragments
is operator-gated (RunFullSyncCycle + psql). paper_mode never touched.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review (writing-plans skill)

**Spec coverage:** the operator's request was (1) agregar/quitar chain arranca desde Excel ✓ (Task 3 Chain Builder + Task 4 generator), (2) privacidad multi-provider ✓ (Task 4 Step 3 shuffle), (3) 5 dapp layers ✓ (Task 4 emits all 5), (4) respeta lo existente ✓ (Task 1 corrects prior styling, Task 0 verifies VBA preserved), (5) excepcional extensión ✓ (Chain Builder is a control panel, not a data dump).

**Placeholder scan:** the only "TBD-like" is `CHAIN_META` name/explorer simplification in Task 4 Step 1 chains_sql line (noted "adjust via CHAIN_META in production"). The full CHAIN_META dict IS defined in Task 2 Step 1; the generator should be wired to it — **fix: replace the simplified line with `CHAIN_META[cid]` lookup** before finalizing.

**Type consistency:** `factories_seed.sql` uses `(dex_name, chain_id, address)` but seed 043 uses `(dex_id, chain_id, address)` where `dex_id` is a UUID FK to `dexes`. **fix: the generator must resolve dex_name → dex_id UUID via a `dexes` lookup table (or emit `dex_id` as `(SELECT id FROM dexes WHERE name=...)` subquery).** This is a real spec gap to close in Task 4 before Step 1 runs.

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-07-04-excel-chain-builder.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task (5 tasks), review between tasks. Best for catching the two spec gaps found in self-review before they propagate.

**2. Inline Execution** — I execute all tasks in this session now, fixing the gaps as I go.

**Note:** the two self-review gaps (`CHAIN_META` wiring + `factories_seed.sql` `dex_id` FK resolution) should be fixed in Task 4 Step 1 BEFORE writing the script. I'll patch the plan inline if you pick execution.

**Which approach?**
