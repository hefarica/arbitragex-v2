#!/usr/bin/env python3
"""Stage this package OUTSIDE the active cartridge loader. Default is read-only.
No Git settings, workflows, environment, deployment, mode or existing code edits.
"""
from __future__ import annotations
import argparse,hashlib,json,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parent
STAGE=Path('integration/agent-cartridges-v4')
COPY_DIRS=('generated','runtime','spec','sources','reports','tools','tests','reference_repo','validation')
COPY_DOCS=('README_ES.md','INTEGRACION.md','DECISIONES_Y_CONFLICTOS.md','VALIDACION.md','REQUIREMENTS_TRACEABILITY.md','INDEX.html')

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def items():
    found=[]
    for name in COPY_DIRS:
        d=ROOT/name
        if d.exists():
            found += [p for p in d.rglob('*')if p.is_file()and '__pycache__'not in p.parts and 'target'not in p.parts and p.suffix not in('.pyc',)]
    found += [ROOT/n for n in COPY_DOCS if(ROOT/n).is_file()]
    found.append(ROOT/'stage_update.py')
    return sorted(set(found))
def stage(repo:Path,apply:bool=False)->dict:
    repo=repo.expanduser().resolve(strict=True)
    if not repo.is_dir()or not((repo/'.git').exists()or(repo/'backend').is_dir()):raise ValueError('repository_root_not_identified')
    dest=repo/STAGE
    for p in [repo/'integration',dest]:
        if p.is_symlink():raise ValueError('symlink_stage_forbidden')
    plan=[];conflicts=[]
    for src in items():
        rel=src.relative_to(ROOT)
        if src.is_symlink()or rel.is_absolute()or'..'in rel.parts:raise ValueError('unsafe_source_path')
        target=dest/rel
        if not target.resolve().is_relative_to(repo):raise ValueError('target_outside_repository')
        for p in [target,*target.parents]:
            if p==repo:break
            if p.is_symlink():raise ValueError('symlink_target_forbidden')
        if target.exists():
            if not target.is_file()or sha(target)!=sha(src):conflicts.append(str(target))
            else:continue
        plan.append((src,target))
    if conflicts:raise ValueError('existing_files_differ_no_overwrite:'+json.dumps(conflicts[:10]))
    if apply:
        # All destinations preflighted before writing. Exclusive creation: no
        # overwrite even if an unrelated process creates a target after preflight.
        for src,target in plan:
            target.parent.mkdir(parents=True,exist_ok=True)
            with target.open('xb')as f:f.write(src.read_bytes())
    return {'mode':'STAGED'if apply else'DRY_RUN','files_to_add'if not apply else'files_added':len(plan),'destination':str(dest),'active_cartridges_replaced':0,'existing_configuration_changes':0,'activation':'NOT_PERFORMED_REQUIRES_V4_INTEGRATION_AND_NATIVE_TESTS'}
def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--repo',type=Path,required=True);ap.add_argument('--apply',action='store_true');a=ap.parse_args()
    try:print(json.dumps(stage(a.repo,a.apply),indent=2));return 0
    except (OSError,ValueError)as e:print(json.dumps({'status':'REFUSED','reason':str(e)}),file=sys.stderr);return 2
if __name__=='__main__':sys.exit(main())
