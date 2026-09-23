import unittest,json,sys,hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'tools'))
from structural_checks import check_script
S=json.loads((ROOT/'spec/strategies.json').read_text(encoding="utf-8"))
class SourceConformance(unittest.TestCase):
    def test_exact_id_set(self):
        tables=json.loads((ROOT/'spec/source_tables.json').read_text(encoding="utf-8"))
        self.assertEqual({s['mev_id']for s in S},{s['MEV_ID']for s in tables['master']})
    def test_distinct_264_files(self):self.assertEqual(len(set(s['filename']for s in S)),264)
    def test_8184_roles(self):self.assertEqual(sum(len(s['operator_roles'])for s in S),8184)
    def test_no_weight_invention(self):self.assertTrue(all(r['Resolved_Weight']is None for s in S for r in s['operator_roles']))
    def test_exact_triangular_scope(self):self.assertEqual(next(s for s in S if s['mev_id']=='MEV-01-016')['effective_search_hops'],[3])
    def test_exact_quadrangular_scope(self):self.assertEqual(next(s for s in S if s['mev_id']=='MEV-01-017')['effective_search_hops'],[4])
    def test_source_16_preserved(self):self.assertTrue(any(s['source_leg_bounds']['max']==16 for s in S))
    def test_no_op32_assigned(self):self.assertTrue(all(r['Operator_ID']<=31 for s in S for r in s['operator_roles']))
    def test_all60families(self):self.assertEqual(len({s['detector_id']for s in S}),60)
    def test_all14_logic_classes(self):self.assertEqual(len({s['family_contract']['logic']for s in S}),14)
    def test_no_original_configs_in_generated(self):self.assertEqual(len(list((ROOT/'generated/cartridges').glob('*.rhai'))),0)
    def test_7_root_originals_preserved(self):self.assertEqual(len(list((ROOT/'reference_repo/backend/searcher-rs/cartridges').glob('*.rhai'))),7)
    def test_original_script_hashes(self):
        for s in S:
            f=ROOT/'reference_repo/backend/searcher-rs/cartridges/strategies'/s['filename'];self.assertEqual(hashlib.sha256(f.read_bytes()).hexdigest(),s['original_sha256'])
    def test_every_source_formula_kept(self):
        for s in S:self.assertEqual(s['equation'],s['source_math_row']['Ecuación / criterio de descubrimiento'])
for s in S:
    setattr(SourceConformance,'test_cartridge_'+s['mev_id'].replace('-','_'),lambda self,s=s:self.assertEqual(check_script(ROOT/'generated/cartridges/strategies'/s['filename'],s)['status'],'STRUCTURAL_PASS_NOT_COMPILED'))
if __name__=='__main__':unittest.main()
