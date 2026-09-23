"""Synthetic mathematical/contract vectors. Never reported as live observations."""
import copy, json, sys, unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'runtime'))
import reference_engine as r

def edge(i,a,b,ro='1000000000000',**patch):
    return {'edge_id':f'e{i}','pool_id':f'p{i}','chain_id':1,'token_in':a,'token_out':b,'protocol':'cpmm_v2','snapshot_id':'TEST_SNAPSHOT','block_hash':'TEST_BLOCK','reserve_in_raw':'1000000000000','reserve_out_raw':ro,'fee_units':30,'fee_denominator':10000,'token_in_decimals':6,'token_out_decimals':6,'adapter_version':'test-v2-1'}|patch

def vector(out='101',gas='0.1'):
    # Quote flow values below are synthetic independent inputs. The separate
    # integer-vector tests verify native-curve arithmetic against known values.
    spec={'mev_id':'TEST_STRATEGY','logic':'closed_route','conflicts':[],
       'operator_requirements':[{'id':15,'role':'PRIMARY','requirement':'PRIMARY_REQUIRED_OR_EQUIVALENT'},{'id':1,'role':'SECONDARY','requirement':'OPTIONAL_SUPPORT'}]}
    ctx={'context_id':'TEST_CONTEXT'};candidate={'plan_id':'TEST_PLAN','plan_hash':'TEST_HASH'}
    costs=[{'kind':'gas','usd':gas,'treatment':'external','reason':None,'evidence_id':'TEST_GAS'},
        {'kind':'financing','usd':None,'treatment':'not_applicable','reason':'test own inventory','evidence_id':'TEST_FINANCE'},
        {'kind':'execution_fees','usd':'0.6','treatment':'embedded','reason':None,'evidence_id':'TEST_QUOTES'}]
    legs=r.quote_path([edge(1,'TEST_A','TEST_B'),edge(2,'TEST_B','TEST_A')],'100000000')
    q={'status':'COMPUTED','context_id':'TEST_CONTEXT','plan_id':'TEST_PLAN','plan_hash':'TEST_HASH','snapshot_id':'TEST_SNAPSHOT','price_revision':'TEST_PRICE_REV','policy_revision':'TEST_POLICY_REV','amount_in_raw':'100000000','capital_usd':'100','profit_basis':'before_financing','economic_kind':'atomic_quote','incoming':[{'name':'route_out','usd':out,'evidence_id':'TEST_FLOW_OUT'}],'outgoing':[{'name':'principal','usd':'100','evidence_id':'TEST_FLOW_IN'}],'costs':costs,'required_cost_kinds':['gas','financing','execution_fees'],'legs':legs,'missing':[],'evidence':{'test_only':True}}
    # Explicit synthetic quote-result vectors for CONTRACT tests, not pool-
    # simulation outputs. Exact pool arithmetic has its own independent tests.
    q['legs'][-1]['amount_out_raw']=str(int(r.money(out)*1000000))
    for leg in q['legs']:leg['quote_method']='TEST_ONLY_QUOTE_RESULT_FIXTURE'
    for flows,leg,side in [(q['outgoing'],q['legs'][0],'in'),(q['incoming'],q['legs'][-1],'out')]:
        flows[0]['valuation']={'chain_id':1,'token_address':leg['token_'+side],'amount_raw':leg['amount_'+side+'_raw'],'token_decimals':6,'price_usd':'1','price_revision':'TEST_PRICE_REV','price_evidence_id':'TEST_ONLY_CANONICAL_PRICE'}
    policy={'enabled':True,'capital_cap_usd':'1000','min_profit_usd':None,'max_gas_usd':None,'snapshot_id':'TEST_SNAPSHOT','price_revision':'TEST_PRICE_REV','policy_revision':'TEST_POLICY_REV','execution_mode':'LIVE_MAINNET','control_state':'ON'}
    evidence={'snapshot_id':'TEST_SNAPSHOT','plan_hash':'TEST_HASH','operators':{'15':{'status':'EQUIVALENT','equivalent_for':15,'evidence_id':'TEST_SIZE','method':'synthetic supplied native size comparison'},'1':{'status':'DISABLED','reason':'operator_switch_off'}}}
    receipts=[{'name':'test_constraint','status':'PASS','reason':None,'evidence_id':'TEST_CONSTRAINT','snapshot_id':'TEST_SNAPSHOT','plan_hash':'TEST_HASH'}]
    return {'spec':spec,'ctx':ctx,'candidate':candidate,'q':q,'policy':policy,'evidence':evidence,'receipts':receipts,'requirements':['test_constraint']}

