"""Bounded read-only observations for the two failing readiness panels.
Run by SSH on stdin. No remote files, maintenance or business-data extraction.
"""
import json, os, subprocess, time, urllib.request, urllib.error
from datetime import datetime, timezone

ROOT = '/opt/arbitragex-v2'

def command(args, timeout=8):
    t = time.monotonic()
    try:
        p = subprocess.run(args, cwd=ROOT, stdin=subprocess.DEVNULL, capture_output=True,
                           text=True, timeout=timeout)
        return {'rc': p.returncode, 'ms': round((time.monotonic()-t)*1000),
                'out': p.stdout[:100000] if p.returncode == 0 else None}
    except (OSError, subprocess.TimeoutExpired) as e:
        return {'rc': None, 'ms': round((time.monotonic()-t)*1000), 'error': type(e).__name__}

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None

def request(url):
    t = time.monotonic()
    result = {'url': url}
    try:
        try:
            r = urllib.request.build_opener(NoRedirect()).open(url, timeout=8)
        except urllib.error.HTTPError as error:
            r = error
        with r:
            result['http'] = r.status
            result['content_type'] = r.headers.get('Content-Type')
            body = r.read(100000)
        try:
            data = json.loads(body)
            if isinstance(data, dict):
                result['keys'] = sorted(data)
                result['states'] = {key: data[key] for key in (
                    'scoring_status','source','mode','scoring_pipeline_state','a4_state','a5_state',
                    'live_trading','submit_enabled','generated_at') if key in data}
                result['has_error'] = bool(data.get('error'))
        except (ValueError, UnicodeError):
            result['json'] = False
    except Exception as error:
        result['error'] = type(error).__name__
    result['ms'] = round((time.monotonic()-t)*1000)
    return result

SQL = """
BEGIN READ ONLY;
SET LOCAL statement_timeout = '1500ms';
SET LOCAL lock_timeout = '500ms';
SELECT json_build_object('server_version',current_setting('server_version'),'recovery',pg_is_in_recovery());
SELECT coalesce(json_agg(x),'[]') FROM (
  SELECT state, wait_event_type, wait_event, count(*) AS sessions,
    round(max(extract(epoch FROM now()-query_start))) AS oldest_query_seconds,
    CASE WHEN query LIKE '%scored_opportunities%' THEN 'scored_opportunities'
         WHEN query LIKE '%paper_trade_runs%' THEN 'paper_trade_runs'
         ELSE 'other' END AS query_class
  FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid()
  GROUP BY state,wait_event_type,wait_event,query_class
) x;
SELECT coalesce(json_agg(x),'[]') FROM (
  SELECT relname, n_live_tup, n_dead_tup, last_analyze, last_autoanalyze,
         pg_total_relation_size(relid) AS bytes
  FROM pg_stat_user_tables
  WHERE relname IN ('scored_opportunities','paper_trade_runs','executions','bayesian_priors','gate_c_validation')
) x;
SELECT coalesce(json_agg(x),'[]') FROM (
  SELECT tablename,indexname,indexdef FROM pg_indexes WHERE schemaname='public'
  AND tablename IN ('scored_opportunities','paper_trade_runs','executions','bayesian_priors','gate_c_validation')
) x;
ROLLBACK;
"""

def main():
    data = {'scope':'read_only_readiness_probe','utc':datetime.now(timezone.utc).isoformat()}
    data['checkout'] = command(['git','rev-parse','HEAD'])
    fs=os.statvfs(ROOT)
    data['free_bytes']=fs.f_bavail*fs.f_frsize
    data['services']=command(['docker','ps','-a','--filter','label=com.docker.compose.project=arbitragex-v2',
      '--format','{{.Names}}|{{.Status}}|{{.Image}}'])
    data['api_stamp']=command(['docker','exec','arbitragex-v2-api-server-1','node','-e',
      "console.log(JSON.stringify({sha:process.env.ARBX_DEPLOY_SHA||null,run:process.env.ARBX_DEPLOY_ID||null}))"])
    data['pg_before']=command(['docker','exec','arbitragex-v2-postgres-1','psql','-U','postgres','-d','arbitragex',
      '-X','-qAt','-v','ON_ERROR_STOP=1','-c',SQL],12)
    data['requests']=[]
    for port, paths in [
        (8080,['/api/health','/api/v1/scoring/status','/api/v1/risk/circuit-breakers/status']),
        (8787,['/health','/api/scoring/status','/api/risk/circuit-breakers/status']),
        (80,['/api/scoring/status','/api/risk/circuit-breakers/status']),
        (5173,['/live-readiness']),
    ]:
        for path in paths:
            data['requests'].append(request(f'http://127.0.0.1:{port}{path}'))
    data['pg_after']=command(['docker','exec','arbitragex-v2-postgres-1','psql','-U','postgres','-d','arbitragex',
      '-X','-qAt','-v','ON_ERROR_STOP=1','-c',SQL],12)
    print(json.dumps(data,indent=2))

if __name__=='__main__': main()
