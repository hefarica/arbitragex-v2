"""Synthetic deterministic fixtures. NOT historical or live market captures."""
import copy
import unittest
from dataclasses import replace
from forensic_integrity.integrity import digest
from forensic_integrity.real_cards import CanonicalPriceView, CardPolicy, COST_NAMES, asset_key, compute_closed_cycle_card, strict_card_audit

H = 'a' * 64
B = '0x' + 'b' * 64
NOW = 1_800_000_000_000
A = asset_key(1, '0x' + '1' * 40)
C = asset_key(1, '0x' + '2' * 40)
REQUIRED = ('capital_in_usd', 'gross_out_usd', 'gross_profit_usd', 'additional_cost_usd', 'net_profit_usd', 'roi_pct', 'net_bps', 'hop_count', *('costs.' + x for x in COST_NAMES))

def fixtures():
    p = CardPolicy(NOW, 500, 100, 5, 7, REQUIRED, {})
    prices = {}
    for key, price in ((A, '1'), (C, '2')):
        prices[key] = {'asset_key': key, 'currency': 'USD', 'purpose': 'valuation', 'price_usd': price,
                       'verdict': 'ok', 'observed_at_ms': NOW-10, 'valid_until_ms': NOW+100,
                       'source_references': ['TEST_ONLY:canonical_pricebus'], 'input_hashes': [H]}
    snap = {'schema': 'arbx.pricebus.export.v1', 'source': 'canonical_pricebus', 'policy_hash': H,
            'generated_at_ms': NOW-5, 'prices': prices}
    seal(snap)
    meta = {k: {'asset_key': k, 'decimals': d, 'source_reference': 'TEST_ONLY:token_metadata', 'evidence_hash': H}
            for k, d in ((A, 6), (C, 18))}
    def leg(ai, ao, x, y):
        return {'asset_in': ai, 'asset_out': ao, 'amount_in_raw': x, 'amount_out_raw': y,
                'context_id': H, 'block_hash': B, 'quote_evidence_hash': H,
                'quote_reference': 'TEST_ONLY:protocol_quote', 'protocol': 'unit_test_protocol', 'quoted_at_ms': NOW-20}
    req = {'event_id': 'TEST_EVENT', 'strategy_id': 'BASE:triangular', 'context_id': H,
           'source_commit': 'c'*40, 'strategy_revision': H, 'config_hash': H, 'route_hash': H,
           'route_kind': 'closed_cycle', 'block_hash': B, 'token_metadata': meta,
           'legs': [leg(A,C,'100000000','50000000000000000000'),leg(C,A,'50000000000000000000','103000000')],
           'costs': {}, 'rejection_reason': 'TEST_lifecycle_reason'}
    for k in COST_NAMES:
        req['costs'][k] = {'context_id': H, 'valid_until_ms': NOW+100, 'evidence_hash': H,
                           'reference': 'TEST_ONLY:cost', 'basis': 'usd_computed', 'amount_usd': '0',
                           'treatment': 'included_in_quote' if k in {'lp_fees','slippage'} else 'deduct'}
    req['costs']['gas']['amount_usd'] = '1'
    return req, snap, p


def seal(s):
    s['snapshot_hash'] = digest({k:v for k,v in s.items() if k!='snapshot_hash'})


def run(req, snap, p):
    return compute_closed_cycle_card(req,snap,p)