class Arithmetic(unittest.TestCase):
    def test_parallel_counterexample(self):
        one=r.cpmm('100000000','1000000000000','1000000000000',30,10000)
        two=r.cpmm('100000000','1000000000000','1001000000000',30,10000)
        self.assertEqual((one,two),('99690060','99789750'))
        self.assertGreater(int(two)-int(one),0)
        a=r.cpmm(one,'1001000000000','1000000000000',30,10000)
        b=r.cpmm(two,'1000000000000','1000000000000',30,10000)
        self.assertEqual((a,b),('99281840','99480483'))
        self.assertLess(int(a),100000000);self.assertLess(int(b),100000000)
    def test_precision_above_2_pow_53(self):self.assertEqual(r.value_usd('1000000000000000001',18,'2000.00000001'),'2000.00000001000000200000000001')
    def test_zero_output_is_real(self):self.assertEqual(r.cpmm('1','1000000','1',30,10000),'0')
    def test_no_stable_assumption(self):self.assertEqual(r.value_usd('100000000',6,'0.98'),'98')
    def test_max_u256(self):self.assertEqual(r.raw(str(2**256-1)),2**256-1)
    def test_overflow(self):self.assertRaises(ValueError,r.raw,str(2**256))
    def test_fee_units_pips(self):self.assertEqual(r.cpmm('10000','1000000','1000000',3000,1000000),r.cpmm('10000','1000000','1000000',30,10000))
    def test_exact_decimal_subtraction(self):self.assertEqual(r.text(r.money('99.48')-r.money('100')),'-0.52')
    def test_no_round_up_in_amount(self):self.assertEqual(r.cpmm('7','19','23',0,10000),'6')
    def test_zero_fee_is_valid_observed(self):self.assertEqual(r.cpmm('1','10','100',0,10000),'9')

for i,v in enumerate(['','-1','01','+1','1.0',' 1','NaN','1e3']):
    setattr(Arithmetic,f'test_invalid_integer_{i}',lambda self,v=v:self.assertRaises(ValueError,r.raw,v))
for i,v in enumerate(['','NaN','Infinity','1e3',' 1','+1','1..2','.5','1.']):
    setattr(Arithmetic,f'test_invalid_money_{i}',lambda self,v=v:self.assertRaises(ValueError,r.money,v))
for d in [0,6,8,18,36,255]:
    setattr(Arithmetic,f'test_decimals_{d}',lambda self,d=d:self.assertEqual(r.value_usd(str(10**d if d<=77 else 1),d,'2'), '2'if d<=77 else '0.'+'0'*254+'2'))
for i,args in enumerate([('1','0','1',30,10000),('1',str(2**112),'10',30,10000),('1','10','10',10000,10000),('1','10','10',0,0),('1','10','10',-1,10000)]):
    setattr(Arithmetic,f'test_invalid_curve_{i}',lambda self,args=args:self.assertRaises(ValueError,r.cpmm,*args))

