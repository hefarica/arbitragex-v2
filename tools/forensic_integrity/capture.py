"""Bounded, read-only HTTP capture. No discovery scanning, redirects or auth.

Explicit public URLs only. HTTP success is not a deployment certificate. This
collector never POSTs, changes an operator, reads .env, signs or executes trades.
"""
from __future__ import annotations
import hashlib
import json
import re
import socket
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

SECRET_KEY = re.compile(r'(?:password|secret|private.?key|api.?key|authorization|access.?token|refresh.?token|cookie)', re.I)
MAX_BYTES = 2_000_000

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def redact(value: Any) -> Any:
    if isinstance(value, dict):
        return {k: "[REDACTED]" if SECRET_KEY.search(k) else redact(v) for k,v in value.items()}
    if isinstance(value, list):
        return [redact(v) for v in value]
    return value


def validate_url(url: str, allow_loopback: bool = False) -> str:
    parsed = urllib.parse.urlsplit(url)
    if parsed.username or parsed.password or parsed.fragment:
        raise ValueError("userinfo and fragments are forbidden")
    if not parsed.hostname:
        raise ValueError("hostname required")
    if any(SECRET_KEY.search(k) for k,_ in urllib.parse.parse_qsl(parsed.query)):
        raise ValueError("credentials in query string are forbidden")
    if parsed.scheme != 'https':
        if not (allow_loopback and parsed.scheme == 'http' and parsed.hostname in {'localhost','127.0.0.1','::1'}):
            raise ValueError("HTTPS required; explicit --allow-loopback for localhost HTTP")
    return url


def capture(url: str, timeout: float = 8, allow_loopback: bool = False) -> dict[str, Any]:
    validate_url(url, allow_loopback)
    if not 0 < timeout <= 30:
        raise ValueError("timeout must be >0 and <=30 seconds")
    started = time.time_ns() // 1_000_000
    out: dict[str,Any] = {"url":url,"method":"GET","observed_at_ms":started,
                          "http_status":None,"verification":"UNAVAILABLE",
                          "production_certified":False}
    request=urllib.request.Request(url,headers={'User-Agent':'ArbitrageX-Forensic-ReadOnly/1.0','Accept':'application/json,text/html'})
    try:
        with urllib.request.build_opener(NoRedirect).open(request,timeout=timeout) as response:
            body=response.read(MAX_BYTES+1)
            if len(body)>MAX_BYTES:
                raise ValueError("capture size limit exceeded")
            out['http_status']=response.status
            out['content_type']=response.headers.get('Content-Type','')
            out['raw_body_sha256']=hashlib.sha256(body).hexdigest()
            out['raw_byte_count']=len(body)
            out['verification']='HTTP_RESPONSE_OBSERVED_ONLY'
            try:
                out['body_redacted']=redact(json.loads(body))
                out['redacted_body_sha256']=hashlib.sha256(json.dumps(out['body_redacted'],sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()
            except (ValueError,UnicodeError):
                # Avoid embedding cookies/tokens serialized in document HTML.
                out['body_saved']=False
                out['document_only']=True
    except urllib.error.HTTPError as exc:
        out['http_status']=exc.code
        out['error_code']=f'HTTP_{exc.code}'
    except (urllib.error.URLError, TimeoutError, socket.timeout, OSError, ValueError) as exc:
        # Exception text can contain provider URLs or secrets; keep type only.
        out['error_code']=type(exc).__name__
        out['error_detail']='Network, DNS, timeout or capture failure; does not prove service outage.'
    out['elapsed_ms']=(time.time_ns()//1_000_000)-started
    return out


def save_capture(path: Path, record: Any) -> None:
    path.parent.mkdir(parents=True,exist_ok=True)
    # Exclusive creation prevents overwriting previous forensic observations.
    with path.open('x',encoding='utf-8') as f:
        json.dump(record,f,indent=2,ensure_ascii=False); f.write('\n')
    path.chmod(0o600)