class RealCards(unittest.TestCase):
    def setUp(self): self.r,self.s,self.p=fixtures()
    def val(self,res,k):return res['envelope']['fields'][k]['value']
    def test_full_exact_card(self):
        o=run(self.r,self.s,self.p)
        self.assertTrue(o['audit']['complete'],o['audit']['work_orders'])
        self.assertEqual(self.val(o,'net_profit_usd'),'2')
        self.assertEqual(self.val(o,'roi_pct'),'2')
        self.assertEqual(self.val(o,'net_bps'),'200')
    def test_no_execution_or_live_certificate(self):
        o=run(self.r,self.s,self.p)
        self.assertFalse(o['execution_authorized']);self.assertFalse(o['audit']['production_verified'])
    def test_preserve_rejection_even_positive_card(self):
        self.assertEqual(run(self.r,self.s,self.p)['lifecycle_rejection_reason'],'TEST_lifecycle_reason')
    def test_negative_net_not_clipped(self):
        self.r['costs']['gas']['amount_usd']='8'
        self.assertEqual(self.val(run(self.r,self.s,self.p),'net_profit_usd'),'-5')
    def test_negative_gross_not_discarded(self):
        self.r['legs'][-1]['amount_out_raw']='97000000'
        self.assertEqual(self.val(run(self.r,self.s,self.p),'gross_profit_usd'),'-3')
    def test_exact_zero_not_missing(self):
        self.r['legs'][-1]['amount_out_raw']='101000000'
        o=run(self.r,self.s,self.p)
        self.assertEqual(self.val(o,'net_profit_usd'),'0');self.assertTrue(o['audit']['complete'])
    def test_input_decimals_not_18(self):self.assertEqual(self.val(run(self.r,self.s,self.p),'capital_in_usd'),'100')
    def test_all_hop_values(self):
        o=run(self.r,self.s,self.p)
        self.assertEqual(self.val(o,'legs[0].in_usd'),'100')
        self.assertEqual(self.val(o,'legs[1].out_usd'),'103')
    def test_no_double_fee_deduction(self):
        self.r['costs']['lp_fees']['amount_usd']='2'
        self.assertEqual(self.val(run(self.r,self.s,self.p),'net_profit_usd'),'2')
    def test_missing_cost_makes_net_fail(self):
        del self.r['costs']['gas']
        o=run(self.r,self.s,self.p)
        self.assertFalse(o['audit']['complete']);self.assertIsNone(self.val(o,'net_profit_usd'))
    def test_stablecoin_depeg_not_one(self):
        self.s['prices'][A]['price_usd']='0.9';seal(self.s)
        self.assertEqual(self.val(run(self.r,self.s,self.p),'capital_in_usd'),'90')
    def test_missing_price_not_one(self):
        del self.s['prices'][A];seal(self.s)
        self.assertIsNone(self.val(run(self.r,self.s,self.p),'capital_in_usd'))
    def test_frozen_price_never_revalued(self):
        self.s['prices'][A]['verdict']='price_divergence_binance_chainlink';seal(self.s)
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_stale_anchor_never_verified(self):
        self.s['prices'][A]['verdict']='stale_anchor';seal(self.s)
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_anchor_only_valid_for_valuation(self):
        self.s['prices'][A]['verdict']='anchor_only';seal(self.s)
        self.assertTrue(run(self.r,self.s,self.p)['audit']['complete'])
    def test_corrupt_snapshot_hash(self):
        self.s['prices'][A]['price_usd']='2'
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_stale_export(self):
        self.s['generated_at_ms']=NOW-101;seal(self.s)
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_source_age_not_reset_by_fresh_export(self):
        self.s['prices'][A]['valid_until_ms']=NOW-1;seal(self.s)
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_future_source_not_zero_age(self):
        self.s['prices'][A]['observed_at_ms']=NOW+200;seal(self.s)
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_price_quote_not_usd_cannot_masquerade(self):
        self.s['prices'][A]['currency']='USDT';seal(self.s)
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_float_price_refused(self):
        self.s['prices'][A]['price_usd']=1.0
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_missing_decimals_no_default(self):
        del self.r['token_metadata'][A]['decimals']
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_quote_from_another_context(self):
        self.r['legs'][1]['context_id']='d'*64
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_quote_from_another_block(self):
        self.r['legs'][1]['block_hash']='0x'+'d'*64
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_stale_quote_never_fresh_by_price(self):
        self.r['legs'][1]['quoted_at_ms']=NOW-501
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_discontinuous_ledger(self):
        self.r['legs'][1]['amount_in_raw']='50000000000000000001'
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_unknown_family_never_forced_through_cpmm(self):
        self.r['route_kind']='nft_sale'
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_risk_not_from_price(self):
        p=replace(self.p,required_fields=(*REQUIRED,'risk_score'))
        o=run(self.r,self.s,p)
        self.assertFalse(o['audit']['complete']);self.assertIsNone(self.val(o,'risk_score'))
    def test_simulation_not_from_arithmetic(self):
        p=replace(self.p,required_fields=(*REQUIRED,'simulation_result'))
        self.assertFalse(run(self.r,self.s,p)['audit']['complete'])
    def test_na_requires_field_specific_rule(self):
        self.r['costs']['flash_fee']={'state':'not_applicable','reason':'own_capital'}
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])
    def test_legitimate_na(self):
        self.r['costs']['flash_fee']={'state':'not_applicable','reason':'own_capital'}
        p=replace(self.p,na_reasons={'costs.flash_fee':('own_capital',)})
        o=run(self.r,self.s,p);self.assertTrue(o['audit']['complete'])
        self.assertIsNone(self.val(o,'costs.flash_fee'))
    def test_producer_cannot_overwrite_net(self):
        self.r['producer_fields']={'net_profit_usd': {'value':'999999'}}
        o=run(self.r,self.s,self.p)
        self.assertEqual(self.val(o,'net_profit_usd'),'2');self.assertFalse(o['audit']['complete'])
    def test_wo_ids_are_idempotent(self):
        del self.r['costs']['gas']
        self.assertEqual(run(self.r,self.s,self.p)['audit']['work_orders'],run(self.r,self.s,self.p)['audit']['work_orders'])
    def test_atomic_snapshot_consumer(self):
        view=CanonicalPriceView(self.s,self.p)
        self.s['prices'][A]['price_usd']='9'
        self.assertEqual(str(view.price(A)[0]),'1')
    def test_placeholder_in_computed_field_fails(self):
        o=run(self.r,self.s,self.p)
        o['envelope']['fields']['net_profit_usd']['value']='no computado'
        self.assertFalse(strict_card_audit(o['envelope'],self.p)['complete'])
    def test_undeclared_extra_is_audited_too(self):
        o=run(self.r,self.s,self.p)
        extra=copy.deepcopy(o['envelope']['fields']['net_profit_usd']);extra['context_id']='d'*64
        o['envelope']['fields']['extra']=extra
        self.assertFalse(strict_card_audit(o['envelope'],self.p)['complete'])
    def test_gas_exact_native_conversion(self):
        # 100000 gas * 1 gwei = 0.0001 native token, marked at fixture price $2.
        self.r['native_asset_key']=C
        self.r['token_metadata'][C]['native_currency']=True
        self.r['costs']['gas'].update(basis='native_gas',gas_units='100000',effective_gas_price_wei='1000000000',asset_key=C)
        self.assertEqual(self.val(run(self.r,self.s,self.p),'costs.gas'),'0.0002')
    def test_snapshot_missing_reference(self):
        self.s['prices'][A]['source_references']=[];seal(self.s)
        self.assertFalse(run(self.r,self.s,self.p)['audit']['complete'])

if __name__=='__main__':unittest.main()
