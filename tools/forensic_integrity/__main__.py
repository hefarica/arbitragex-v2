from __future__ import annotations
import argparse
import json
from pathlib import Path
from .capture import capture, save_capture
from .integrity import reconcile, DEFAULT_STAGES, CONTROL_STAGES, validate_envelope, flatten
from .inventory import census, export


def read_json(path: Path):
    if path.stat().st_size > 20_000_000:
        raise ValueError('input larger than 20MB: split the window before reconciliation')
    def invalid_constant(value):
        raise ValueError(f'non-JSON constant: {value}')
    return json.loads(path.read_text(encoding='utf-8-sig'), parse_constant=invalid_constant)


def main() -> None:
    p=argparse.ArgumentParser(description='ArbitrageX audit-only toolkit; never modifies production/configuration.')
    subs=p.add_subparsers(dest='command',required=True)
    s=subs.add_parser('inventory'); s.add_argument('--repo',type=Path,required=True);s.add_argument('--out',type=Path,required=True)
    s.add_argument('--map',type=Path,action='append',default=[])
    s=subs.add_parser('capture');s.add_argument('--url',action='append',required=True);s.add_argument('--out',type=Path,required=True)
    s.add_argument('--timeout',type=float,default=8);s.add_argument('--allow-loopback',action='store_true')
    s=subs.add_parser('reconcile');s.add_argument('--receipts',type=Path,required=True);s.add_argument('--expected-ids',type=Path,required=True)
    s.add_argument('--out',type=Path,required=True);s.add_argument('--control',action='store_true');s.add_argument('--max-latency-ms',type=int)
    s=subs.add_parser('validate');s.add_argument('--envelope',type=Path,required=True)
    s=subs.add_parser('fields');s.add_argument('--payload',type=Path,required=True);s.add_argument('--out',type=Path,required=True)
    s=subs.add_parser('cards');s.add_argument('--request',type=Path,required=True);s.add_argument('--pricebus',type=Path,required=True)
    s.add_argument('--policy',type=Path,required=True);s.add_argument('--out',type=Path,required=True)
    args=p.parse_args()
    if args.command=='cards':
        from .real_cards import CardPolicy, compute_closed_cycle_card
        raw=read_json(args.policy)
        raw['required_fields']=tuple(raw['required_fields'])
        raw['na_reasons']={k:tuple(v) for k,v in raw['na_reasons'].items()}
        result=compute_closed_cycle_card(read_json(args.request),read_json(args.pricebus),CardPolicy(**raw))
        save_capture(args.out,result)
        print(json.dumps(result['audit'],indent=2))
        raise SystemExit(0 if result['audit']['complete'] else 2)
    if args.command=='inventory':
        repo,out=args.repo.resolve(),args.out.resolve()
        if repo==out or repo in out.parents:
            p.error('output must be outside the source repository')
        report=census(repo,args.map); export(report,out); print(json.dumps(report['summary'],indent=2));return
    if args.command=='capture':
        if len(args.url)>5:
            p.error('maximum five explicit URLs per run; this is not a load test')
        result=[capture(u,args.timeout,args.allow_loopback) for u in args.url]
        save_capture(args.out,result)
        print(json.dumps([{k:v for k,v in r.items() if k!='body_redacted'} for r in result],indent=2))
        raise SystemExit(0 if all(r['http_status']==200 for r in result) else 2)
    if args.command=='reconcile':
        result=reconcile(read_json(args.receipts),read_json(args.expected_ids),
                         CONTROL_STAGES if args.control else DEFAULT_STAGES,args.max_latency_ms)
        save_capture(args.out,result);print(json.dumps(result,indent=2))
        raise SystemExit(0 if result['verification']=='CONSISTENT' else 2)
    if args.command=='validate':
        errors=validate_envelope(read_json(args.envelope));print(json.dumps({'errors':errors,'schema_valid':not errors}))
        raise SystemExit(bool(errors))
    if args.command=='fields':
        result={'scope':'supplied_payload_only','field_values':flatten(read_json(args.payload))}
        save_capture(args.out,result);print(f'{len(result["field_values"])} observed leaves (including absent/null values)')

if __name__=='__main__':
    main()