class GraphAndQuotes(unittest.TestCase):
    def test_both_directions(self):
        es=[edge(1,'A','B'),edge(2,'B','C'),edge(3,'C','A'),edge(4,'A','C'),edge(5,'C','B'),edge(6,'B','A')]
        rep=r.cycles(es,'A',3,100,100);self.assertIn(['e1','e2','e3'],rep['paths']);self.assertIn(['e4','e5','e6'],rep['paths'])
    def test_no_repeated_pool(self):self.assertEqual(r.cycles([edge(1,'A','B'),edge(2,'B','A',pool_id='p1')],'A',2,100,100)['paths'],[])
    def test_budget_visible(self):self.assertTrue(r.cycles([edge(1,'A','B'),edge(2,'B','A')],'A',2,1,100)['truncated'])
    def test_duplicate_edge_rejected(self):self.assertRaises(ValueError,r.cycles,[edge(1,'A','B'),edge(1,'B','A')],'A',2,100,100)
    def test_no_phantom_reverse(self):self.assertEqual(r.cycles([edge(1,'A','B')],'A',2,100,100)['paths'],[])
    def test_path_boundary(self):self.assertRaises(ValueError,r.cycles,[],'A',8,100,100)
    def test_partial_ledger_preserved(self):
        progress=r.quote_progress([edge(1,'A','B'),edge(2,'B','A',protocol='clamm_v3')],'100')
        self.assertFalse(progress['complete']);self.assertEqual(len(progress['legs']),1);self.assertEqual(progress['failed_hop'],1)
    def test_no_cpmm_fallback(self):self.assertRaises(ValueError,r.quote_path,[edge(1,'A','B',protocol='clamm_v3')],'100')
    def test_stale_cache_key(self):
        e=edge(1,'A','B',protocol='clamm_v3');q={k:e[k]for k in ['edge_id','snapshot_id','block_hash','token_in','token_out','adapter_version']}|{'amount_in_raw':'100','amount_out_raw':'99','quote_id':'TEST_QUOTER','precision':'protocol_exact_integer','fees_and_impact_embedded':True}
        cache={r.request_key(e,'100'):q}
        self.assertEqual(r.quote_path([e],'100',cache)[0]['amount_out_raw'],'99')
        self.assertRaises(ValueError,r.quote_path,[e],'101',cache)
        cache[r.request_key(e,'100')]['precision']='single_tick_upper_bound'
        self.assertRaises(ValueError,r.quote_path,[e],'100',cache)
    def test_repeated_pool_quote(self):self.assertRaises(ValueError,r.quote_path,[edge(1,'A','B'),edge(2,'B','A',pool_id='p1')],'100')
    def test_freshness_not_relabelled(self):self.assertRaises(ValueError,r.quote_path,[edge(1,'A','B'),edge(2,'B','A',snapshot_id='OTHER')],'100')
    def test_chain_mismatch(self):self.assertRaises(ValueError,r.quote_path,[edge(1,'A','B'),edge(2,'B','A',chain_id=2)],'100')
    def test_block_mismatch(self):self.assertRaises(ValueError,r.quote_path,[edge(1,'A','B'),edge(2,'B','A',block_hash='OTHER')],'100')
    def test_token_mismatch(self):self.assertRaises(ValueError,r.quote_path,[edge(1,'A','B'),edge(2,'C','A')],'100')
    def test_missing_fee(self):self.assertRaises(ValueError,r.quote_path,[edge(1,'A','B',fee_units=None)],'100')

for n in range(2,8):
    def length_test(self,n=n):
        toks=[f'T{i}'for i in range(n)];es=[edge(i,toks[i],toks[(i+1)%n],ro='1100000000000'if i==n-1 else'1000000000000')for i in range(n)]
        paths=r.cycles(es,toks[0],n,1000,100)['paths'];self.assertEqual(paths,[[f'e{i}'for i in range(n)]])
        ledger=r.quote_path(es,'100000000');self.assertEqual(len(ledger),n);self.assertGreater(int(ledger[-1]['amount_out_raw']),100000000)
        for i in range(1,n):self.assertEqual(ledger[i-1]['amount_out_raw'],ledger[i]['amount_in_raw'])
    setattr(GraphAndQuotes,f'test_exact_cycle_{n}_hops',length_test)

