#!/usr/bin/env python3
"""Executable independent reference model, NOT the deployed Rhai/Rust runtime.
All fixtures must be labelled TEST_ONLY. This module has no network or signer.
Quotes, costs, policies and constraint receipts are inputs, never fabricated.
"""
from __future__ import annotations
from decimal import Decimal, localcontext
import hashlib, json, re, copy, math
from typing import Any
U256_MAX=2**256-1
U112_MAX=2**112-1

def raw(s: str) -> int:
    if not isinstance(s,str) or not re.fullmatch(r'0|[1-9][0-9]*',s): raise ValueError('invalid_base_unit_integer')
    n=int(s)
    if n>U256_MAX: raise ValueError('u256_overflow')
    return n

def money(s: str) -> Decimal:
    if not isinstance(s,str) or len(s)>512 or not re.fullmatch(r'-?[0-9]+(?:\.[0-9]+)?',s):
        raise ValueError('invalid_usd_decimal')
    return Decimal(s)

def text(n: Decimal) -> str:
    if not n.is_finite(): raise ValueError('non_finite_money')
    s=format(n,'f')
    if '.' in s:s=s.rstrip('0').rstrip('.')
    return '0' if Decimal(s)==0 else s

def digest(v: Any) -> str:
    return hashlib.sha256(json.dumps(v,ensure_ascii=False,sort_keys=True,separators=(',',':'),allow_nan=False).encode()).hexdigest()

