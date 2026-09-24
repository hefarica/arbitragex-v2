"""Reproducible static census. Lexical findings are leads, not execution proofs."""
from __future__ import annotations
import argparse
import csv
import hashlib
import io
import json
import re
from collections import Counter
from pathlib import Path
from typing import Any

PIN = "538cd191b397749238947b1558e45a1d0746c382"
REPO_URL = "https://github.com/hefarica/arbitragex-v2"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def dump_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def dump_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if not rows:
        path.write_text("", encoding="utf-8"); return
    keys = list(dict.fromkeys(k for r in rows for k in r))
    with path.open("w", encoding="utf-8-sig", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=keys)
        writer.writeheader()
        for row in rows:
            def safe(v: Any) -> Any:
                if isinstance(v, (dict, list)):
                    v = json.dumps(v, ensure_ascii=False, separators=(",", ":"))
                if isinstance(v, str) and v.startswith(("=", "+", "-", "@")):
                    v = "'" + v
                return v
            writer.writerow({k: safe(v) for k, v in row.items()})


def location(path: str, text: str, needle: str, pin: str = PIN) -> str:
    index = text.find(needle)
    line = text[:index].count("\n") + 1 if index >= 0 else 1
    return f"{REPO_URL}/blob/{pin}/{path}#L{line}"


def string_value(text: str, key: str) -> str:
    m = re.search(r'\b' + re.escape(key) + r'\s*:\s*"([^"\n]*)"', text)
    return m[1] if m else ""


def array_value(text: str, key: str) -> list[Any]:
    m = re.search(r'\b' + re.escape(key) + r'\s*:\s*\[([^\]]*)\]', text)
    if not m:
        return []
    return [int(n) for n in re.findall(r'\b\d+\b', m[1])] if '"' not in m[1] else re.findall(r'"([^"]*)"', m[1])


def strip_comments(text: str) -> str:
    # Preserve line positions; this is a lexical census, not a compiler.
    text = re.sub(r'/\*.*?\*/', lambda m: "\n" * m[0].count("\n"), text, flags=re.S)
    return re.sub(r'(?m)^\s*//.*$', '', text)


def parse_maps(paths: list[Path]) -> dict[str, Any]:
    rows, problems, dependencies = [], [], []
    for path in paths:
        txt = path.read_text(encoding="utf-8-sig")
        section = ""
        in_csv = False
        csv_headers: list[str] = []
        table_headers: list[str] = []
        for lineno, line in enumerate(txt.splitlines(), 1):
            if line.startswith("## "):
                section = line.lstrip("# "); table_headers = []
            if line.strip().startswith("```csv"):
                in_csv = True; csv_headers = []; continue
            if line.strip().startswith("```") and in_csv:
                in_csv = False; continue
            if in_csv and line.strip():
                vals = next(csv.reader([line]))
                if not csv_headers:
                    csv_headers = vals; continue
                if len(vals) != len(csv_headers):
                    problems.append({"file": path.name, "line": lineno, "issue": "CSV_WIDTH_MISMATCH", "expected": len(csv_headers), "actual": len(vals), "raw": line})
                data = dict(zip(csv_headers, vals))
                rows.append({"source_file": path.name, "source_line": lineno, "source_section": section,
                             "source_status": "USER_MAP_CLAIM_NOT_OBSERVED", "data": data, "raw": line})
            elif line.startswith("|") and line.endswith("|"):
                vals = [x.strip() for x in re.split(r'(?<!\\)\|', line)[1:-1]]
                if vals and all(re.fullmatch(r'[:\- ]+', x or '-') for x in vals):
                    continue
                if not table_headers:
                    table_headers = vals; continue
                if len(vals) != len(table_headers):
                    problems.append({"file": path.name, "line": lineno, "issue": "TABLE_WIDTH_MISMATCH", "expected": len(table_headers), "actual": len(vals), "raw": line})
                data = dict(zip(table_headers, vals))
                rows.append({"source_file": path.name, "source_line": lineno, "source_section": section,
                             "source_status": "USER_MAP_CLAIM_NOT_OBSERVED", "data": data, "raw": line})
        known = set()
        for row in rows:
            if row["source_file"] != path.name:
                continue
            fid = row["data"].get("Field_ID", "")
            if re.fullmatch(r'[RPFDO]\d{3}', fid):
                known.add(fid)
        for row in rows:
            if row["source_file"] != path.name:
                continue
            for col in ("Input_Fields", "Dependencies", "Field_IDs", "Input_Evidence"):
                for fid in set(re.findall(r'\b[RPFDO]\d{3}\b', row["data"].get(col, ""))):
                    if fid not in known:
                        dependencies.append({"source_file": path.name, "line": row["source_line"], "owner": row["data"].get("Field_ID", row["data"].get("Component", "")), "dependency": fid, "status": "UNDEFINED_IN_SOURCE_MAP"})
    return {"rows": rows, "parse_problems": problems, "undefined_dependencies": dependencies}


