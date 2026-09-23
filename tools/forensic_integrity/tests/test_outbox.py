import tempfile
import sqlite3
import contextlib
import unittest
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
from forensic_integrity.outbox import AuditOutbox
class OutboxTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.path=Path(self.temp.name)/'audit.sqlite';self.box=AuditOutbox(self.path)
    def tearDown(self):self.temp.cleanup()
    def put(self,key='unit'):return self.box.enqueue(key,{'value':'-1'},1000,20000)
    def test_survives_reopen(self):self.put();self.assertEqual(AuditOutbox(self.path).states(),{'pending':1})
    def test_idempotent_enqueue(self):self.put();self.put();self.assertEqual(self.box.states(),{'pending':1})
    def test_conflict_refused(self):
        self.put()
        with self.assertRaises(ValueError):self.box.enqueue('unit',{'value':'2'},1000,20000)
    def test_claim_ack(self):
        h=self.put();r=self.box.claim(1001,100,1)[0];self.box.ack('unit',r['lease_token'],h,'test-sink-receipt');self.assertEqual(self.box.states(),{'acked':1})
    def test_ack_wrong_hash(self):
        self.put();r=self.box.claim(1001,100,1)[0]
        with self.assertRaises(ValueError):self.box.ack('unit',r['lease_token'],'bad','receipt')
    def test_ack_requires_receipt(self):
        h=self.put();r=self.box.claim(1001,100,1)[0]
        with self.assertRaises(ValueError):self.box.ack('unit',r['lease_token'],h,'')
    def test_lease_recovery_after_crash(self):
        self.put();r=self.box.claim(1001,100,1)[0];self.assertEqual(self.box.claim(1050,100,1),[])
        r2=AuditOutbox(self.path).claim(1102,100,1)[0];self.assertNotEqual(r['lease_token'],r2['lease_token']);self.assertEqual(r2['attempt'],2)
    def test_old_worker_cannot_ack(self):
        h=self.put();r=self.box.claim(1001,10,1)[0];self.box.claim(1012,10,1)
        with self.assertRaises(ValueError):self.box.ack('unit',r['lease_token'],h,'receipt')
    def test_expiry_retained(self):
        self.put();self.assertEqual(self.box.claim(20001,100,1),[]);self.assertEqual(self.box.states(),{'expired':1})
    def test_retry_backoff(self):
        self.put();r=self.box.claim(1001,100,1)[0];self.box.fail('unit',r['lease_token'],'TIMEOUT',1200,3)
        self.assertEqual(self.box.claim(1100,100,1),[]);self.assertEqual(len(self.box.claim(1200,100,1)),1)
    def test_dead_letter_retained(self):
        self.put();r=self.box.claim(1001,100,1)[0];self.box.fail('unit',r['lease_token'],'INVALID',1200,1);self.assertEqual(self.box.states(),{'dead_letter':1})
    def test_parallel_claim_disjoint(self):
        for i in range(10):self.put('u'+str(i))
        with ThreadPoolExecutor(2) as ex:rows=list(ex.map(lambda _:self.box.claim(1001,100,5),range(2)))
        self.assertEqual(len({r['event_id'] for batch in rows for r in batch}),10)
    def test_other_database_not_modified(self):
        p=Path(self.temp.name)/'foreign.sqlite'
        # sqlite3 connection context manager commits but does NOT close; the
        # leaked handle blocks TemporaryDirectory cleanup on Windows (WinError 32).
        with contextlib.closing(sqlite3.connect(p)) as db:db.execute('CREATE TABLE important(x)')
        data=p.read_bytes()
        with self.assertRaises(ValueError):AuditOutbox(p)
        self.assertEqual(data,p.read_bytes())
if __name__=='__main__':unittest.main()
