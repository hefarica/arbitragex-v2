"""Build the unified matrix from declared DTO fields + cartridge requirements.

This is a source-level inventory. An identifier occurring in a file does NOT
prove its runtime value was delivered. Every row states that distinction.
"""
from __future__ import annotations
import argparse
import json
import re
from collections import defaultdict
from pathlib import Path
from .inventory import PIN, REPO_URL, dump_json, dump_csv, sha256

SOURCE_FILES = [
 ('backend/shared-rs/src/contracts.rs','core_wire'),
 ('backend/shared-rs/src/candidates.rs','route_wire'),
 ('backend/searcher-rs/src/route_intent.rs','intent'),
 ('backend/searcher-rs/src/reserves.rs','source_cache'),
 ('backend/searcher-rs/src/size_optimizer.rs','sizing'),
 ('backend/math-engine/src/operators/mod.rs','math'),
 ('backend/math-engine/src/api.rs','math_api'),
 ('frontend/lib/store/types.ts','frontend_model'),
 ('backend/api-server/src/routes/opportunities-live.ts','api_live'),
 ('backend/prioritization-spine/src/route_plan.rs','economic_route'),
 ('backend/prioritization-spine/src/types.rs','economic_spine'),
]
COMPONENTS = ['frontend/components/OpportunityTradeCard.tsx',
 'frontend/components/opportunities/OpportunitySummaryGrid.tsx',
 'frontend/components/opportunities/OpportunityDetailTabs.tsx',
 'frontend/lib/store/types.ts']


def declared_fields(text: str, language: str):
    """Top-level Rust pub structs / TS interfaces. Conservative lexical parser."""
    container=None
    for line_no, line in enumerate(text.splitlines(),1):
        start=re.match(r'^(?:pub(?:\([^)]*\))?\s+)?struct\s+(\w+)[^{]*\{',line) if language=='rust' else re.match(r'^(?:export\s+)?interface\s+(\w+)[^{]*\{',line)
        if start:
            container=start[1];continue
        if container and line.startswith('}'):
            container=None;continue
        if not container or line.strip().startswith(('/', '*', '#')):
            continue
        m=re.match(r'\s+pub\s+(\w+)\s*:\s*(.+)',line) if language=='rust' else re.match(r'^  (\w+)(\?)?\s*:\s*(.+)',line)
        if m:
            yield container,m[1],(m[2] if language=='rust' else m[3]).rstrip(',;'),line_no


def unit_for(field: str, type_name: str) -> str:
    f=field.lower()
    if 'sqrt' in f and '96' in f:return 'Q64.96 (price ratio, not USD)'
    if f.endswith('_usd') or f.startswith('target_net_usd'):return 'USD; exact decimal string for audit'
    if f.endswith('_wei') or f in {'leg_amounts_in','leg_amounts_out','amount_in_wei'}:return 'raw token integer; asset+decimals required'
    if 'fee_pips' in f:return 'pips / 1,000,000'
    if f=='fee_bps':return 'PROTOCOL-DEPENDENT LEGACY FIELD: verify bps vs V3 pips'
    if f.endswith('_bps'):return 'basis points / 10,000'
    if f.endswith('_pct'):return 'percent, not fraction; bps = percent * 100'
    if f.endswith('_ms'):return 'milliseconds; clock/epoch must be specified'
    if 'timestamp' in f or f.endswith('_at'):return 'epoch/UTC per source; no time substitution'
    if 'decimals' in f:return 'uint8 token exponent; absence is NOT 18'
    if 'price' in f:return 'asset_pair + direction + decimal scale required'
    if 'confidence' in f or 'probab' in f:return 'calibrated probability only with model/version'
    if 'liquidity' in f:return 'protocol-specific units; V3 L is not TVL'
    if 'hash' in f:return 'digest with algorithm/domain identified'
    if 'address' in f or f in {'token_in','token_out','pool'}:return '(chain_id,address) identity'
    if 'block' in f:return 'chain/block identity; validate canonical hash/reorg'
    if f in {'r0','r1','reserve0','reserve1','reserves_source'}:return 'raw token0/token1 units + snapshot'
    return 'explicit semantic contract required; not inferred from name alone'


