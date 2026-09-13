#!/usr/bin/env python3
"""Bounded, read-only observations for A.8/A.6. No credentials or business rows emitted."""
import datetime, hashlib, ipaddress, json, os, re, subprocess, time
import urllib.request, urllib.error, urllib.parse

def run(argv, limit=8):
    try:
        p = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=limit)
        return p.returncode, p.stdout
    except (OSError, subprocess.TimeoutExpired):
        return 124, ''

def origin(value):
    try:
        u = urllib.parse.urlsplit(value)
        if u.scheme not in ('http', 'https', 'ws', 'wss') or not u.hostname or u.username or u.password:
            return None
        port = u.port
        scheme = {'ws':'http','wss':'https'}.get(u.scheme, u.scheme)
        host = u.hostname
        if not re.fullmatch(r'[A-Za-z0-9.-]+', host):
            return None
        return scheme + '://' + host + ((':' + str(port)) if port else '')
    except ValueError:
        return None

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None

OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

def http(url):
    t = time.monotonic()
    result = {'url':url}
    try:
        try:
            r = OPENER.open(urllib.request.Request(url, headers={'Accept':'application/json','Connection':'close'}), timeout=7)
        except urllib.error.HTTPError as e:
            r = e
        with r:
            result['status'] = r.code
            data = r.read(524289)
            result['bytes_read'] = len(data)
            result['body_truncated'] = len(data) > 524288
            result['sha256'] = hashlib.sha256(data).hexdigest()
            result['content_type'] = r.headers.get('Content-Type','')
            try:
                payload = json.loads(data)
                if isinstance(payload, dict):
                    result['keys'] = sorted(payload.keys())
                    fields = ('mode','scoring_status','scoring_pipeline_state','a4_state','a5_state',
                              'live_trading','paper_only','submit_enabled','generated_at','source')
                    result['state'] = {k:v for k,v in payload.items() if k in fields and isinstance(v,(str,bool,int,type(None)))}
                    result['breaker_count'] = len(payload['breakers']) if isinstance(payload.get('breakers'),list) else None
                    result['error_keys'] = sorted(payload.get('error',{}).keys()) if isinstance(payload.get('error'),dict) else []
            except (ValueError, TypeError):
                pass
    except Exception as e:
        result['error_class'] = type(e).__name__
    result['elapsed_ms'] = round((time.monotonic()-t)*1000)
    return result

def sql(statement):
    rc, text = run(['docker','exec','arbitragex-v2-postgres-1','psql','-U','postgres','-d','arbitragex',
                    '-X','-qAt','-v','ON_ERROR_STOP=1','-c',
                    "BEGIN READ ONLY; SET LOCAL statement_timeout='1500ms'; SET LOCAL lock_timeout='300ms'; " + statement + '; COMMIT'], 5)
    if rc:
        return {'exit_code':rc,'available':False}
    try:
        value=json.loads(text)
        if isinstance(value,(dict,list)): return {'exit_code':0,'rows':[value]}
    except ValueError: pass
    rows=[]
    for line in text.splitlines():
        try:
            value=json.loads(line)
            if isinstance(value,(dict,list)): rows.append(value)
        except ValueError: pass
    return {'exit_code':rc,'rows':rows}

def main():
    out={'scope':'read_only_status_timeout_diagnosis','captured_at':datetime.datetime.now(datetime.timezone.utc).isoformat()}
    v=os.statvfs('/')
    out['root_free_bytes']=v.f_bavail*v.f_frsize
    out['load']=os.getloadavg()
    out['checkout']=run(['git','-C','/opt/arbitragex-v2','rev-parse','HEAD'])[1].strip()
    public=[]
    out['containers']=[]
    for service in ('api-server','edge','frontend','postgres','redis'):
        name='arbitragex-v2-'+service+'-1'
        rc, raw=run(['docker','inspect','--format','{{json .}}',name])
        if rc:
            out['containers'].append({'service':service,'available':False});continue
        try:
            item=json.loads(raw)
            env=dict(x.split('=',1) for x in item.get('Config',{}).get('Env',[]) if '=' in x)
            state=item.get('State',{})
            entry={'service':service,'state':state.get('Status'),'health':state.get('Health',{}).get('Status'),
                   'restarts':item.get('RestartCount'),'image_id':item.get('Image'),'started_at':state.get('StartedAt'),
                   'deploy_sha':env.get('ARBX_DEPLOY_SHA'), 'origins':{}}
            for k in ('NEXT_PUBLIC_APP_URL','NEXT_PUBLIC_EDGE_URL','NEXT_PUBLIC_WS_URL','NEXT_PUBLIC_API_URL','INTERNAL_EDGE_URL','API_BASE_URL'):
                value=origin(env.get(k,''))
                if value:
                    entry['origins'][k]=value
                    h=urllib.parse.urlsplit(value).hostname
                    try: is_public=ipaddress.ip_address(h).is_global
                    except ValueError: is_public='.' in h and h not in ('localhost',) and not h.endswith('.local')
                    if is_public and k.startswith('NEXT_PUBLIC_'): public.append(value)
            out['containers'].append(entry)
        except (ValueError,TypeError):
            out['containers'].append({'service':service,'available':False,'error':'inspect_parse'})
    out['public_origins']=list(dict.fromkeys(public))[:3]
    out['http']=[]
    for base in ('http://127.0.0.1:8080','http://127.0.0.1:8787','http://127.0.0.1:5173','http://127.0.0.1'):
        prefix='/api/v1' if base.endswith(':8080') else '/api'
        for suffix in ('/scoring/status','/risk/circuit-breakers/status'):
            out['http'].append(http(base+prefix+suffix))
    for base in out['public_origins']:
        for path in ('/api/scoring/status','/api/risk/circuit-breakers/status'):
            out['http'].append(http(base+path))
    out['pg_activity']=sql("SELECT json_build_object('state',state,'wait_type',wait_event_type,'wait',wait_event,'connections',count(*),'max_seconds',max(extract(epoch from now()-query_start))) FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() GROUP BY state,wait_event_type,wait_event")
    out['pg_tables']=sql("SELECT json_build_object('table',relname,'estimated_live_rows',n_live_tup,'dead_rows',n_dead_tup,'bytes',pg_total_relation_size(relid),'last_analyze',last_analyze,'last_autoanalyze',last_autoanalyze) FROM pg_stat_user_tables WHERE relname IN ('scored_opportunities','paper_trade_runs','executions','bayesian_priors','gate_c_validation')")
    out['pg_scoring_plan']=sql("EXPLAIN (FORMAT JSON) SELECT COUNT(*)::int AS total, MAX(created_at) AS last, MIN(created_at) AS first FROM scored_opportunities")
    rc,raw=run(['docker','exec','arbitragex-v2-redis-1','redis-cli','INFO','persistence'])
    allowed={'loading','aof_enabled','aof_last_write_status','aof_last_bgrewrite_status','rdb_last_bgsave_status'}
    out['redis']={k:v.strip() for line in raw.splitlines() if ':' in line for k,v in [line.split(':',1)] if k in allowed}
    out['redis_query_exit']=rc
    print(json.dumps(out,sort_keys=True))

if __name__=='__main__':main()
