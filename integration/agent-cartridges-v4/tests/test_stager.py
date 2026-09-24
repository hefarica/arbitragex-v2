import unittest,tempfile,sys,hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT));import stage_update as st
class Stager(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.repo=Path(self.tmp.name)/'repo';self.repo.mkdir();(self.repo/'backend').mkdir();(self.repo/'.github').mkdir();(self.repo/'.github/workflow.yml').write_text('KEEP');(self.repo/'.env').write_text('KEEP ENV')
        self.original_items=st.items
        self.file=ROOT/'runtime/reference_engine.py';st.items=lambda:[self.file]
    def tearDown(self):st.items=self.original_items;self.tmp.cleanup()
    def test_dry_run_writes_nothing(self):st.stage(self.repo);self.assertFalse((self.repo/'integration').exists())
    def test_add_only(self):
        o=st.stage(self.repo,True);self.assertEqual(o['files_added'],1);self.assertEqual((self.repo/'.env').read_text(encoding="utf-8"),'KEEP ENV');self.assertEqual((self.repo/'.github/workflow.yml').read_text(encoding="utf-8"),'KEEP')
    def test_idempotent(self):st.stage(self.repo,True);self.assertEqual(st.stage(self.repo,True)['files_added'],0)
    def test_different_existing_refused(self):
        st.stage(self.repo,True);p=self.repo/st.STAGE/'runtime/reference_engine.py';p.write_text('UNRELATED');self.assertRaises(ValueError,st.stage,self.repo,True);self.assertEqual(p.read_text(encoding="utf-8"),'UNRELATED')
    def test_no_active_replacement(self):
        p=self.repo/'backend/searcher-rs/cartridges/strategies';p.mkdir(parents=True);(p/'existing.rhai').write_text('KEEP SCRIPT');st.stage(self.repo,True);self.assertEqual((p/'existing.rhai').read_text(encoding="utf-8"),'KEEP SCRIPT')
    def test_no_root_identification(self):self.assertRaises(ValueError,st.stage,Path(self.tmp.name))
    def test_symlink_refused(self):
        (self.repo/'integration').symlink_to(self.tmp.name,target_is_directory=True);self.assertRaises(ValueError,st.stage,self.repo,True)
if __name__=='__main__':unittest.main()
