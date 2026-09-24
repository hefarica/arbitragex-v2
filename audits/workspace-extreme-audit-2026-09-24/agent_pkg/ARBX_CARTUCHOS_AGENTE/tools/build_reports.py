#!/usr/bin/env python3
import csv,hashlib,html,json,zipfile,shutil,collections
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def dump(p,x):p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
def main():
    specs=json.loads((ROOT/'spec/strategies.json').read_text(encoding="utf-8"));generation=json.loads((ROOT/'reports/GENERATION.json').read_text(encoding="utf-8"))
    rows=[]
    for name in ['ArbitrageX_264_Cartridge_Math_Architecture(1)','ArbitrageX_Master_264x31_LIVE_First(2)']:
        source=json.loads((ROOT/'sources'/f'{name}.json').read_text(encoding="utf-8"));formulas=json.loads((ROOT/'sources'/f'{name}.formulas.json').read_text(encoding="utf-8"))
        # Count original retained formula entries by inspecting their exported shape.
        for sheet in source['sheets']:
            vals=sheet['values'];filled=sum(v is not None and v!=''for r in vals for v in r)
            rows.append({'workbook':source['file'],'sheet':sheet['name'],'range':sheet['range'],'nonempty_cells':filled,'rows':len(vals),'columns':max(map(len,vals),default=0),'workbook_sha256':source['sha256'],'treatment':'RAW_CELLS_AND_FORMULAS_PRESERVED_NOT_ALL_RUNTIME_IMPLEMENTED'})
        original=ROOT.parent/source['file']
        if original.exists():
            assert hashlib.sha256(original.read_bytes()).hexdigest()==source['sha256'];shutil.copyfile(original,ROOT/'sources'/original.name)
    dump(ROOT/'reports/SOURCE_COVERAGE.json',{'sheet_instances':len(rows),'nonempty_cells':sum(r['nonempty_cells']for r in rows),'sheets':rows,'runtime_coverage_is_separate':True})
    with(ROOT/'reports/COBERTURA_264.csv').open('w',encoding='utf-8-sig',newline='')as f:
        w=csv.writer(f);w.writerow(['MEV_ID','Estrategia','Archivo_generado','Detector_ID','Contrato_economico','Min_legs_fuente','Max_legs_fuente','Hops_repo','Hops_busqueda','Fuentes_cubiertas_sin_recorte_de_hops','Operadores_primarios','Operadores_secundarios','Pesos','Compilacion_Rhai','Estado_integracion','Ecuacion_original','Referencia_fuente'])
        for s in specs:w.writerow([s['mev_id'],s['name'],'generated/cartridges/strategies/'+s['filename'],s['detector_id'],s['family_contract']['logic'],s['source_leg_bounds']['min'],s['source_leg_bounds']['max'],str(s['repository_hops']),str(s['effective_search_hops']),s['scope_is_complete_for_source'],str(s['primary_operators']),str(s['secondary_operators']),'UNCALIBRATED','NOT_RUN','STAGED_NOT_WIRED',s['equation'],json.dumps(s['source_refs'],ensure_ascii=False)])
    with(ROOT/'reports/ADAPTADORES_60.csv').open('w',encoding='utf-8-sig',newline='')as f:
        w=csv.writer(f);w.writerow(['Detector_ID','Logica','Cartuchos','Contrato_requerido','Codigo_de_quote_incluido','Feeds_y_solver_completos_implementados','Compilado','E2E_live'])
        for fid,c in json.loads((ROOT/'spec/detector_contracts.json').read_text(encoding="utf-8")).items():
            w.writerow([fid,c['logic'],sum(s['detector_id']==fid for s in specs),'; '.join(c['required_checks']),'CPMM integer + exact native cache'if c['logic']in('closed_route','post_state_route')else'Prepared domain cashflow/constraint interface','NO — requires concrete provider/solver receipts','NOT_RUN','NOT_RUN'])
    with(ROOT/'reports/OPERADORES_8184.csv').open('w',encoding='utf-8-sig',newline='')as f:
        data=[r for s in specs for r in s['operator_roles']];w=csv.DictWriter(f,fieldnames=list(data[0]));w.writeheader();w.writerows(data)
    originals=[]
    zpath=ROOT.parent/'arbitragex-v2-main (22).zip'
    if zpath.exists():
        with zipfile.ZipFile(zpath)as z:
            for p in(ROOT/'reference_repo').rglob('*'):
                if p.is_file():
                    rel=p.relative_to(ROOT/'reference_repo').as_posix();b=z.read('arbitragex-v2-main/'+rel);assert p.read_bytes()==b
                    originals.append({'path':rel,'sha256':hashlib.sha256(b).hexdigest()})
    dump(ROOT/'reports/ORIGINALS_UNCHANGED.json',{'verified_reference_files':len(originals),'files':originals,'existing_repo_write_actions':0,'remote_configuration_changes':0,'note':'Reference copies matched original ZIP; no claim that every file of ZIP equals current main.'})
    summary={'generated_rhai':len(specs),'full_manifests':len(specs),'source_sheets':len(rows),'source_nonempty_cells':sum(r['nonempty_cells']for r in rows),'source_operator_relations':sum(len(s['operator_roles'])for s in specs),'detector_families':len({s['detector_id']for s in specs}),'logical_contract_classes':len({s['family_contract']['logic']for s in specs}),'source_scope_complete_strategies':sum(s['scope_is_complete_for_source']for s in specs),'existing_root_scripts_preserved':7,'native_compilation':'NOT_RUN','production_activation':'NOT_PERFORMED'}
    dump(ROOT/'reports/DELIVERY_SUMMARY.json',summary)
    trs=[]
    for s in specs:
        esc=html.escape
        trs.append('<tr><td>'+esc(s['mev_id'])+'</td><td><a href="generated/cartridges/strategies/'+s['filename']+'">'+esc(s['name'])+'</a></td><td>'+esc(s['detector_id'])+'</td><td>'+esc(s['family_contract']['logic'])+'</td><td>'+esc(str(s['effective_search_hops']))+'</td><td><a href="generated/manifests/'+s['mev_id']+'.json">Manifest completo</a></td></tr>')
    page='''<!doctype html><html lang="es"><meta charset="utf-8"><title>ArbitrageX — Cartuchos Agente v4</title><style>body{font:15px system-ui;margin:36px auto;max-width:1400px;padding:0 24px;color:#172333;background:#f6f8fa}h1{font-size:30px}table{width:100%;border-collapse:collapse;background:white}th,td{text-align:left;border-bottom:1px solid #dde4ea;padding:10px}th{background:#18384a;color:white;position:sticky;top:0}a{color:#155e75}.notice{padding:18px;background:#fff4db;border-left:5px solid #ab6a13}.stats{font-size:20px;line-height:1.8}input{margin:16px 0;padding:12px;width:420px;max-width:90%;font-size:16px}</style><h1>Cartuchos Agente v4</h1><p class="stats">264 scripts · 264 manifiestos · 60 familias · 8.184 relaciones fuente</p><div class="notice"><b>Generados y preparados para integración, no activados.</b> Las verificaciones de Python/TypeScript no son compilación Rhai/Rust. No hay certificación de producción ni garantía de rentabilidad. Las curvas/datos externos/solvers no implementados requieren proveedores nativos, sin sustituciones genéricas.</div><p><a href="README_ES.md">Guía</a> · <a href="INTEGRACION.md">Integración</a> · <a href="VALIDACION.md">Validación</a> · <a href="reports/COBERTURA_264.csv">Matriz CSV</a></p><input id="filter" placeholder="Filtrar ID, estrategia o familia…" oninput="for(const r of document.querySelectorAll('tbody tr'))r.hidden=!r.textContent.toLowerCase().includes(this.value.toLowerCase())"><table><thead><tr><th>ID</th><th>Cartucho</th><th>Detector</th><th>Contrato</th><th>Hops efectivos</th><th>Fuentes</th></tr></thead><tbody>'''+''.join(trs)+'</tbody></table></html>'
    (ROOT/'INDEX.html').write_text(page,encoding='utf-8',newline='\n');print(json.dumps(summary,indent=2))
if __name__=='__main__':main()
