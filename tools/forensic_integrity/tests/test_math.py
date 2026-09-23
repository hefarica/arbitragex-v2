"""Independent integer/rational unit vectors, not production market samples."""
import unittest
from dataclasses import replace
from fractions import Fraction
from forensic_integrity.math_reference import *

class MathTests(unittest.TestCase):
    def pool(self,identifier='p',r0=1000,r1=1000,token0='A',token1='B'):
        return PoolSnapshot(identifier,token0,token1,r0,r1,3,1000,1,'0x'+'1'*64)
    def test_v2_pinned_vector(self):self.assertEqual(v2_amount_out(100,1000,1000,3,1000),90)
    def test_v2_zero_floor(self):self.assertEqual(v2_amount_out(1,1000,2,3,1000),0)
    def test_no_default_fee(self):
        with self.assertRaises(TypeError):v2_amount_out(100,1000,1000)
    def test_zero_liquidity_rejected(self):
        with self.assertRaises(ValueError):v2_amount_out(10,0,100,3,1000)
    def test_v2_fractional_unit_not_float(self):
        with self.assertRaises(ValueError):v2_amount_out(1.5,100,100,3,1000)
    def test_fee100percent_invalid(self):
        with self.assertRaises(ValueError):v2_amount_out(10,100,100,1000,1000)
    def test_uint_overflow(self):
        with self.assertRaises(ValueError):v2_amount_out((1<<256)-1,1000,1000,3,1000)
    def test_bool_not_amount(self):
        with self.assertRaises(ValueError):uint(True)
    def test_loss_signed(self):
        a=self.pool();b=self.pool('q');r=quote_cycle(100,[Hop(a,'A','B'),Hop(b,'B','A')],7)
        self.assertEqual(r['amount_out_raw'],'82');self.assertEqual(r['gross_profit_raw'],'-18')
    def test_no_execution_authorization(self):
        r=quote_cycle(100,[Hop(self.pool(),'A','B'),Hop(self.pool('q'),'B','A')],7);self.assertFalse(r['execution_authorized'])
    def test_repeated_pool_mutates_reserves(self):
        a=self.pool();r=quote_cycle(100,[Hop(a,'A','B'),Hop(a,'B','A')],7)
        self.assertEqual(r['amount_out_raw'],'98');self.assertEqual(r['gross_profit_raw'],'-2')
    def test_zero_intermediate_is_loss_not_zero_profit(self):
        a=self.pool('p',1000,2);b=self.pool('q',1000,1000)
        r=quote_cycle(1,[Hop(a,'A','B'),Hop(b,'B','A')],7);self.assertEqual(r['gross_profit_raw'],'-1')
    def test_snapshot_mismatch(self):
        a=self.pool();b=replace(self.pool('q'),block_hash='0x'+'2'*64)
        with self.assertRaises(ValueError):quote_cycle(100,[Hop(a,'A','B'),Hop(b,'B','A')],7)
    def test_chain_mismatch(self):
        a=self.pool();b=replace(self.pool('q'),chain_id=2)
        with self.assertRaises(ValueError):quote_cycle(100,[Hop(a,'A','B'),Hop(b,'B','A')],7)
    def test_contradicting_repeated_pool(self):
        a=self.pool();b=replace(a,reserve0=999)
        with self.assertRaises(ValueError):quote_cycle(100,[Hop(a,'A','B'),Hop(b,'B','A')],7)
    def test_open_route(self):
        with self.assertRaises(ValueError):quote_cycle(100,[Hop(self.pool(),'A','B'),Hop(self.pool('q'),'A','B')],7)
    def test_unsupported_hops(self):
        with self.assertRaises(ValueError):quote_cycle(100,[Hop(self.pool(),'A','B')],7)
    def test_v3_price_one(self):self.assertEqual(sqrt_x96_price(1<<96,18,18),1)
    def test_v3_decimals(self):self.assertEqual(sqrt_x96_price(1<<96,6,18),Fraction(1,10**12))
    def test_v3_not_right_shift(self):self.assertEqual(sqrt_x96_price(3*(1<<95),18,18),Fraction(9,4))
    def test_v3_zero_invalid(self):
        with self.assertRaises(ValueError):sqrt_x96_price(0,18,18)
    def test_v3_uint160(self):
        with self.assertRaises(ValueError):sqrt_x96_price(1<<160,18,18)
    def test_fee_pips_to_bps(self):self.assertEqual(fee_rate(3000,'pips'),fee_rate(30,'bps'))
    def test_fee_no_unit(self):
        with self.assertRaises(ValueError):fee_rate(3000,'unknown')
    def test_ledger_negative_preserved(self):self.assertEqual(economic_ledger('100','99',{'gas':'3'},['lp_fees'])['net_profit_usd'],'-4')
    def test_lp_fee_double_count_rejected(self):
        with self.assertRaises(ValueError):economic_ledger('100','101',{'lp_fees':'1'},['lp_fees'])
    def test_roi_bps(self):self.assertEqual(economic_ledger('100','105',{'gas':'1'},[])['roi_bps'],'400.00')
    def test_missing_cost_not_zero(self):
        with self.assertRaises(ValueError):economic_ledger('100','105',{'gas':None},[])
    def test_decimal_float_rejected(self):
        with self.assertRaises(ValueError):decimal(0.1)
    def test_decimal_nonfinite_rejected(self):
        with self.assertRaises(ValueError):decimal('Infinity')
    def samples(self):return [{'base_asset':'A','quote_asset':'B','price':'1','timestamp_ms':1000,'block_hash':'h1','venue':'v1'},{'base_asset':'A','quote_asset':'B','price':'2','timestamp_ms':2000,'block_hash':'h2','venue':'v1'}]
    def test_temporal_prices(self):self.assertEqual(homogeneous_prices(self.samples(),'time'),[Decimal(1),Decimal(2)])
    def test_mixed_assets_refused(self):
        s=self.samples();s[1]['quote_asset']='C'
        with self.assertRaises(ValueError):homogeneous_prices(s,'time')
    def test_venues_not_time(self):
        s=self.samples();s[1]['venue']='v2'
        with self.assertRaises(ValueError):homogeneous_prices(s,'time')
    def test_venue_snapshot(self):
        with self.assertRaises(ValueError):homogeneous_prices(self.samples(),'venue')
    def test_monotonic_output_property_on_small_domain(self):
        outs=[v2_amount_out(i,1000,3000,3,1000) for i in range(1,1000)]
        self.assertEqual(outs,sorted(outs));self.assertTrue(all(0<=x<3000 for x in outs))
    def test_fee_reduces_output(self):
        for n in range(1,100):self.assertLessEqual(v2_amount_out(n,1000,3000,3,1000),v2_amount_out(n,1000,3000,0,1000))
if __name__=='__main__':unittest.main()