def build_matrix(repo: Path, inventory: dict) -> dict:
    records=[]
    components={p:(repo/p).read_text() for p in COMPONENTS if (repo/p).exists()}
    for rel,domain in SOURCE_FILES:
        file=repo/rel
        if not file.is_file():continue
        txt=file.read_text()
        for typ,field,dtype,line in declared_fields(txt,'rust' if rel.endswith('.rs') else 'ts'):
            refs=[]
            for cp,ct in components.items():
                matches=[i for i,l in enumerate(ct.splitlines(),1) if re.search(r'\b'+re.escape(field)+r'\b',l) and not l.strip().startswith(('*','//'))]
                if matches:refs.append(f'{cp}:L{matches[0]}')
            is_nullable='Option<' in dtype or 'null' in dtype
            key=f'{typ}.{field}'
            records.append({'field_id':f'F{len(records)+1:04}', 'canonical_field':key,'domain':domain,
              'declared_type':dtype,'unit_contract':unit_for(field,typ),
              'origin_status':'DECLARED_IN_SOURCE_NOT_SOURCE_ATTESTED','origin_reference':f'{rel}:L{line}',
              'preprocessing':'Validate type, unit, asset, snapshot; no implicit fallback',
              'calculation':'Producer/transform receipt required for value-level proof',
              'postprocessing':'Preserve state+value; exact raw integer strings; record transform version',
              'persistence':'NOT_OBSERVED: require event_id/COMMIT receipt, not row-count proof',
              'delivery':'NOT_OBSERVED: require stage receipts + expected-ingress census',
              'frontend_references':refs,
              'input_value':None,'output_value':None,'value_observation_status':'NOT_OBSERVED_LIVE',
              'null_policy':'Null means missing/not computed; reason needed' if is_nullable else 'Required by declaration; prove actual producer value',
              'verification_status':'STATIC_DECLARATION_ONLY','required_proof':'Same event/context through producer, storage, API and intended consumer',
              'source_url':f'{REPO_URL}/blob/{PIN}/{rel}#L{line}','source_sha256':sha256(txt.encode())})
    # All static input requirements, including nested keys reached by local aliases.
    input_usage=defaultdict(list)
    for s in inventory['strategies']:
        txt=(repo/s['source']).read_text()
        # Do not turn comment-only example fields into proved input accesses.
        code=re.sub(r'(?m)^\s*//.*$','',txt)
        aliases=dict(re.findall(r'let\s+(\w+)\s*=\s*pool_data\.(\w+)\s*;',code))
        fields={f'pool_data.{x}' for x in s['input_fields']}
        for alias,parent in aliases.items():
            fields.update(f'pool_data.{parent}.{k}' for k in re.findall(r'\b'+re.escape(alias)+r'\.(\w+)',code) if k not in {'len','to_float','to_string','push','contains'})
        for field in fields:
            top=field.split('.')[1]
            input_usage[field].append({'strategy_id':s['strategy_id'],'source_url':s['source_url'],
                                      'builder_provides_top_level':top in inventory['builder_top_level_keys']})
    for field,uses in sorted(input_usage.items()):
        missing=not uses[0]['builder_provides_top_level']
        records.append({'field_id':f'F{len(records)+1:04}','canonical_field':field,'domain':'cartridge_input',
            'declared_type':'Rhai Dynamic; runtime schema required','unit_contract':unit_for(field.split('.')[-1],''),
            'origin_status':'NOT_SUPPLIED_BY_INTENT_BUILDER' if missing else 'TOP_LEVEL_BUILDER_KEY_PRESENT',
            'origin_reference':'backend/searcher-rs/src/cartridge_boot.rs::build_cartridge_pool_data',
            'preprocessing':'Must validate full external/pool state; existence of a key is not freshness',
            'calculation':'Required by '+','.join(u['strategy_id'] for u in uses),
            'postprocessing':'Explicit missing/unavailable outcome; do not emit fictitious zero profit',
            'persistence':'Need terminal outcome audit even when no opportunity is emitted',
            'delivery':'Intended destination is cartridge evaluator + audit trace; not necessarily card',
            'frontend_references':[], 'input_value':None,'output_value':None,
            'value_observation_status':'NOT_OBSERVED_LIVE','null_policy':'Absent field remains absent; no fabricated external feed',
            'verification_status':'CONTEXT_WIRING_GAP' if missing else 'STATIC_READ_ONLY',
            'required_proof':'Supplier adapter + event-bound snapshot + family replay',
            'source_url':uses[0]['source_url'],'source_sha256':'see strategy inventory', 'strategy_count':len(uses)})
    # User-map field names preserved separately instead of falsely aliasing them.
    for row in inventory['supplied_maps']['rows']:
        d=row['data'];field=d.get('Field_Name') or d.get('Output_Field')
        if not field:continue
        records.append({'field_id':f'F{len(records)+1:04}','canonical_field':'USER_MAP.'+str(field),
            'domain':'supplied_map_claim','declared_type':d.get('Data_Type','not specified'),
            'unit_contract':d.get('Unit','not specified'), 'origin_status':'USER_DESIGN_NOT_OBSERVATION',
            'origin_reference':f"{row['source_file']}:L{row['source_line']}",
            'preprocessing':d.get('Preprocessing_Step',d.get('Validation_Rule','not specified')),
            'calculation':d.get('Calculation_Logic',d.get('Calculation_Step','not specified')),
            'postprocessing':d.get('Output_Step','not specified'),
            'persistence':d.get('Guarantee_Method','CLAIM_ONLY'),
            'delivery':d.get('Destination_Service',d.get('Final_Destination','CLAIM_ONLY')),
            'frontend_references':[], 'input_value':None,'output_value':None,'value_observation_status':'NOT_OBSERVED_LIVE',
            'null_policy':d.get('Failover_Behavior','No zero fallback accepted as measured data'),
            'verification_status':'RECONCILIATION_REQUIRED','required_proof':'Reconcile names, dimensions, units and real source at each boundary',
            'source_url':row['source_file'], 'source_sha256':'see source_maps manifest'})
    return {'scope':'Declared DTO fields in listed files + explicit Rhai input accesses + supplied-map fields. Dynamic maps remain open; runtime payload census is required.',
            'source_files':[p for p,_ in SOURCE_FILES], 'records':records,'input_usage':input_usage}


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--repo',type=Path,required=True);p.add_argument('--inventory',type=Path,required=True);p.add_argument('--out',type=Path,required=True)
    a=p.parse_args(); repo=a.repo.resolve();out=a.out.resolve()
    if repo==out or repo in out.parents:p.error('output must be outside source tree')
    result=build_matrix(repo,json.loads(a.inventory.read_text()))
    dump_json(out/'field_matrix.json',result);dump_csv(out/'field_matrix.csv',result['records'])
    print(json.dumps({'matrix_rows':len(result['records']),'scope':result['scope']}))
if __name__=='__main__':main()
