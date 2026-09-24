#!/usr/bin/env python3
"""Build source-authoritative cartridge specifications from the preserved XLSX exports.
The XLSX imports are made with artifact_tool; this step consumes their lossless
JSON values. It never edits the input workbooks or a repository configuration.
"""
import json, re, hashlib, collections
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
def load(p): return json.loads(p.read_text(encoding='utf-8'))
def dump(p,v):
    p.parent.mkdir(parents=True,exist_ok=True)
    p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')

def build():
    # Never silently regenerate from stale exports after an XLSX is edited.
    for export in (ROOT/'sources').glob('*.json'):
        if export.name.endswith('.formulas.json'): continue
        record=load(export)
        if not isinstance(record,dict) or not isinstance(record.get('file'),str): continue
        original=ROOT/'sources'/record['file']
        if original.exists() and hashlib.sha256(original.read_bytes()).hexdigest()!=record['sha256']:
            raise ValueError('source_xlsx_changed_reimport_required:'+record['file'])
    source = load(ROOT/'spec/source_tables.json')
    catalog={x['MEV_ID']:x for x in source['catalog']}
    maths={x['MEV_ID']:x for x in source['strategies']}
    families={x['Detector ID']:x for x in source['families']}
    byid=collections.defaultdict(list)
    for row in source['master']: byid[row['MEV_ID']].append(row)
    expected=set(maths)
    assert len(expected)==264 and expected==set(catalog)==set(byid)
    existing=ROOT/'reference_repo/backend/searcher-rs/cartridges/strategies'
    paths={}
    for p in existing.glob('*.rhai'):
        m=re.match(r'mev_(\d{2})_(\d{3})_',p.name)
        if m:
            key=f'MEV-{m[1]}-{m[2]}'
            assert key not in paths, f'duplicate {key}'
            paths[key]=p
    assert set(paths)==expected
    policy=load(ROOT/'reference_repo/docs/quotebase_strategy_hop_map.json')
    policyd={r['MEV_ID']:r for r in policy}
    print('Policy keys:', list(policy[0]))
    readme_source={
      'architecture':'ArbitrageX_264_Cartridge_Math_Architecture(1).xlsx',
      'master':'ArbitrageX_Master_264x31_LIVE_First(2).xlsx',
      'repo':'hefarica/arbitragex-v2',
      'checked_ref':'2245eeb325ab7a3d0e40f09cfeca357369975d1a',
      'source_archive':'arbitragex-v2-main (22).zip',
      'archive_is_not_asserted_equal_to_all_of_main':True,
    }
    contracts=family_contracts(families)
    conflicts=[]
    specs=[]
    for n,key in enumerate(sorted(expected),2):
        m,c=maths[key],catalog[key]
        roles=sorted(byid[key],key=lambda r:r['Operator_ID'])
        assert len(roles)==31 and [r['Operator_ID'] for r in roles]==list(range(1,32))
        assert all(r['Resolved_Weight'] is None for r in roles)
        assert all(r['LIVE_MAINNET_Role']==r['TESTNET_Role']==r['PAPER_SHADOW_Role']==r['Audited_Role'] for r in roles)
        def ops(role): return [r['Operator_ID'] for r in roles if r['Audited_Role']==role]
        # Keep each policy cell intact. This specification does not rewrite Rust tables.
        p=policyd[key]
        mask=p.get('HopMask_u8')
        if mask is None: mask=p.get('HopMask')
        if mask is not None:
            mask=int(mask)
            allowed=[h for h in range(2,8) if mask&(1<<(h-2))]
        else:
            # Read the exact committed Rust table, not a guessed fallback.
            rust=(ROOT/'reference_repo/backend/searcher-rs/src/strategy_hop_mask.rs').read_text(encoding="utf-8")
            match=re.search(r'\("'+re.escape(key)+r'",\s*(\d+)\)',rust)
            if not match: raise ValueError('missing repository hop policy '+key)
            mask=int(match[1]); allowed=[h for h in range(2,8) if mask&(1<<(h-2))]
        source_min,source_max=int(m['Legs mínimos']),int(m['Legs máximos'])
        effective=[h for h in allowed if source_min<=h<=min(source_max,7)]
        row_conflicts=[]
        if source_max>7 or source_min<2:
            row_conflicts.append('SOURCE_LEGS_VS_REQUESTED_SWAP_SEARCH_SCOPE')
        if key=='MEV-01-016':
            row_conflicts.append('TRIANGULAR_FIXED_3_VS_PSEUDOCODE_3_TO_7')
        if key=='MEV-01-022':
            row_conflicts.append('NON_CYCLIC_NAME_VS_CLOSED_CYCLE_SPECIFIC_NOTE')
        for code in row_conflicts:
            conflicts.append({'id':code,'strategy':key,'source_min_legs':source_min,'source_max_legs':source_max,
                              'repository_hops':allowed,'selected_scope':effective,
                              'source_ref':f"{readme_source['master']}!02_CARTRIDGE_MATH_MAP!A{n}:AB{n}",
                              'resolution':'Preserve source and repository policy. Search only their intersection with requested 2..7 swap hops; do not call that full source coverage.',
                              'status':'DISCLOSED_SCOPE_LIMIT' if code!='NON_CYCLIC_NAME_VS_CLOSED_CYCLE_SPECIFIC_NOTE' else 'REQUIRES_SOURCE_RESOLUTION'})
        b=paths[key].read_bytes()
        spec={
          'contract':'arbx.cartridge.agent/4', 'mev_id':key,'filename':paths[key].name,
          'name':m['Estrategia'],'mission':c['Descripción operativa'],'category':m['Módulo backend'],
          'family':m['Familia Excel'],'detector_id':m['Detector ID'],'execution_class':m['Clase de ejecución'],
          'classification':m['Clasificación determinista'],'equation':m['Ecuación / criterio de descubrimiento'],
          'specific_note':m['Nota específica de estrategia'],'original_live_gate':m['Gate LIVE original'],
          'source_leg_bounds':{'min':source_min,'max':source_max,'model':m['Modelo de legs']},
          'repository_hop_mask':mask,'repository_hops':allowed,'requested_search_hops':[2,3,4,5,6,7],
          'effective_search_hops':effective,
          'scope_is_complete_for_source':source_min>=2 and source_max<=7 and set(range(source_min,source_max+1))<=set(allowed),
          'primary_operators':ops('PRIMARY'),'secondary_operators':ops('SECONDARY'),
          'operator_roles':roles, 'operator_registry_32':'PRESERVED_NOT_ASSIGNED_BY_31_COLUMN_SOURCE',
          'weights_calibrated':False,
          'modes':['LIVE_MAINNET','TESTNET','PAPER_SHADOW'], 'economic_logic_mode_invariant':True,
          'trigger_description':c['Trigger/order position'],
          'source_dependencies':{'oracle':c['Dependencia oracle'],'bridge':c['Dependencia bridge'],'external':c['Dependencia CEX/externa'],'inventory':c['Inventario requerido']},
          'required_data_text':m['Datos / bindings requeridos'],
          'proposed_config_text':m['Configuración frontend propuesta'],
          'source_atomicity':{'possible':c['Atómico posible'],'kind':c['Tipo de atomicidad'],'non_atomic':c['Tipo no atómico']},
          'source_risks':c['Riesgos dominantes'],'source_toxicity':c['Toxicidad'],
          'source_state':m['Estado de diseño'],'source_implemented':c['Implementado'],
          'original_blob_sha1':hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest(),
          'original_sha256':hashlib.sha256(b).hexdigest(),
          'source_refs':{
             'catalog':f"{readme_source['master']}!01_MEV_MATRIX_1_11!A{n}:AA{n}",
             'math':f"{readme_source['master']}!02_CARTRIDGE_MATH_MAP!A{n}:AB{n}",
             'master':f"{readme_source['master']}!07_MASTER_264x31!A{2+(n-2)*31}:AJ{32+(n-2)*31}",
          },
          'math_sources':[m[k] for k in ['Fuente matemática 1','Fuente matemática 2','Fuente matemática 3'] if m[k]],
          'family_contract':contracts[m['Detector ID']],
          'conflicts':row_conflicts,
          'source_catalog_row':c,'source_math_row':m,'source_repo_policy_row':p,
        }
        specs.append(spec)
        dump(ROOT/'generated/manifests'/f'{key}.json',spec)
    # These are documentary corrections, not silent remapping or changed configuration.
    conflicts += [
       {'id':'COUNT_270_VS_264_PLUS_7','resolution':'Generate exactly 264 numbered replacements. Preserve all seven root scripts; never delete one to force 270.','status':'RESOLVED_GENERATION_SCOPE'},
       {'id':'OPS_31_VS_REGISTRY_32','resolution':'Preserve 31 source columns and original numeric IDs. Do not disable, relabel, or assign a made-up role/weight to NSGA-II operator 32.','status':'EXTENSION_NOT_SPECIFIED'},
       {'id':'RISK_WEIGHT_VS_QUALITY_GATE','resolution':'Do not map quality >=70 to probability <0.10. Native risk gate must report metric name, scale, direction and evidence.','status':'REQUIRES_RUNTIME_GATE_BINDING'},
       {'id':'PSEUDOCODE_DOUBLE_COST','resolution':'Quote outputs already include curve impact/LP fees. Subtract only documented external costs once, in a common numeraire.','status':'IMPLEMENTED_ACCOUNTING_RULE'},
       {'id':'MASTER_NO_CALIBRATED_WEIGHTS','resolution':'All 8184 Resolved_Weight cells remain null; no flat 0.5 confidence or normalized fake evidence.','status':'PRESERVED'},
       {'id':'SOURCE_STATIC_AUDIT_IS_NOT_LIVE_EVIDENCE','resolution':'06_B_STATIC_DIAG and 08_CONFLICTS are supplied findings, not current production measurements. Preserve their wording and source dates.','status':'PRESERVED_NOT_REATTESTED'},
       {'id':'SOURCE_CONFLICTS_ROW_ALIGNMENT','resolution':'Rows C-004/C-005/C-006 contain shifted short rows under ten headers. Preserve exact cells; do not infer displaced Status values.','status':'PRESERVED_RAW'},
    ]
    dump(ROOT/'spec/strategies.json',specs)
    dump(ROOT/'spec/detector_contracts.json',contracts)
    dump(ROOT/'reports/CONFLICTS.json',conflicts)
    dump(ROOT/'reports/SOURCE_SCOPE.json',readme_source|{'numbered_generated':len(specs),'root_preserved':7,'operator_rows':len(source['master']),
          'families':len(families),'unresolved_weight_cells':sum(x['Resolved_Weight'] is None for x in source['master']),
          'source_role_counts':dict(collections.Counter(r['Audited_Role'] for r in source['master'])),
          'family_logic_counts':dict(collections.Counter(s['family_contract']['logic'] for s in specs))})
    return specs

