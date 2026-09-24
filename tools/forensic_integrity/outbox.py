"""Durable local outbox for AUDIT receipts, not for trade execution.

Own SQLite file only; never substitutes production PostgreSQL. At-least-once
transport: a crash after sink-write/before ack may redeliver. The sink must dedup
by event_id + payload_hash. Retains expired/dead-letter rows; no silent deletion.
"""
from __future__ import annotations
import json
import sqlite3
import uuid
from contextlib import contextmanager
from pathlib import Path
from typing import Any
from .integrity import canonical_bytes, digest

APP_ID=0x41524258

class AuditOutbox:
    def __init__(self,path:Path):
        if path.is_symlink():raise ValueError('symlink outbox refused')
        self.path=path.resolve()
        if self.path.is_symlink():raise ValueError('symlink outbox refused')
        if self.path.exists():
            # Read-only probe: never initialize somebody else's SQLite database.
            connection=sqlite3.connect(self.path.as_uri()+'?mode=ro',uri=True)
            try:
                if connection.execute('PRAGMA application_id').fetchone()[0]!=APP_ID:
                    raise ValueError('not an ArbitrageX audit outbox')
            finally:connection.close()
        else:
            self.path.parent.mkdir(parents=True,exist_ok=True)
        with self._connect() as db:
            db.execute(f'PRAGMA application_id={APP_ID}')
            db.execute('PRAGMA journal_mode=WAL')
            db.execute('PRAGMA synchronous=FULL')
            db.execute('''CREATE TABLE IF NOT EXISTS audit_outbox (
                event_id TEXT PRIMARY KEY, payload_hash TEXT NOT NULL, payload_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL, expires_at_ms INTEGER NOT NULL,
                state TEXT NOT NULL CHECK(state IN ('pending','leased','acked','expired','dead_letter')),
                attempts INTEGER NOT NULL DEFAULT 0, next_attempt_ms INTEGER NOT NULL,
                lease_until_ms INTEGER, lease_token TEXT, last_error TEXT,
                sink_receipt TEXT)''')
        self.path.chmod(0o600)
    @contextmanager
    def _connect(self):
        db=sqlite3.connect(self.path,timeout=5)
        db.row_factory=sqlite3.Row
        db.execute('PRAGMA synchronous=FULL')
        try:
            with db:
                yield db
        finally:
            db.close()
    def enqueue(self,event_id:str,payload:Any,now_ms:int,expires_at_ms:int)->str:
        if not event_id or len(event_id)>256:raise ValueError('invalid event id')
        if not isinstance(now_ms,int) or not isinstance(expires_at_ms,int) or now_ms<=0:raise ValueError('explicit timestamps required')
        payload_hash=digest(payload);body=canonical_bytes(payload).decode()
        with self._connect() as db:
            db.execute('BEGIN IMMEDIATE')
            old=db.execute('SELECT payload_hash FROM audit_outbox WHERE event_id=?',(event_id,)).fetchone()
            if old:
                if old['payload_hash']!=payload_hash:raise ValueError('conflicting idempotency key')
                return payload_hash
            state='expired' if expires_at_ms<=now_ms else 'pending'
            db.execute('INSERT INTO audit_outbox(event_id,payload_hash,payload_json,created_at_ms,expires_at_ms,state,next_attempt_ms) VALUES(?,?,?,?,?,?,?)',
                       (event_id,payload_hash,body,now_ms,expires_at_ms,state,now_ms))
        return payload_hash
    def claim(self,now_ms:int,lease_ms:int,limit:int)->list[dict[str,Any]]:
        if lease_ms<=0 or not 1<=limit<=1000:raise ValueError('bounded positive lease/batch required')
        claimed=[]
        with self._connect() as db:
            db.execute('BEGIN IMMEDIATE')
            db.execute("UPDATE audit_outbox SET state='expired',lease_token=NULL,lease_until_ms=NULL WHERE state IN ('pending','leased') AND expires_at_ms<=?",(now_ms,))
            db.execute("UPDATE audit_outbox SET state='pending',lease_token=NULL WHERE state='leased' AND lease_until_ms<=?",(now_ms,))
            rows=db.execute("SELECT * FROM audit_outbox WHERE state='pending' AND next_attempt_ms<=? ORDER BY created_at_ms,event_id LIMIT ?",(now_ms,limit)).fetchall()
            for row in rows:
                token=uuid.uuid4().hex
                db.execute("UPDATE audit_outbox SET state='leased',attempts=attempts+1,lease_token=?,lease_until_ms=? WHERE event_id=?",(token,now_ms+lease_ms,row['event_id']))
                claimed.append({'event_id':row['event_id'],'payload_hash':row['payload_hash'],
                                'payload':json.loads(row['payload_json']),'lease_token':token,'attempt':row['attempts']+1})
        return claimed
    def ack(self,event_id:str,lease_token:str,payload_hash:str,sink_receipt:str)->None:
        if not sink_receipt:raise ValueError('sink receipt required; send attempted is not delivered')
        with self._connect() as db:
            result=db.execute("UPDATE audit_outbox SET state='acked',sink_receipt=?,lease_token=NULL,lease_until_ms=NULL WHERE event_id=? AND state='leased' AND lease_token=? AND payload_hash=?",(sink_receipt,event_id,lease_token,payload_hash))
            if result.rowcount!=1:raise ValueError('stale lease, bad hash or unknown event')
    def fail(self,event_id:str,lease_token:str,reason_code:str,next_attempt_ms:int,max_attempts:int)->None:
        if max_attempts<=0 or not reason_code or len(reason_code)>128:raise ValueError('bounded reason and attempts required')
        with self._connect() as db:
            db.execute('BEGIN IMMEDIATE')
            row=db.execute("SELECT attempts FROM audit_outbox WHERE event_id=? AND lease_token=? AND state='leased'",(event_id,lease_token)).fetchone()
            if not row:raise ValueError('stale lease')
            state='dead_letter' if row['attempts']>=max_attempts else 'pending'
            db.execute('UPDATE audit_outbox SET state=?,last_error=?,next_attempt_ms=?,lease_token=NULL,lease_until_ms=NULL WHERE event_id=?',(state,reason_code,next_attempt_ms,event_id))
    def states(self)->dict[str,int]:
        with self._connect() as db:
            return {r[0]:r[1] for r in db.execute('SELECT state,COUNT(*) FROM audit_outbox GROUP BY state')}
