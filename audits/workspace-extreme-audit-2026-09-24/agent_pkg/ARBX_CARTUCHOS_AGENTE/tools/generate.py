#!/usr/bin/env python3
"""Deterministic source-driven generator. Output is STAGED, never auto-activated."""
import json, hashlib, re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def literal(x):
    if x is None:return '()'
    if x is True:return 'true'
    if x is False:return 'false'
    if isinstance(x,str):return json.dumps(x,ensure_ascii=False)
    if isinstance(x,(int,float)):return str(x)
    if isinstance(x,list):return '['+', '.join(literal(v) for v in x)+']'
    if isinstance(x,dict):return '#{\n'+',\n'.join('        '+json.dumps(k)+': '+literal(v) for k,v in x.items())+'\n    }'
    raise TypeError(type(x))
def main():
    specs=json.loads((ROOT/'spec/strategies.json').read_text(encoding="utf-8"))
    template=(ROOT/'runtime/agent_template.rhai.in').read_text(encoding="utf-8")
    results=[]
    for s in specs:
        # No per-strategy weights or lower-mode mathematical shortcuts.
        meta={
          'name':s['name'],'version':'4.0.0','author':'ArbitrageX — workbook-derived generator',
          'description':s['mission'],'category':s['category'],'mev_id':s['mev_id'],
          'contract_version':'arbx.cartridge.agent/4','detector_id':s['detector_id'],
          'execution_class':s['execution_class'],'target_chains':[],
          'primary_operators':s['primary_operators'],'secondary_operators':s['secondary_operators'],
          'min_legs':s['source_leg_bounds']['min'],'max_legs':s['source_leg_bounds']['max'],
          'allowed_search_hops':s['effective_search_hops'],
          'requested_discovery_cap':7,'requires_native_agent_v4':True,
          'trigger_description':s['trigger_description'],
          'required_bindings':['agent_v4_discover','agent_v4_quote','agent_v4_operators','agent_v4_economic_check','agent_v4_seal','agent_v4_money_compare','agent_v4_build_payload'],
          'config_schema':{
             'enabled':{'type':'bool','source':'existing strategy config','default':None},
             'min_profit_usd':{'type':'decimal_string','source':'existing strategy config','default':None},
             'capital_cap_usd':{'type':'decimal_string','source':'canonical capital provider','default':None},
             'operator_switches':{'type':'map<bool>','allowed_ids':s['primary_operators']+s['secondary_operators'],'default':None},
             'source_specific_parameters':{'type':'read_only_contract_text','value':s['proposed_config_text']},
          },
          'source_references':s['source_refs'],'full_manifest_path':f"generated/manifests/{s['mev_id']}.json"
        }
        # Lean executable manifest; every additional original cell remains in the
        # adjacent full manifest and lossless workbook export, not silently lost.
        manifest={k:s[k] for k in ['contract','mev_id','name','category','detector_id','execution_class','equation','specific_note','source_leg_bounds','effective_search_hops','repository_hop_mask','weights_calibrated','modes','conflicts']}
        manifest['logic']=s['family_contract']['logic']
        manifest['allowed_search_hops']=s['effective_search_hops']
        manifest['required_data_text']=s['required_data_text']
        manifest['source_atomicity']=s['source_atomicity']
        manifest['original_live_gate']=s['original_live_gate']
        manifest['source_digest']=hashlib.sha256(json.dumps(s,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()
        manifest['operator_requirements']=[{'id':r['Operator_ID'],'role':r['Audited_Role'],'phase':r['Audited_Phase'],'semantic_role':r['Audited_Semantic_Role'],'requirement':r['Requirement'],'weight':None} for r in s['operator_roles']]
        script=template.replace('__METADATA__',literal(meta)).replace('__MANIFEST__',literal(manifest)).replace('__REQUIREMENTS__',literal(s['family_contract']['required_checks']))
        assert '__METADATA__' not in script and '__REQUIREMENTS__' not in script
        p=ROOT/'generated/cartridges/strategies'/s['filename'];p.write_text(script,encoding='utf-8',newline='\n')
        results.append({'mev_id':s['mev_id'],'path':str(p.relative_to(ROOT)),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'manifest_digest':manifest['source_digest'],'bytes':p.stat().st_size,'source_sha256':s['original_sha256']})
    (ROOT/'reports/GENERATION.json').write_text(json.dumps({'generated':len(results),'artifacts':results},indent=2)+'\n',encoding='utf-8',newline='\n')
    print('Generated:',len(results),'Rhai cartridges with individual manifests.')
if __name__=='__main__':main()