def cpmm(amount: str, rin: str, rout: str, fee: int, denominator: int) -> str:
    if type(fee)is not int or type(denominator)is not int or not 0<=fee<denominator<=2**32-1: raise ValueError('invalid_fee_fraction')
    x,a,b=raw(amount),raw(rin),raw(rout)
    if not 0<a<=U112_MAX or not 0<b<=U112_MAX: raise ValueError('invalid_v2_reserve_uint112')
    dx=x*(denominator-fee)
    return str(b*dx//(a*denominator+dx))

def value_usd(amount: str, decimals: int, price: str) -> str:
    if type(decimals)is not int or not 0<=decimals<=255:raise ValueError('invalid_decimals')
    x,p=raw(amount),money(price)
    if p<=0:raise ValueError('nonpositive_canonical_price')
    with localcontext() as c:
        c.prec=2048
        return text(Decimal(x).scaleb(-decimals)*p)

def cycles(edges: list[dict], start: str, max_hops: int, max_expansions: int, max_paths: int) -> dict:
    if type(max_hops)is not int or not 2<=max_hops<=7 or max_expansions<=0 or max_paths<=0:raise ValueError('invalid_search_limits')
    if not start:raise ValueError('missing_start_token')
    adjacency={};seen=set()
    for e in edges:
        if not e['edge_id'] or e['edge_id'] in seen or not e['pool_id'] or e['token_in']==e['token_out']:raise ValueError('invalid_or_duplicate_graph_edge')
        seen.add(e['edge_id']);adjacency.setdefault(e['token_in'],[]).append(e)
    for es in adjacency.values():es.sort(key=lambda e:e['edge_id'])
    report={'paths':[],'expansions':0,'truncated':False,'stopping_reason':None}
    def visit(token,path,tokens,pools):
        if report['truncated'] or len(path)>=max_hops:return
        for e in adjacency.get(token,[]):
            if report['expansions']>=max_expansions:
                report.update(truncated=True,stopping_reason='expansion_budget_exhausted');return
            report['expansions']+=1
            pool=(e['chain_id'],e['pool_id']);closes=e['token_out']==start
            if pool in pools or (closes and len(path)+1<2) or (not closes and e['token_out'] in tokens):continue
            p=path+[e['edge_id']]
            if closes:
                if len(report['paths'])>=max_paths:
                    report.update(truncated=True,stopping_reason='path_budget_exhausted');return
                report['paths'].append(p)
            else:visit(e['token_out'],p,tokens|{e['token_out']},pools|{pool})
            if report['truncated']:return
    visit(start,[],{start},set())
    return report

def request_key(e,amount):
    return digest({k:e[k] for k in ['edge_id','snapshot_id','block_hash','token_in','token_out','adapter_version']}|{'amount_in_raw':amount})

def quote_path(edges: list[dict], amount: str, exact: dict|None=None) -> list[dict]:
    if not edges:raise ValueError('empty_path')
    current=str(raw(amount));ledger=[];exact={} if exact is None else exact;pools=set()
    for i,e in enumerate(edges):
        if any(e[k]!=edges[0][k] for k in ['snapshot_id','block_hash','chain_id']):raise ValueError('mixed_block_or_domain_in_atomic_route')
        pool=(e['chain_id'],e['pool_id'])
        if pool in pools:raise ValueError('repeated_pool_requires_stateful_adapter')
        pools.add(pool)
        if i and edges[i-1]['token_out']!=e['token_in']:raise ValueError('broken_token_path')
        key=request_key(e,current)
        if e['protocol']=='cpmm_v2':
            for k in ['reserve_in_raw','reserve_out_raw','fee_units','fee_denominator']:
                if e.get(k)is None:raise ValueError('missing_'+k)
            amount_out=cpmm(current,e['reserve_in_raw'],e['reserve_out_raw'],e['fee_units'],e['fee_denominator'])
            qid=digest({'request':key,'reserve_in':e['reserve_in_raw'],'reserve_out':e['reserve_out_raw'],'fee':e['fee_units'],'denominator':e['fee_denominator'],'out':amount_out})
            method='cpmm_exact_integer'
            ref_num=int(current)*(e['fee_denominator']-e['fee_units'])*int(e['reserve_out_raw'])
            ref_den=int(e['reserve_in_raw'])*e['fee_denominator']
            metrics={'lp_fee_input_raw':{'status':'COMPUTED','numerator':str(int(current)*e['fee_units']),'denominator':str(e['fee_denominator']),'treatment':'embedded'},'price_impact_fraction':({'status':'COMPUTED','numerator':str(ref_num-int(amount_out)*ref_den),'denominator':str(ref_num),'basis':'fee_adjusted_marginal_output_including_integer_rounding'}if ref_num else{'status':'NOT_APPLICABLE','reason':'zero_input_has_no_relative_impact'})}
        else:
            if key not in exact:raise ValueError('exact_protocol_quote_required_no_cpmm_fallback')
            q=exact[key]
            if any(q[k]!=e[k] for k in ['edge_id','snapshot_id','block_hash','token_in','token_out','adapter_version']) or q['amount_in_raw']!=current:raise ValueError('quote_context_mismatch')
            if q['precision']!='protocol_exact_integer' or q['fees_and_impact_embedded']is not True or not q['quote_id']:raise ValueError('quote_is_bound_or_missing_provenance')
            amount_out=str(raw(q['amount_out_raw']));qid=q['quote_id'];method=q['precision'];metrics=q.get('metrics')
        ledger.append({k:e[k] for k in ['edge_id','chain_id','protocol','token_in','token_out','token_in_decimals','token_out_decimals','snapshot_id','block_hash','adapter_version']}|{'pool':e['pool_id'],'index':i,'amount_in_raw':current,'amount_out_raw':amount_out,'quote_id':qid,'quote_method':method,'fees_and_impact_embedded':True,'metrics':metrics})
        current=amount_out
    return ledger

def numeric(v):
    if type(v)in(int,float):return math.isfinite(v)
    return isinstance(v,list) and len(v)>0 and all(numeric(x)for x in v)

def _economic(spec,ctx,candidate,q,policy,evidence,receipts,requirements):
    repairs=[]
    def err(f,r):repairs.append({'field':f,'reason':r})
    for k,expected in [('context_id',ctx.get('context_id')),('snapshot_id',policy.get('snapshot_id')),('price_revision',policy.get('price_revision')),('policy_revision',policy.get('policy_revision')),('plan_id',candidate.get('plan_id')),('plan_hash',candidate.get('plan_hash'))]:
        if not q.get(k)or q[k]!=expected:err(k,'context_revision_mismatch')
    if policy.get('execution_mode')not in ['LIVE_MAINNET','TESTNET','PAPER_SHADOW']:err('execution_mode','unknown_mode')
    try:raw(q.get('amount_in_raw'))
    except (ValueError,TypeError):err('amount_in_raw','noncanonical_integer')
    if q.get('status')!='COMPUTED':err('quote','incomplete_quote')
    if q.get('profit_basis')not in ['before_financing','retained_after_repayment']:err('profit_basis','unknown_profit_basis')
    if q.get('economic_kind')not in ['atomic_quote','execution_improvement','expected_carry','guaranteed_payoff','non_atomic_expected','settlement_quote']:err('economic_kind','unknown_economic_kind')
    kind={'closed_route':'atomic_quote','carry_projection':'expected_carry','cross_domain':'non_atomic_expected'}.get(spec['logic'])
    if kind and q.get('economic_kind')!=kind:err('economic_kind','strategy_kind_mismatch')
    for m in q.get('missing',[]):err(m,'applicable_input_missing')
    legs=q.get('legs',[])
    if spec['logic']in ['closed_route','post_state_route']:
        if not legs:err('legs','missing_leg_ledger')
        for i,l in enumerate(legs):
            for k in ['token_in','token_out','amount_in_raw','amount_out_raw','quote_id','snapshot_id']:
                if not isinstance(l.get(k),str)or not l[k]:err(f'legs.{i}.{k}','missing_leg_field')
            if l.get('snapshot_id')!=q.get('snapshot_id'):err(f'legs.{i}','mixed_snapshot')
            if i==0 and l.get('amount_in_raw')!=q.get('amount_in_raw'):err('legs.0.amount_in_raw','amount_mismatch')
            if i and (legs[i-1].get('token_out')!=l.get('token_in')or legs[i-1].get('amount_out_raw')!=l.get('amount_in_raw')):err(f'legs.{i}','broken_token_or_amount_continuity')
        if legs and spec['logic']=='closed_route'and legs[0]['token_in']!=legs[-1]['token_out']:err('legs','not_closed')
    if spec['logic']in ['closed_route','post_state_route']:
        if len(q.get('incoming',[]))!=1 or len(q.get('outgoing',[]))!=1:err('cashflows','sequential_route_requires_bound_principal_and_output')
        elif legs:
            for flow,leg,side in [(q['outgoing'][0],legs[0],'in'),(q['incoming'][0],legs[-1],'out')]:
                v=flow.get('valuation')
                if not v:err('cashflows.'+flow['name'],'missing_token_valuation_provenance');continue
                try:
                    valid=leg['token_'+side]==v['token_address']and leg['amount_'+side+'_raw']==v['amount_raw']and leg['token_'+side+'_decimals']==v['token_decimals']and leg['chain_id']==v['chain_id']and v['price_revision']==q['price_revision']and bool(v.get('price_evidence_id'))and money(value_usd(v['amount_raw'],v['token_decimals'],v['price_usd']))==money(flow['usd'])
                    if not valid:err('cashflows.'+flow['name'],'valuation_not_bound_to_ledger')
                except(ValueError,KeyError,TypeError):err('cashflows.'+flow['name'],'invalid_valuation')
            a,b=q['outgoing'][0].get('valuation'),q['incoming'][0].get('valuation')
            if a and b and a.get('chain_id')==b.get('chain_id')and a.get('token_address')==b.get('token_address'):
                try:
                    if a['token_decimals']!=b['token_decimals']or money(a['price_usd'])!=money(b['price_usd']):err('cashflows','same_asset_has_conflicting_price_or_decimals')
                except(ValueError,KeyError):err('cashflows','invalid_asset_price')
            if q.get('capital_usd')!=q['outgoing'][0]['usd']:err('capital_usd','capital_differs_from_ledger_input')
    def sum_flows(flows):
        if not flows:raise ValueError('missing_flow')
        total=Decimal(0)
        for f in flows:
            if not f.get('evidence_id'):raise ValueError('missing_flow_evidence')
            v=money(f['usd'])
            if v<0:raise ValueError('negative_flow')
            total+=v
        return total
    try:gross=sum_flows(q['incoming'])-sum_flows(q['outgoing'])
    except (KeyError,ValueError):gross=None;err('cashflows','invalid_or_unproven_flow')
    external=Decimal(0);gas=Decimal(0);seen=set();complete=True
    if not q.get('required_cost_kinds'):complete=False;err('required_cost_kinds','empty_cost_contract')
    for l in q.get('costs',[]):
        k=l['kind'];t=l['treatment']
        if k in seen:complete=False;err('costs','duplicate_cost_kind')
        seen.add(k)
        if not l.get('evidence_id'):complete=False;err('costs.'+k,'missing_cost_evidence')
        if q.get('profit_basis')=='retained_after_repayment'and k=='financing'and t=='external':complete=False;err('costs.financing','already_in_retained_spread')
        if q.get('economic_kind')=='atomic_quote'and k=='execution_fees'and t=='external':complete=False;err('costs.execution_fees','quote_already_includes_swap_fees')
        if t in ['external','embedded']:
            try:
                v=money(l.get('usd'))
                if v<0:raise ValueError('negative_cost')
                if t=='external':
                    external+=v
                    if k=='gas':gas=v
            except (ValueError,TypeError):complete=False;err('costs.'+k,'missing_or_invalid_cost')
        elif t=='not_applicable':
            if l.get('usd')is not None or not l.get('reason'):complete=False;err('costs','invalid_not_applicable')
        else:complete=False;err('costs','unknown_cost_treatment')
    required_costs=set(q.get('required_cost_kinds',[]))
    if q.get('economic_kind')=='atomic_quote':required_costs|={'gas','financing','execution_fees'}
    for k in sorted(required_costs-seen):complete=False;err('costs.'+k,'required_cost_missing')
    net=gross-external if gross is not None and complete else None
    policy_ok=policy.get('enabled')is True and policy.get('control_state')=='ON'
    try:
        a,b=money(q.get('capital_usd')),money(policy.get('capital_cap_usd'))
        if not 0<a<=b:raise ValueError('cap')
    except (ValueError,TypeError):policy_ok=False;err('capital_usd','capital_missing_or_cap_exceeded')
    for name in ['min_profit_usd','max_gas_usd']:
        if policy.get(name)is not None:
            try:
                lim=money(policy[name])
                if name=='min_profit_usd'and (net is None or net<=lim):policy_ok=False
                if name=='max_gas_usd'and gas>lim:policy_ok=False
            except ValueError:err(name,'invalid_usd_decimal')
    for name in requirements:
        rs=[r for r in receipts if r['name']==name]
        if len(rs)!=1:err(name,'missing_or_duplicate_constraint_receipt');continue
        r=rs[0]
        if r['status']!='PASS'or not r.get('evidence_id')or r['snapshot_id']!=q['snapshot_id']or r['plan_hash']!=q['plan_hash']:err(name,r.get('reason')or'constraint_not_verified')
    for r in spec['operator_requirements']:
        if r['role']=='N/A':continue
        e=evidence.get('operators',{}).get(str(r['id']),{});st=e.get('status','MISSING')
        computed=st=='COMPUTED'and numeric(e.get('value'))and bool(e.get('evidence_id'))
        equivalent=st=='EQUIVALENT'and e.get('equivalent_for')==r['id']and bool(e.get('evidence_id'))and bool(e.get('method'))
        disabled=st=='DISABLED'and r['requirement']=='OPTIONAL_SUPPORT'and e.get('reason')=='operator_switch_off'
        if not(computed or equivalent or disabled):err('operators.'+str(r['id']),st)
    if evidence.get('snapshot_id')!=q['snapshot_id']or evidence.get('plan_hash')!=q['plan_hash']:err('operators','operator_context_mismatch')
    if 'NON_CYCLIC_NAME_VS_CLOSED_CYCLE_SPECIFIC_NOTE'in spec.get('conflicts',[]):err('strategy_identity','source_conflict_requires_resolution')
    observe=spec['logic']=='observe_only';comparison=q['economic_kind']=='execution_improvement'
    eligible=not observe and not comparison and not repairs and policy_ok and net is not None and net>0
    status='CANDIDATE'if eligible else'OBSERVE_ONLY'if observe else'DATA_GAP'if repairs else'IMPROVEMENT_ONLY'if comparison else'REJECTED'
    return {'status':status,'candidate_eligible':eligible,'approved_for_execution':False,'gross_profit_usd':None if gross is None else text(gross),'net_profit_usd':None if net is None else text(net),'external_cost_usd':text(external)if complete else None,'legs':copy.deepcopy(legs),'costs':copy.deepcopy(q['costs']),'operators':copy.deepcopy(evidence),'repairs':repairs,'economic_kind':q['economic_kind'],'execution_mode':policy['execution_mode'],'simulation':{'status':'NOT_RUN_BY_CARTRIDGE','passed':None},'validation_environment':'REFERENCE_MODEL_NOT_RHAI'}

def economic(*args,**kwargs):
    with localcontext()as ctx:
        ctx.prec=2048
        return _economic(*args,**kwargs)

def select(results: list[dict]) -> dict|None:
    eligible=[r for r in results if r['candidate_eligible']]
    computed=[r for r in results if r.get('net_profit_usd')is not None]
    pool=eligible or computed
    return copy.deepcopy(max(pool,key=lambda r:money(r['net_profit_usd'])))if pool else copy.deepcopy(results[0])if results else None

def quote_progress(edges, amount, exact=None):
    ledger=[]
    if not edges:return {'legs':[],'complete':False,'failed_hop':None,'reason':'empty_path'}
    for i in range(len(edges)):
        try:ledger=quote_path(edges[:i+1],amount,exact)
        except ValueError as e:return {'legs':ledger,'complete':False,'failed_hop':i,'reason':str(e)}
    return {'legs':ledger,'complete':True,'failed_hop':None,'reason':None}

if __name__=='__main__':
    import argparse,pathlib
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('request',type=pathlib.Path);ap.add_argument('--out',required=True,type=pathlib.Path);a=ap.parse_args()
    request=json.loads(a.request.read_text(encoding="utf-8"));result=economic(**request)
    a.out.write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n',encoding='utf-8',newline='\n');print(a.out)