class EconomicContract(unittest.TestCase):
    def test_positive_not_execution(self):
        o=r.economic(**vector());self.assertEqual(o['net_profit_usd'],'0.9');self.assertTrue(o['candidate_eligible']);self.assertFalse(o['approved_for_execution']);self.assertIsNone(o['simulation']['passed'])
    def test_loss_preserved(self):
        o=r.economic(**vector('99'));self.assertEqual(o['net_profit_usd'],'-1.1');self.assertFalse(o['candidate_eligible']);self.assertTrue(o['legs'])
    def test_flow_cannot_contradict_raw_ledger(self):
        v=vector();v['q']['incoming'][0]['usd']='150';self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_valuation_requires_price_revision(self):
        v=vector();v['q']['incoming'][0]['valuation']['price_revision']='OTHER';self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_same_asset_price_cannot_differ(self):
        v=vector();v['q']['incoming'][0]['valuation']['price_usd']='2';v['q']['incoming'][0]['usd']='202';self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_missing_valuation_not_accepted(self):
        v=vector();v['q']['incoming'][0]['valuation']=None;self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_zero_preserved(self):self.assertEqual(r.economic(**vector('100.1'))['net_profit_usd'],'0')
    def test_embedded_not_double_subtracted(self):self.assertEqual(r.economic(**vector())['external_cost_usd'],'0.1')
    def test_gas_not_in_net_omission(self):self.assertEqual(r.economic(**vector('101','1.5'))['net_profit_usd'],'-0.5')
    def test_retained_not_double_financed(self):
        v=vector();v['q']['profit_basis']='retained_after_repayment';v['q']['costs'][1].update(treatment='external',usd='0.1');self.assertIsNone(r.economic(**v)['net_profit_usd'])
    def test_loss_selected_not_discarded(self):
        rs=[r.economic(**vector('98')),r.economic(**vector('99'))];self.assertEqual(r.select(rs)['net_profit_usd'],'-1.1')
    def test_no_flows_is_not_zero(self):
        v=vector();v['q']['incoming']=[];o=r.economic(**v);self.assertIsNone(o['gross_profit_usd']);self.assertIsNone(o['net_profit_usd'])
    def test_invalid_output_keeps_partial_ledger(self):
        v=vector();v['q']['missing']=['quote_hop_2'];v['q']['status']='DATA_GAP';o=r.economic(**v);self.assertTrue(o['legs']);self.assertFalse(o['candidate_eligible'])
    def test_real_zero_operator_value(self):
        v=vector();v['evidence']['operators']['15']={'status':'COMPUTED','value':0,'evidence_id':'TEST_ZERO'};self.assertTrue(r.economic(**v)['candidate_eligible'])
    def test_fake_operator_nan_string(self):
        v=vector();v['evidence']['operators']['15']={'status':'COMPUTED','value':'NaN','evidence_id':'TEST_BAD'};self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_disabled_primary_needs_equivalent(self):
        v=vector();v['evidence']['operators']['15']={'status':'DISABLED','reason':'operator_switch_off'};self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_optional_off_no_invented_value(self):
        v=vector();o=r.economic(**v);self.assertTrue(o['candidate_eligible']);self.assertNotIn('value',o['operators']['operators']['1'])
    def test_no_constraint_guessing(self):
        v=vector();v['receipts']=[];self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_constraint_wrong_plan(self):
        v=vector();v['receipts'][0]['plan_hash']='OTHER';self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_duplicate_constraint(self):
        v=vector();v['receipts']*=2;self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_policy_cap_not_changed(self):
        v=vector();v['policy']['capital_cap_usd']='99';original=copy.deepcopy(v);self.assertFalse(r.economic(**v)['candidate_eligible']);self.assertEqual(v,original)
    def test_kill_switch(self):
        v=vector();v['policy']['control_state']='OFF';self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_disabled_strategy(self):
        v=vector();v['policy']['enabled']=False;self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_max_gas(self):
        v=vector();v['policy']['max_gas_usd']='0.05';self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_min_profit(self):
        v=vector();v['policy']['min_profit_usd']='1';self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_modes_same_economics(self):
        out=[]
        for mode in ['LIVE_MAINNET','TESTNET','PAPER_SHADOW']:
            v=vector();v['policy']['execution_mode']=mode;o=r.economic(**v);out.append(o['net_profit_usd']);self.assertFalse(o['approved_for_execution'])
        self.assertEqual(len(set(out)),1)
    def test_observe_no_execution(self):
        v=vector();v['spec']['logic']='observe_only';o=r.economic(**v);self.assertEqual(o['status'],'OBSERVE_ONLY');self.assertFalse(o['candidate_eligible'])
    def test_conflict_not_silently_overridden(self):
        v=vector();v['spec']['conflicts']=['NON_CYCLIC_NAME_VS_CLOSED_CYCLE_SPECIFIC_NOTE'];self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_execution_improvement_not_profit(self):
        v=vector();v['spec']['logic']='path_comparison';v['q']['economic_kind']='execution_improvement';o=r.economic(**v);self.assertEqual(o['status'],'IMPROVEMENT_ONLY');self.assertFalse(o['candidate_eligible'])
    def test_carry_not_atomic(self):
        v=vector();v['spec']['logic']='carry_projection';self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_bridge_not_atomic(self):
        v=vector();v['spec']['logic']='cross_domain';self.assertFalse(r.economic(**v)['candidate_eligible'])
    def test_projections_labelled(self):
        v=vector();v['spec']['logic']='carry_projection';v['q']['economic_kind']='expected_carry';o=r.economic(**v);self.assertEqual(o['economic_kind'],'expected_carry');self.assertFalse(o['approved_for_execution'])

for k in ['context_id','snapshot_id','price_revision','policy_revision','plan_id','plan_hash']:
    def changed(self,k=k):
        v=vector();v['q'][k]='OTHER';self.assertFalse(r.economic(**v)['candidate_eligible'])
    setattr(EconomicContract,'test_revision_'+k,changed)
for k in ['gas','financing','execution_fees']:
    def missing(self,k=k):
        v=vector();v['q']['costs']=[l for l in v['q']['costs']if l['kind']!=k];self.assertIsNone(r.economic(**v)['net_profit_usd'])
    setattr(EconomicContract,'test_missing_cost_'+k,missing)
for field in ['usd','evidence_id']:
    def invalid(self,field=field):
        v=vector();v['q']['costs'][0][field]=None;self.assertIsNone(r.economic(**v)['net_profit_usd'])
    setattr(EconomicContract,'test_invalid_cost_'+field,invalid)

if __name__=='__main__':unittest.main()
