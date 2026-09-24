"""Synthetic unit vectors only. These records are NOT live evidence."""
import copy
import unittest
from forensic_integrity.integrity import *
from forensic_integrity.evidence import evaluate_evidence

CTX='a'*64
class IntegrityTests(unittest.TestCase):
    def chain(self):
        out=[]
        for i,stage in enumerate(DEFAULT_STAGES):
            out.append(make_receipt('unit-event',stage,CTX,{'net':'-3.0','zero':'0'},1000+i,out[-1] if out else None))
        return out
    def test_hash_deterministic(self):self.assertEqual(digest({'b':1,'a':'x'}),digest({'a':'x','b':1}))
    def test_hash_state_sensitive(self):self.assertNotEqual(digest({'v':None}),digest({'v':'0'}))
    def test_no_float(self):
        with self.assertRaises(ValueError):canonical_bytes({'v':0.5})
    def test_no_nan(self):
        with self.assertRaises(ValueError):canonical_bytes({'v':float('nan')})
    def test_no_unsafe_integer(self):
        with self.assertRaises(ValueError):canonical_bytes({'v':2**53})
    def test_no_nonascii_keys(self):
        with self.assertRaises(ValueError):canonical_bytes({'á':1})
    def test_signed_integer_string(self):self.assertIn(b'-100000000000000000000',canonical_bytes({'x':'-100000000000000000000'}))
    def test_null_zero_separate_leaves(self):self.assertEqual(flatten({'a':None,'b':0}),{'a':None,'b':0})
    def test_empty_containers_kept(self):self.assertEqual(flatten({'a':[],'b':{}}),{'a':[],'b':{}})
    def test_full_receipt_chain(self):self.assertEqual(reconcile(self.chain(),['unit-event'])['verification'],'CONSISTENT')
    def test_not_production_proof(self):self.assertFalse(reconcile(self.chain(),['unit-event'])['production_delivery_certified'])
    def test_empty_is_not_100_percent(self):self.assertIsNone(reconcile([],[])['observed_completeness_ratio'])
    def test_expected_census_missing_event(self):self.assertEqual(reconcile(self.chain(),['unit-event','missing'])['complete_events'],1)
    def test_missing_pg_fails(self):self.assertEqual(reconcile([r for r in self.chain() if r['stage']!='persisted'],['unit-event'])['complete_events'],0)
    def test_duplicate_idempotent(self):
        c=self.chain();r=reconcile(c+[c[0]],['unit-event']);self.assertEqual(r['exact_duplicate_receipts'],1);self.assertEqual(r['complete_events'],1)
    def test_duplicate_conflict(self):
        c=self.chain();other=make_receipt('unit-event','ingested',CTX,{'net':'7'},1000)
        self.assertTrue(reconcile(c+[other],['unit-event'])['invalid_receipts'])
    def test_arrival_order_independent(self):self.assertEqual(reconcile(list(reversed(self.chain())),['unit-event'])['complete_events'],1)
    def test_payload_tamper(self):
        c=self.chain();c[2]['payload']['net']='10';self.assertTrue(verify_receipt(c[2]))
    def test_broken_parent(self):
        c=self.chain();c[1]['parent_hash']='c'*64;c[1]['receipt_hash']=digest({k:v for k,v in c[1].items() if k!='receipt_hash'})
        self.assertEqual(reconcile(c,['unit-event'])['complete_events'],0)
    def test_context_change_refused(self):
        with self.assertRaises(ValueError):make_receipt('unit-event','api_served','b'*64,{},2000,self.chain()[0])
    def test_cross_opportunity_refused(self):
        with self.assertRaises(ValueError):make_receipt('other','api_served',CTX,{},2000,self.chain()[0])
    def test_sla_breach(self):self.assertEqual(reconcile(self.chain(),['unit-event'],max_latency_ms=2)['complete_events'],0)
    def test_controls_require_applied(self):
        c=[]
        for i,stage in enumerate(CONTROL_STAGES):c.append(make_receipt('toggle-unit',stage,CTX,{'enabled':False},1000+i,c[-1] if c else None))
        self.assertEqual(reconcile(c,['toggle-unit'],CONTROL_STAGES)['complete_events'],1)
        self.assertEqual(reconcile(c[:2],['toggle-unit'],CONTROL_STAGES)['complete_events'],0)
    def test_undeclared_stage(self):
        r=make_receipt('unit-event','not_an_ack',CTX,{},1000);self.assertTrue(reconcile([r],['unit-event'])['invalid_receipts'])
    def test_absence_reason(self):
        e={'schema':'arbx.field-lineage.v1','event_id':'unit','strategy_id':'triangular','context_id':CTX,'route_hash':CTX,'config_hash':CTX,'strategy_revision':CTX,'source_commit':'a'*40,'fields':{'net':{'state':'missing','value':None,'unit':'USD','reason':'not_evaluated'}}}
        self.assertEqual(validate_envelope(e),[])
        e['fields']['net']['value']='0';self.assertTrue(validate_envelope(e))
    def test_truncated_block_hash_refused(self):
        e={'fields':{'block':{'state':'observed','value':'19584777','unit':'block','source':{'kind':'onchain','reference':'eth_getBlockByNumber','as_of_ms':1000,'chain_id':1,'block_hash':'0x7a3f...'}}}}
        self.assertIn('block:unverifiable_block_hash',validate_envelope(e))
    def test_required_operator_denominator_kept(self):
        c=operator_coverage([1,2,3,35],[1,2,3],[2],[{'operator_id':1,'context_id':CTX,'state':'computed','value':'0'}],CTX)
        self.assertEqual((c['required'],c['computed']),(4,1));self.assertEqual(c['operators'][1]['state'],'disabled')
    def test_operator_context_mismatch(self):
        r=operator_coverage([1],[1],[],[{'operator_id':1,'context_id':'b'*64,'state':'computed','value':'9'}],CTX)
        self.assertEqual(r['computed'],0)
    def test_operator_nonfinite(self):
        r=operator_coverage([1],[1],[],[{'operator_id':1,'context_id':CTX,'state':'computed','value':'NaN'}],CTX)
        self.assertEqual(r['computed'],0)
    def snapshot(self,value='0'):
        return {'schema':'arbx.bound-evidence.v1','context_id':CTX,'operators':[{'operator_id':32,'context_id':CTX,'state':'computed','value':value}]}
    def calibration(self):return {'version':'unit-model-v1','evidence_schema':'arbx.bound-evidence.v1','weights_by_operator':{'32':'1'}}
    def test_bound_object_evidence_supported(self):
        r=evaluate_evidence(self.snapshot(),CTX,[32],[32],[],'0',self.calibration());self.assertEqual(r['posterior_probability'],'0.5')
    def test_no_legacy_array_upgrade(self):
        r=evaluate_evidence({'operators':[0]*31},CTX,[32],[32],[],'0',self.calibration());self.assertEqual(r['state'],'legacy_or_unknown_schema')
    def test_missing_calibration_not_zero(self):
        r=evaluate_evidence(self.snapshot(),CTX,[32],[32],[],'0',None);self.assertIsNone(r['posterior_probability'])
    def test_missing_operator_not_zero(self):
        r=evaluate_evidence(self.snapshot(),CTX,[1,32],[1,32],[],'0',self.calibration());self.assertEqual(r['state'],'incomplete')
    def test_posterior_stable_large_negative(self):
        r=evaluate_evidence(self.snapshot('-10000'),CTX,[32],[32],[],'0',self.calibration());self.assertEqual(r['state'],'computed_model')
    def test_evidence_wrong_context_rejected(self):
        r=evaluate_evidence(self.snapshot(),'b'*64,[32],[32],[],'0',self.calibration());self.assertEqual(r['state'],'context_mismatch')
    def test_disabled_operator_never_computed(self):
        r=evaluate_evidence(self.snapshot(),CTX,[32],[32],[32],'0',self.calibration());self.assertEqual(r['state'],'incomplete')

if __name__=='__main__':unittest.main()