def census(repo: Path, map_paths: list[Path], pin: str = PIN) -> dict[str, Any]:
    if not (repo / 'backend/math-engine/src/operators/mod.rs').is_file():
        raise ValueError("not an ArbitrageX source tree")
    map_data = parse_maps(map_paths)
    supplied = {r["data"]["Strategy_ID"]: r for r in map_data["rows"] if "Strategy_ID" in r["data"]}
    user_ops = {int(r["data"]["Operator_ID"].split('-')[1]): r for r in map_data["rows"] if re.fullmatch(r'OP-\d+', r["data"].get("Operator_ID", ""))}
    mod_path = 'backend/math-engine/src/operators/mod.rs'
    mod = (repo / mod_path).read_text()
    registry = re.findall(r'(\d+)\s*=>\s*Box::new\(crate::operators::(op_\w+)::(\w+)::new\(\)\)', mod)
    operators = []
    for oid_raw, module, ctor in registry:
        oid = int(oid_raw)
        path = f'backend/math-engine/src/operators/{module}.rs'
        if not (repo / path).is_file():
            path = f'backend/math-engine/src/operators/{module}/mod.rs'
        txt = (repo / path).read_text()
        pre_tests = txt.split('#[cfg(test)]')[0]
        body = strip_comments(pre_tests)
        name_m = re.search(r"fn name\(&self\) -> &'static str\s*\{\s*\"([^\"]+)\"", txt)
        cat_m = re.search(r"fn category\(&self\) -> &'static str\s*\{\s*\"([^\"]+)\"", txt)
        feature_keys = sorted(set(re.findall(r'(?:features\s*\.\s*get|feature_value)\(\s*"([^"]+)"', body)))
        defaults = [f"{i}:{l.strip()}" for i,l in enumerate(pre_tests.splitlines(),1) if 'unwrap_or(' in l and not l.strip().startswith('//')]
        user = user_ops.get(oid, {}).get('data', {})
        operators.append({"operator_id": oid, "registered_name": name_m[1] if name_m else ctor,
                          "module": module, "category": cat_m[1] if cat_m else "not_extracted",
                          "map_name_same_id": user.get('Operator_Name'), "feature_keys": feature_keys,
                          "reads_price_matrix": 'price_matrix' in body,
                          "reads_liquidity": 'liquidity_reserves' in body,
                          "possible_defaults": defaults, "unit_test_attributes": len(re.findall(r'#\[test\]', txt)),
                          "math_status": "UNTRAINED_POLICY_SOURCE" if oid == 31 else "SOURCE_PRESENT_NOT_NUMERICALLY_CERTIFIED",
                          "runtime_status": "NOT_OBSERVED", "source": path, "source_sha256": sha256(txt.encode()),
                          "source_url": location(path, txt, 'fn evaluate', pin)})
    ids = {x['operator_id'] for x in operators}
    host_path = 'backend/searcher-rs/src/cartridge/host_bindings.rs'
    host = (repo / host_path).read_text()
    host_bindings = set(re.findall(r'register_fn\(\s*"([^"]+)"', host))
    capfile = repo / 'skills/arbitragex-ultra/capability_matrix.json'
    capability = json.loads(capfile.read_text())
    strategies, edges, auxiliary = [], [], []
    for file in sorted((repo / 'backend/searcher-rs/cartridges').rglob('*.rhai')):
        txt = file.read_text()
        sid = string_value(txt, 'mev_id')
        rel = file.relative_to(repo).as_posix()
        if not re.fullmatch(r'MEV-\d{2}-\d{3}', sid):
            auxiliary.append({"file": rel, "name": string_value(txt, 'name'),
                              "status": "AUXILIARY_FILE_NOT_COUNTED_AS_NUMBERED_STRATEGY", "source_sha256": sha256(txt.encode()),
                              "source_url": location(rel, txt, 'fn init_strategy', pin)})
            continue
        canonical_path = repo / f'skills/arbitragex-ultra/strategies/{sid}/STRATEGY.json'
        canon = json.loads(canonical_path.read_text()) if canonical_path.exists() else {}
        prim = array_value(txt,'primary_operators'); sec = array_value(txt,'secondary_operators')
        def op_numbers(xs):
            return [int(re.search(r'\d+', str(x))[0]) for x in xs]
        expected_prim = op_numbers(canon.get('primary_operators',[]))
        expected_sec = op_numbers(canon.get('secondary_operators',[]))
        required = set(expected_prim + expected_sec)
        declared = set(prim + sec)
        bindings = array_value(txt,'required_bindings')
        fields = sorted(set(re.findall(r'pool_data\.([a-zA-Z_]\w*)', strip_comments(txt))))
        eval_body = txt.split('fn evaluate_opportunity',1)[-1].split('fn build_payload',1)[0]
        lexical = []
        patterns = {"FEE_DEFAULT_REVIEW": r'(?:fee_bps|pool_fee).*\b(?:30|0\.003)\b',
                    "CONFIDENCE_CONSTANT_REVIEW": r'(?:conf|confidence)\s*=\s*0\.[0-9]+',
                    "GAS_DEFAULT_REVIEW": r'(?:gas_cost|gas_usd).*\b(?:5\.0|10\.0|150000|2000\.0)\b',
                    "FLOAT_RAW_AMOUNT_REVIEW": r'(?:r0|r1|reserve|amount_in|amount_out).*to_float\(',
                    "ZERO_OUTPUT_OR_HINT_REVIEW": r'(?:profit_usd|estimated_profit)\s*[:=]\s*0\.0'}
        for kind, pat in patterns.items():
            for i, line in enumerate(txt.splitlines(),1):
                if re.search(pat,line) and not line.strip().startswith('//'):
                    lexical.append(f'{kind}@L{i}')
        user = supplied.get(sid,{}).get('data',{})
        row = {"strategy_id":sid,"repo_name":string_value(txt,'name'),"canonical_name":canon.get('estrategia'),
               "user_map_name":user.get('Strategy_Name'),"family":canon.get('familia'),
               "required_surface":canon.get('required_surface'),"backend_module":canon.get('backend_module'),
               "detector_id":string_value(txt,'detector_id'),"execution_class":string_value(txt,'execution_class'),
               "min_legs":canon.get('min_legs'),"max_legs":canon.get('max_legs'),
               "canonical_primary":expected_prim,"canonical_secondary":expected_sec,
               "rhai_primary":prim,"rhai_secondary":sec,
               "canonical_not_in_rhai":sorted(required-declared),"rhai_not_in_canonical":sorted(declared-required),
               "unregistered_operators":sorted((required|declared)-ids),
               "required_bindings":bindings,"bindings_not_found_in_host_file":sorted(set(bindings)-host_bindings),
               "input_fields":fields,"lexical_review_flags":lexical,
               "operators_link_status":"DECLARATION_DRIFT" if required != declared else "DECLARATIONS_MATCH_NOT_EXECUTION_PROOF",
               "runtime_status":"NOT_OBSERVED","numerical_validation":"PENDING_FAMILY_AND_STRATEGY_REPLAY",
               "source":rel,"source_sha256":sha256(txt.encode()),"source_url":location(rel,txt,'fn evaluate_opportunity',pin),
               "canonical_url":f'{REPO_URL}/blob/{pin}/skills/arbitragex-ultra/strategies/{sid}/STRATEGY.json',
               "evaluation_body_sha256":sha256(eval_body.encode()),
               "capability_declared_operators":op_numbers(capability.get(sid,{}).get('operators',[]))}
        strategies.append(row)
        for oid in sorted(required|declared):
            edges.append({"strategy_id":sid,"operator_id":oid,
                          "canonical_role":"primary" if oid in expected_prim else "secondary" if oid in expected_sec else "undeclared",
                          "rhai_role":"primary" if oid in prim else "secondary" if oid in sec else "undeclared",
                          "registered":oid in ids,"invocation_observed":False,"consumption_observed":False,
                          "status":"DECLARATION_DRIFT" if (oid in required) != (oid in declared) else "RUNTIME_UNVERIFIED",
                          "source_url":row['source_url']})
    wire_path = 'shared-ts/src/contracts/strategy-kinds.ts'
    wire_txt = (repo / wire_path).read_text()
    wire_match = re.search(r'STRATEGY_KINDS\s*=\s*\[(.*?)\] as const', wire_txt, re.S)
    wire_kinds = re.findall(r'"([^"\n]+)"', wire_match[1]) if wire_match else []
    stem_map = {Path(r['source']).stem: r for r in strategies}
    engine_paths = {'dex_arb':'dex_engine.rs','triangular':'triangular_engine.rs',
                    'backrun':'backrun_engine.rs','liquidation':'liquidation_engine.rs',
                    'flashloan_arb':'flashloan_engine.rs'}
    boot = (repo/'backend/searcher-rs/src/cartridge_boot.rs').read_text()
    builder = boot.split('pub fn build_cartridge_pool_data(',1)[1].split('\n    m\n}',1)[0]
    supplied_keys = set(re.findall(r'(?<!\w)m\.insert\(\s*"([^"\n]+)"', builder))
    supplied_keys.update({'fee_bps','fee_pips','fee_error'})
    for r in strategies:
        r['inputs_not_supplied_by_intent_builder'] = sorted(set(r['input_fields']) - supplied_keys)
        r['input_wiring_status'] = 'CONTEXT_PROVIDER_REQUIRED' if r['inputs_not_supplied_by_intent_builder'] else 'BUILDER_KEYS_FOUND_NOT_RUNTIME_PROOF'
    kinds = []
    for kind in wire_kinds:
        r = stem_map.get(kind)
        source = r['source'] if r else 'backend/searcher-rs/src/engines/' + engine_paths.get(kind,'UNRESOLVED')
        kinds.append({'strategy_kind':kind,'strategy_id':r['strategy_id'] if r else 'BASE:'+kind,
                      'kind_type':'NUMBERED_CARTRIDGE' if r else 'BASE_FAMILY',
                      'name':r['repo_name'] if r else kind,'detector_id':r['detector_id'] if r else 'native_engine',
                      'source':source,'source_present':(repo/source).is_file(),
                      'operators_link_status':r['operators_link_status'] if r else 'NATIVE_PIPELINE_REVIEW',
                      'input_wiring_status':r['input_wiring_status'] if r else 'NATIVE_PIPELINE_REVIEW',
                      'runtime_status':'NOT_OBSERVED','source_url':f'{REPO_URL}/blob/{pin}/{source}'})
    summary = {"source_commit":pin,"requested_count":269,"wire_strategy_kinds":len(kinds),"base_families":len(kinds)-len(strategies),"numbered_rhai_files":len(strategies),
               "unique_numbered_ids":len({r['strategy_id'] for r in strategies}),"auxiliary_files":len(auxiliary),
               "total_rhai_files":len(strategies)+len(auxiliary),"registered_operators":len(operators),
               "user_map_strategies":len(supplied),"user_map_operators":len(user_ops),
               "canonical_rhai_operator_drift_strategies":sum(bool(r['canonical_not_in_rhai'] or r['rhai_not_in_canonical']) for r in strategies),
               "applicability_edges":len(edges),"strategies_with_context_provider_gap":sum(bool(x["inputs_not_supplied_by_intent_builder"]) for x in strategies),"detector_family_count":len({r['detector_id'] for r in strategies}),
               "execution_classes":dict(Counter(r['execution_class'] for r in strategies)),
               "detector_families":dict(Counter(r['detector_id'] for r in strategies)),
               "undefined_map_dependencies":len(map_data['undefined_dependencies']),"map_parse_problems":len(map_data['parse_problems']),
               "live_status":"NOT_VERIFIED", "method":"static lexical census + source inspection; no Rhai execution"}
    return {"summary":summary,"wire_kinds":kinds,"builder_top_level_keys":sorted(supplied_keys),"strategies":strategies,"auxiliary":auxiliary,"operators":operators,"strategy_operator_edges":edges,
            "supplied_maps":map_data,"host_bindings":sorted(host_bindings)}


def export(report: dict[str,Any], out: Path) -> None:
    dump_json(out/'inventory.json',report)
    dump_json(out/'summary.json',report['summary'])
    for key in ('strategies','wire_kinds','auxiliary','operators','strategy_operator_edges'):
        dump_csv(out/(key+'.csv'),report[key])
    dump_csv(out/'map_undefined_dependencies.csv',report['supplied_maps']['undefined_dependencies'])
    dump_json(out/'source_maps_preserved.json',report['supplied_maps'])


def main() -> None:
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--repo',type=Path,required=True);p.add_argument('--out',type=Path,required=True)
    p.add_argument('--map',action='append',type=Path,default=[]);p.add_argument('--source-commit',default=PIN)
    args=p.parse_args()
    repo,out=args.repo.resolve(),args.out.resolve()
    if repo==out or repo in out.parents:
        raise SystemExit('Audit output must be OUTSIDE the source repository')
    report=census(repo,args.map,args.source_commit);export(report,out)
    print(json.dumps(report['summary'],ensure_ascii=False,indent=2))
if __name__=='__main__':
    main()
