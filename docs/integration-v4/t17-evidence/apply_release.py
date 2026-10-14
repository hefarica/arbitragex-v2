#!/usr/bin/env python3
# apply_release.py — alpha7 FALSATOR reconstruction of the release APPLIER with the
# A0d BLAST-RADIUS guard (contract t17 F7). The applier writes ONLY the intentional
# diff of release R-20261013T085048Z (ASSEMBLY.md): detector_policy.rs (M) +
# releases/ + t9-evidence/. Transactional: it validates EVERY destination against
# the blast radius BEFORE the first write; any destination outside => ABORT with
# zero bytes written anywhere.
#   usage: apply_release.py <release_dir> <target_repo> [--extra <relpath>] [--extra-content <text>]
# exit codes: 0 applied; 2 ABORT blast-radius violation (nothing written).
import shutil, sys
from pathlib import Path

BLAST_RADIUS = [
    "backend/searcher-rs/src/detector_policy.rs",
    "docs/integration-v4/releases/",
    "docs/integration-v4/t9-evidence/",
]

def in_blast(rel_posix: str) -> bool:
    for p in BLAST_RADIUS:
        if p.endswith("/"):
            if rel_posix.startswith(p): return True
        elif rel_posix == p:
            return True
    return False

def main():
    rel = Path(sys.argv[1]).resolve()
    tgt = Path(sys.argv[2]).resolve()
    extra = None
    extra_content = "tamper"
    if "--extra" in sys.argv:
        extra = sys.argv[sys.argv.index("--extra") + 1]
    if "--extra-content" in sys.argv:
        extra_content = sys.argv[sys.argv.index("--extra-content") + 1]

    # plan: (source, dest_rel_posix)
    plan = [
        (rel / "gen/detector_policy.rs", "backend/searcher-rs/src/detector_policy.rs"),
        (rel, "docs/integration-v4/releases/" + rel.name),
        (rel.parent.parent / "t9-evidence", "docs/integration-v4/t9-evidence/"),
    ]
    if extra:
        plan.append((None, extra))

    # GUARD: validate all destinations BEFORE any write
    violations = [d for _, d in plan if not in_blast(d)]
    if violations:
        print("ABORT [A0d blast radius]: destinations outside BLAST_RADIUS:", violations)
        print("BLAST_RADIUS =", BLAST_RADIUS)
        print("NOTHING WRITTEN (transactional abort before first write)")
        sys.exit(2)

    # apply (atomic-ish: write all planned files)
    for src, dst in plan:
        d = tgt / dst
        if src is None:
            d.parent.mkdir(parents=True, exist_ok=True)
            d.write_text(extra_content, encoding="utf-8")
            print("WROTE extra:", d)
        elif src.is_dir():
            if d.exists(): shutil.rmtree(d)
            shutil.copytree(src, d)
            print("COPIED dir:", d)
        else:
            d.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, d)
            print("COPIED file:", d)
    print("APPLIED OK")
    sys.exit(0)

if __name__ == "__main__":
    main()
