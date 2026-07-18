#!/usr/bin/env python3
"""
Patch quirurgico para repo_vps_deepwiki.py v1.0.0
Aplica los 4 fixes documentados en PLAN_MAESTRO_100_DEEPWIKI.md
"""
from pathlib import Path
import sys

SCANNER = Path("C:/Users/HFRC/.claude/skills/repo-vps-deepwiki/bin/repo_vps_deepwiki.py")
if not SCANNER.exists():
    print("ERROR: Scanner no encontrado")
    sys.exit(1)

content = SCANNER.read_text(encoding="utf-8")
original = content
changes = []

# ─── FIX 2: hash_cmd con array bash ───
old_hash = '''        quoted = " ".join(shell_quote(path) for path in safe_paths)
        hash_cmd = (
            f"cd {shell_quote(repo_path)} 2>/dev/null && "
            f"for p in {quoted}; do if [ -f \\"$p\\" ]; then sha256sum -- \\"$p\\"; "
            "else printf 'MISSING  %s\\\\n' \\"$p\\"; fi; done"
        )'''

new_hash = '''        quoted = " ".join(shell_quote(path) for path in safe_paths)
        hash_cmd = (
            f"cd {shell_quote(repo_path)} 2>/dev/null && "
            f"paths=({quoted}); "
            "for p in \\"${paths[@]}\\"; do "
            "if [ -f \\"$p\\" ]; then sha256sum -- \\"$p\\"; "
            "else printf 'MISSING  %s\\\\n' \\"$p\\"; fi; done"
        )'''

if old_hash in content:
    content = content.replace(old_hash, new_hash)
    changes.append("[FIX 2] hash_cmd array bash: APLICADO")
else:
    changes.append("[FIX 2] hash_cmd: NO ENCONTRADO (ya aplicado?)")

# ─── FIX 3: meta_cmd robusto para VPS SHA ───
old_meta = '''        f"cd {shell_quote(repo_path)} 2>/dev/null && "
        "printf 'SHA\\t'; git rev-parse HEAD 2>/dev/null; "
        "printf 'BRANCH\\t'; git branch --show-current 2>/dev/null; "
        "printf 'DIRTY\\t'; git status --porcelain=v1 2>/dev/null | wc -l; "'''

new_meta = '''        "REPO_PATH=" + shell_quote(repo_path) + "; "
        "if [ -d \\"$REPO_PATH/.git\\" ]; then "
        "  cd \\"$REPO_PATH\\" || exit 1; "
        "  printf 'SHA\\t%s\\n' \\"$(git rev-parse HEAD 2>/dev/null || echo '')\\"; "
        "  printf 'BRANCH\\t%s\\n' \\"$(git branch --show-current 2>/dev/null || echo detached)\\"; "
        "  printf 'DIRTY\\t%s\\n' \\"$(git status --porcelain=v1 2>/dev/null | wc -l)\\"; "
        "else "
        "  printf 'SHA\\t\\n'; printf 'BRANCH\\t\\n'; printf 'DIRTY\\t\\n'; "
        "fi; "'''

if old_meta in content:
    content = content.replace(old_meta, new_meta)
    changes.append("[FIX 3] meta_cmd robusto: APLICADO")
else:
    changes.append("[FIX 3] meta_cmd: NO ENCONTRADO (ya aplicado?)")

# ─── FIX 4: worktree exclusion (verificar estado) ───
if '.claude/worktrees/' in content and 'subdirs.clear()' in content:
    changes.append("[FIX 4] worktree exclusion: YA APLICADO")
else:
    old_walk = '''    for root, subdirs, names in os.walk(repo):
        subdirs[:] = [d for d in subdirs if d not in IGNORE_DIRS]
        root_path = Path(root)
        rel_root = root_path.relative_to(repo).as_posix()
        if rel_root != ".":
            dirs.add(rel_root)
        for name in names:
            path = (root_path / name).relative_to(repo).as_posix()
            files.append(path)'''
    new_walk = '''    for root, subdirs, names in os.walk(repo):
        subdirs[:] = [d for d in subdirs if d not in IGNORE_DIRS]
        root_path = Path(root)
        rel_root = root_path.relative_to(repo).as_posix()
        # Skip agent worktrees — ephemeral clones, not canonical source
        if ".claude/worktrees/" in rel_root:
            subdirs.clear()
            continue
        if rel_root != ".":
            dirs.add(rel_root)
        for name in names:
            path = (root_path / name).relative_to(repo).as_posix()
            files.append(path)'''
    if old_walk in content:
        content = content.replace(old_walk, new_walk)
        changes.append("[FIX 4] worktree exclusion: APLICADO")
    else:
        changes.append("[FIX 4] worktree exclusion: NO ENCONTRADO")

for c in changes:
    print(c)

if content != original:
    SCANNER.write_text(content, encoding="utf-8")
    print(f"\n✅ Scanner actualizado: {SCANNER}")
    print("   Backup: repo_vps_deepwiki.py.v1.0.0.backup")
else:
    print("\n⚠️  Sin cambios")