def family_contracts(families):
    # A quote adapter computes protocol-specific state transitions; these clauses
    # validate that the candidate is economically the strategy the workbook names.
    # They are NOT substitutes for actual external feeds, curve implementations,
    # complete payoff proofs or authorized execution/settlement.
    groups={
      'closed_route':['R_CLOSED_CYCLE','CF_CPMM','CF_CLAMM','CF_CROSSINV','CF_CONSTANT_SUM','CF_BOND','CF_STABLESWAP','CF_WEIGHTED','CF_LB','CF_PMM','CF_DYNAMIC'],
      'post_state_route':['E_POST','E_STATE','E_ORACLE','E_LATENCY','CF_TWAMM'],
      'path_comparison':['R_DIRECT_INDIRECT'],
      'split_allocation':['R_SPLIT'],
      'firm_pair':['R_ORDERBOOK','C_CEXDEX','M_AMM','M_CROSS'],
      'carry_projection':['C_CEXDERIV','D_BASIS','D_FUNDING','D_SETTLE','L_RATE','L_LOOP','CF_VAMM','P_YIELD'],
      'payoff_portfolio':['D_OPTIONS_PARITY','D_OPTIONS_SURFACE','M_LOGIC','M_COMPLETE'],
      'auction':['E_AUCTION','I_DUTCH','L_AUCTION'],
      'constrained_settlement':['CF_BATCH','R_COW','I_BATCH','I_ROUTE','I_ORDERFLOW'],
      'liquidation':['L_LIQ','N_LIQ'],
      'redemption':['L_COLLATERAL','P_4626','P_LST','P_NAV','P_PEG','P_PTYT','P_WRAP','R_BASKET_NAV','N_REDEEM'],
      'nft_firm_exit':['N_AMM','N_FLOOR','N_IDENTICAL'],
      'cross_domain':['X_BRIDGE','X_ORACLE','X_PREPOS'],
      'observe_only':['OBSERVE'],
    }
    by={f:g for g,fs in groups.items() for f in fs}
    assert set(by)==set(families), (set(families)-set(by),set(by)-set(families))
    base={
      'closed_route':['closed_token_cycle','protocol_exact_quotes','same_snapshot','token_continuity'],
      'post_state_route':['confirmed_transition','post_state_bound','protocol_exact_quotes','settlement_executable'],
      'path_comparison':['identical_input','identical_output_asset','same_snapshot','firm_baseline','settlement_executable'],
      'split_allocation':['input_allocation_conserved','nonnegative_allocations','shared_pool_state_consistent','firm_unsplit_baseline','settlement_executable'],
      'firm_pair':['same_economic_asset','matched_quantity','firm_depth','inventory_available','settlement_executable'],
      'carry_projection':['hedged_position','matched_horizon','margin_sufficient','funding_schedule','finite_horizon','settlement_executable'],
      'payoff_portfolio':['same_underlying','payoff_model_complete','firm_all_legs','quantity_conservation','settlement_executable'],
      'auction':['auction_rules_valid','lot_available','deadline_valid','firm_unwind','settlement_executable'],
      'constrained_settlement':['order_limits_respected','balance_conservation','approvals_valid','orders_not_expired','settlement_executable'],
      'liquidation':['protocol_liquidatable','close_factor_respected','collateral_available','oracle_fresh','firm_unwind','settlement_executable'],
      'redemption':['conversion_contract_valid','redemption_within_limits','component_quotes_firm','delay_costed','settlement_executable'],
      'nft_firm_exit':['asset_identity_match','firm_exit_bid','ownership_and_approval','royalties_accounted','settlement_executable'],
      'cross_domain':['same_economic_asset','all_domain_snapshots','inventory_or_bridge_capacity','finality_model','failure_states_costed','settlement_executable'],
      'observe_only':[],
    }
    extra={
      'CF_CLAMM':['full_tick_traversal_or_protocol_quoter'], 'CF_STABLESWAP':['exact_invariant_version_and_rates'],
      'CF_WEIGHTED':['normalized_weights_and_scaling'], 'CF_LB':['all_crossed_bins_and_variable_fees'],
      'CF_PMM':['oracle_round_and_inventory_branch'], 'CF_DYNAMIC':['dynamic_parameters_and_hook_state'],
      'CF_TWAMM':['virtual_orders_advanced_to_snapshot'], 'CF_BOND':['exact_integrated_cost_curve'],
      'CF_CONSTANT_SUM':['reserve_boundary_respected'], 'E_LATENCY':['independent_timestamps','execution_before_quote_expiry'],
      'E_ORACLE':['oracle_round_verified'], 'I_ORDERFLOW':['orderflow_explicitly_authorized'],
      'I_DUTCH':['auction_price_at_evaluation_time'], 'D_OPTIONS_PARITY':['same_strike_expiry_multiplier','financing_and_dividends'],
      'D_OPTIONS_SURFACE':['strike_calendar_constraints','portfolio_is_executable'],
      'M_COMPLETE':['exhaustive_mutually_exclusive_partition','actual_collateral_payout','split_merge_available'],
      'M_LOGIC':['payoff_states_exhaustive','worst_case_nonnegative'], 'M_CROSS':['same_event_and_resolution_rules'],
      'M_AMM':['identical_payout_claim'], 'N_FLOOR':['exit_is_not_floor_proxy'], 'N_AMM':['nft_curve_version_verified'],
      'P_4626':['preview_and_max_limits'], 'P_PTYT':['same_underlying_maturity','split_merge_available'],
      'P_WRAP':['representation_allowlisted'], 'P_LST':['withdrawal_queue_measured'],
      'X_BRIDGE':['bridge_quote_valid','non_atomic_settlement'], 'X_ORACLE':['cross_domain_oracle_timestamps','non_atomic_settlement'],
      'X_PREPOS':['prepositioned_inventory_all_domains','non_atomic_settlement'],
      'L_LOOP':['health_buffer','rate_feedback_model'], 'L_RATE':['borrow_supply_indices'],
      'L_LIQ':['health_metric_protocol_specific'], 'N_LIQ':['nft_firm_bid_not_appraisal'],
    }
    result={}
    for id,f in families.items():
        result[id]={'logic':by[id],'required_checks':base[by[id]]+extra.get(id,[])+([] if by[id]=='observe_only' else ['strategy_specific_note_verified','native_risk_and_impact_policy']),
                    'original_family_row':f,'equation_source':'03_DETECTOR_FAMILIES',
                    'quote_semantics':'Protocol adapter computes fee/impact-embedded outputs; no generic CPMM substitution.',
                    'native_solver_required':by[id] in ('split_allocation','payoff_portfolio','constrained_settlement'),
                    'guarantee':'Best among supplied/explored candidates, not proof of global optimum.',
                    'implementation_boundary':'Cartridge validates/ranks quotes and settlements; native adapters supply exact protocol transitions and domain constraint certificates.'}
    return result

if __name__=='__main__':
    specs=build()
    print('Built',len(specs),'specifications; 60 family contracts; source data retained.')
