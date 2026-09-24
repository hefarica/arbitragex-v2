import unittest
from dataclasses import replace
from forensic_integrity.integrity import digest
from forensic_integrity.quote_router import *
class ProtocolTests(unittest.TestCase):
    def setUp(self):
        self.snap={'pool_address':'unit-pool','chain_id':1,'block_hash':'0x'+'1'*64,'token0':'A','token1':'B','reserve0':'1000','reserve1':'1000','fee_numerator':3,'fee_denominator':1000}
        self.request=QuoteRequest('uniswap_v2_cpmm',1,self.snap['block_hash'],'unit-pool','A','B','100',digest(self.snap))
        self.router=QuoteRouter();self.router.register('uniswap_v2_cpmm',v2_reference_adapter(self.snap))
    def test_exact_v2_bound_quote(self):self.assertEqual(self.router.quote(self.request)['amount_out_raw'],'90')
    def test_unknown_protocol_no_cpmm_fallback(self):
        q=self.router.quote(replace(self.request,protocol='curve_stableswap'));self.assertEqual(q['reason'],'protocol_adapter_unavailable');self.assertIsNone(q['amount_out_raw'])
    def test_disguised_protocol_rejected(self):
        self.router.register('curve_stableswap',v2_reference_adapter(self.snap));self.assertEqual(self.router.quote(replace(self.request,protocol='curve_stableswap'))['state'],'not_computed')
    def test_wrong_block_not_reused(self):self.assertEqual(self.router.quote(replace(self.request,block_hash='0x'+'2'*64))['state'],'not_computed')
    def test_wrong_input_response_rejected(self):
        self.router.register('bad',lambda req:QuoteEvidence('b'*64,'1','onchain_quote','test-only-provider','a'*64,'unit'))
        self.assertEqual(self.router.quote(replace(self.request,protocol='bad'))['reason'],'stale_or_cross_request_quote')
    def test_duplicate_registration_refused(self):
        with self.assertRaises(ValueError):self.router.register('uniswap_v2_cpmm',v2_reference_adapter(self.snap))
    def test_unknown_output_not_zero(self):
        self.router.register('bad',lambda req:QuoteEvidence(digest(asdict(req)),'NaN','onchain_quote','unit','a'*64,'unit'))
        self.assertIsNone(self.router.quote(replace(self.request,protocol='bad'))['amount_out_raw'])
if __name__=='__main__':unittest.main()
