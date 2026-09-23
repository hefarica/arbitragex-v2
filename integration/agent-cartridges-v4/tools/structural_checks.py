#!/usr/bin/env python3
"""Lexical checks + source conformance; NOT a Rhai compiler or economic audit."""
from pathlib import Path
import json,re,hashlib,sys
ROOT=Path(__file__).resolve().parents[1]

def strip_literals(text):
    out=list(text);i=0;state=None;start=0
    while i<len(text):
        if state=='line':
            if text[i]=='\n':state=None
            else:out[i]=' '
        elif state=='block':
            if text.startswith('*/',i):out[i]=out[i+1]=' ';i+=1;state=None
            elif text[i]!='\n':out[i]=' '
        elif state=='string':
            if text[i]=='\\':
                out[i]=' '
                if i+1<len(text):out[i+1]=' ';i+=1
            elif text[i]=='"':out[i]=' ';state=None
            elif text[i]!='\n':out[i]=' '
        else:
            if text.startswith('//',i):out[i]=out[i+1]=' ';i+=1;state='line'
            elif text.startswith('/*',i):out[i]=out[i+1]=' ';i+=1;state='block'
            elif text[i]=='"':out[i]=' ';state='string'
        i+=1
    if state in('string','block'):raise ValueError('unterminated_'+state)
    return ''.join(out)

def check_script(path,spec):
    original=path.read_text(encoding="utf-8");text=strip_literals(original);stack=[]
    for c in text:
        if c in '({[':stack.append(c)
        elif c in ')}]':
            if not stack or stack.pop()!={')':'(',']':'[','}':'{'}[c]:raise ValueError('unbalanced_delimiters')
    if stack:raise ValueError('unclosed_delimiters')
    funcs=[(m[0],len([a for a in m[1].split(',')if a.strip()]))for m in re.findall(r'\bfn\s+(\w+)\s*\(([^)]*)\)',text)]
    if len(funcs)!=len(set(funcs)):raise ValueError('duplicate_function_arity')
    for f in [('init_strategy',0),('evaluate_opportunity',1),('build_payload',1)]:
        if f not in funcs:raise ValueError('contract_arity_missing:'+str(f))
    if re.search(r'\b(?:null|undefined|IDENTITY)\b',text):raise ValueError('unsupported_template_symbol')
    if re.search(r'\bconst\b',text):raise ValueError('global_scope_dependency')
    if re.search(r'\b(?:import|eval|print)\b',text):raise ValueError('unexpected_external_surface')
    if not all(label in original for label in ['CAPA A','CAPA B','CAPA C','CAPA D']):raise ValueError('missing_layer')
    if f'"{spec["mev_id"]}"'not in original:raise ValueError('identity_missing')
    if '__METADATA__'in original or '__MANIFEST__'in original:raise ValueError('unexpanded_template')
    bound=set(re.findall(r'\b(agent_v4_\w+)\s*\(',text))
    bridge=(ROOT/'runtime/rhai_agent_bridge.rs').read_text(encoding="utf-8")
    for b in bound:
        if f'register_fn("{b}"'not in bridge:raise ValueError('native_binding_not_shipped:'+b)
    manifest=json.loads((ROOT/f'generated/manifests/{spec["mev_id"]}.json').read_text(encoding="utf-8"))
    if manifest!=spec:raise ValueError('manifest_differs_from_specification')
    if len(spec['operator_roles'])!=31:raise ValueError('incomplete_operator_matrix')
    for row in spec['operator_roles']:
        if row['Resolved_Weight']is not None:raise ValueError('unexpected_calibrated_weight')
    digest=hashlib.sha256(json.dumps(spec,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()
    if digest not in original:raise ValueError('manifest_binding_missing')
    return {'mev_id':spec['mev_id'],'functions':len(funcs),'native_bindings':len(bound),'status':'STRUCTURAL_PASS_NOT_COMPILED'}

def main():
    specs=json.loads((ROOT/'spec/strategies.json').read_text(encoding="utf-8"));rows=[];errors=[]
    for s in specs:
        try:rows.append(check_script(ROOT/'generated/cartridges/strategies'/s['filename'],s))
        except Exception as e:errors.append({'mev_id':s['mev_id'],'error':str(e)})
    report={'checks':len(specs),'pass':len(rows),'fail':len(errors),'scope':'lexical delimiters/contract arities/binding presence/source manifests; NOT Rhai compile or execution','rows':rows,'errors':errors}
    (ROOT/'validation/structural.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8',newline='\n')
    print(json.dumps({k:v for k,v in report.items()if k not in('rows','errors')},indent=2));return bool(errors)
if __name__=='__main__':sys.exit(main())
